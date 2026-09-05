//! One manager per desktop session, activation from launchers, and logout detection.
use crate::tray::TrayAction;
use crossbeam_channel::{Receiver, Sender, unbounded};
use eframe::egui::Context;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use uuid::Uuid;
use zbus::blocking::{Connection, Proxy, connection::Builder};

const NAME: &str = "com.hoozter.RustRDP";
const PATH: &str = "/com/hoozter/RustRDP";

struct Activation {
    tx: Sender<TrayAction>,
    wake: Arc<Mutex<Option<Context>>>,
}

#[zbus::interface(name = "com.hoozter.RustRDP")]
impl Activation {
    fn activate(&self, profile: &str) -> zbus::fdo::Result<()> {
        let action = if profile.is_empty() {
            TrayAction::OpenWindow
        } else {
            TrayAction::Connect(
                Uuid::parse_str(profile)
                    .map_err(|_| zbus::fdo::Error::InvalidArgs("Invalid connection ID".into()))?,
            )
        };
        self.tx
            .send(action)
            .map_err(|_| zbus::fdo::Error::Failed("RustRDP is exiting".into()))?;
        if let Some(ctx) = self.wake.lock().unwrap().as_ref() {
            ctx.request_repaint();
        }
        Ok(())
    }
}

pub struct Runtime {
    connection: Connection,
    pub actions: Receiver<TrayAction>,
    wake: Arc<Mutex<Option<Context>>>,
    terminate: Arc<AtomicBool>,
    signals: Vec<signal_hook::SigId>,
}

impl Runtime {
    /// None means the existing manager accepted the activation.
    pub fn start(
        connect: Option<Uuid>,
        minimized: bool,
    ) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let (tx, actions) = unbounded();
        let wake = Arc::new(Mutex::new(None));
        let connection = Builder::session()?
            .method_timeout(Duration::from_secs(2))
            .serve_at(
                PATH,
                Activation {
                    tx: tx.clone(),
                    wake: wake.clone(),
                },
            )?
            .build()?;
        match connection
            .request_name_with_flags(NAME, zbus::fdo::RequestNameFlags::DoNotQueue.into())
        {
            Ok(zbus::fdo::RequestNameReply::PrimaryOwner) => {}
            Ok(zbus::fdo::RequestNameReply::Exists) | Err(zbus::Error::NameTaken) => {
                if connect.is_some() || !minimized {
                    let proxy = Proxy::new(&connection, NAME, PATH, NAME)?;
                    let _: () = proxy.call(
                        "Activate",
                        &(connect.map(|id| id.to_string()).unwrap_or_default(),),
                    )?;
                }
                return Ok(None);
            }
            Ok(_) => return Err("Could not exclusively own the RustRDP desktop service".into()),
            Err(error) => return Err(error.into()),
        }
        let terminate = Arc::new(AtomicBool::new(false));
        let mut runtime = Self {
            connection,
            actions,
            wake,
            terminate,
            signals: Vec::new(),
        };
        for signal in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
            runtime.signals.push(signal_hook::flag::register(
                signal,
                runtime.terminate.clone(),
            )?);
        }
        if let Some(id) = connect {
            tx.send(TrayAction::Connect(id))?;
        }
        Ok(Some(runtime))
    }

    pub fn attach(&self, ctx: &Context) {
        *self.wake.lock().unwrap() = Some(ctx.clone());
    }

    pub fn terminating(&self) -> bool {
        self.terminate.load(Ordering::Relaxed)
    }

    /// Wayland close events do not distinguish the titlebar from session logout.
    /// Ask KDE only when a close arrives; never poll or inhibit shutdown.
    pub fn desktop_is_shutting_down(&self) -> bool {
        let result = Proxy::new(
            &self.connection,
            "org.kde.ksmserver",
            "/KSMServer",
            "org.kde.KSMServerInterface",
        )
        .and_then(|proxy| proxy.call::<_, _, bool>("isShuttingDown", &()));
        match result {
            Ok(value) => value,
            Err(error) => {
                tracing::debug!(%error, "KDE logout state unavailable");
                false
            }
        }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        for id in self.signals.drain(..) {
            signal_hook::low_level::unregister(id);
        }
    }
}

pub fn should_hide_on_close(
    close_to_tray: bool,
    tray_available: bool,
    quitting: bool,
    logout: bool,
) -> bool {
    close_to_tray && tray_available && !quitting && !logout
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SessionState(Arc<AtomicBool>);

    #[zbus::interface(name = "org.kde.KSMServerInterface")]
    impl SessionState {
        #[zbus(name = "isShuttingDown")]
        fn is_shutting_down(&self) -> bool {
            self.0.load(Ordering::Relaxed)
        }
    }

    // Explicitly run on a private bus so tests never activate or close a user's app.
    #[test]
    #[ignore = "run under dbus-run-session with RUSTRDP_PRIVATE_BUS_TEST=1"]
    fn private_bus_activation_logout_and_termination() {
        assert_eq!(
            std::env::var("RUSTRDP_PRIVATE_BUS_TEST").as_deref(),
            Ok("1")
        );
        let id = Uuid::new_v4();
        let primary = Runtime::start(Some(id), false).unwrap().unwrap();
        assert!(
            matches!(primary.actions.recv().unwrap(), TrayAction::Connect(value) if value == id)
        );
        assert!(Runtime::start(None, true).unwrap().is_none());
        assert!(primary.actions.try_recv().is_err());
        assert!(Runtime::start(None, false).unwrap().is_none());
        assert!(matches!(
            primary.actions.recv().unwrap(),
            TrayAction::OpenWindow
        ));
        assert!(Runtime::start(Some(id), false).unwrap().is_none());
        assert!(
            matches!(primary.actions.recv().unwrap(), TrayAction::Connect(value) if value == id)
        );
        let proxy = Proxy::new(&primary.connection, NAME, PATH, NAME).unwrap();
        assert!(
            proxy
                .call::<_, _, ()>("Activate", &("not-a-uuid",))
                .is_err()
        );
        assert!(!primary.desktop_is_shutting_down());
        let logout = Arc::new(AtomicBool::new(false));
        let _session = Builder::session()
            .unwrap()
            .serve_at("/KSMServer", SessionState(logout.clone()))
            .unwrap()
            .name("org.kde.ksmserver")
            .unwrap()
            .build()
            .unwrap();
        assert!(!primary.desktop_is_shutting_down());
        logout.store(true, Ordering::Relaxed);
        assert!(primary.desktop_is_shutting_down());
        assert!(!should_hide_on_close(
            true,
            true,
            false,
            primary.desktop_is_shutting_down()
        ));
        signal_hook::low_level::raise(signal_hook::consts::SIGTERM).unwrap();
        assert!(primary.terminating());
        drop(proxy);
        drop(primary);
        assert!(Runtime::start(None, true).unwrap().is_some());
    }

    #[test]
    fn logout_and_quit_override_close_to_tray() {
        assert!(should_hide_on_close(true, true, false, false));
        assert!(!should_hide_on_close(true, true, false, true));
        assert!(!should_hide_on_close(true, true, true, false));
        assert!(!should_hide_on_close(true, false, false, false));
        assert!(!should_hide_on_close(false, true, false, false));
    }
}
