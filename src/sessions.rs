use crate::freerdp::ConnectionCommand;
use crate::model::Profile;
use crossbeam_channel::{Receiver, Sender, unbounded};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, ChildStderr, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use uuid::Uuid;
use zeroize::Zeroize;

const ACTIVE_AFTER: Duration = Duration::from_millis(750);
const POLL_INTERVAL: Duration = Duration::from_millis(100);
const MAX_TECHNICAL_OUTPUT: usize = 64 * 1024;

#[derive(Clone, Debug)]
pub struct Session {
    pub id: Uuid,
    pub profile_id: Uuid,
    pub profile_name: String,
    pub pid: u32,
    pub state: SessionState,
    profile: Profile,
    used_saved_credential: bool,
    credential_failure_handled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionState {
    Connecting,
    Active,
    Disconnecting,
    Exited(SessionExit),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionExit {
    pub kind: SessionExitKind,
    pub code: Option<i32>,
    pub title: String,
    pub message: String,
    pub technical_details: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionExitKind {
    Normal,
    Disconnected,
    Authentication,
    Certificate,
    Connectivity,
    Unexpected,
}

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("could not start FreeRDP: {0}")]
    Spawn(io::Error),
    #[error("could not pass the password securely to FreeRDP: {0}")]
    CredentialHandoff(io::Error),
    #[error("could not start the fullscreen safety bar: {0}")]
    Controller(io::Error),
    #[error("session not found")]
    NotFound,
}

#[derive(Debug)]
enum Control {
    Disconnect,
}

#[derive(Debug)]
enum Event {
    Active(Uuid),
    Exited(Uuid, SessionExit),
}

pub struct SessionManager {
    sessions: BTreeMap<Uuid, Session>,
    controls: BTreeMap<Uuid, Sender<Control>>,
    events_tx: Sender<Event>,
    events_rx: Receiver<Event>,
}

impl Default for SessionManager {
    fn default() -> Self {
        let (events_tx, events_rx) = unbounded();
        Self {
            sessions: BTreeMap::new(),
            controls: BTreeMap::new(),
            events_tx,
            events_rx,
        }
    }
}

impl Drop for SessionManager {
    fn drop(&mut self) {
        for control in self.controls.values() {
            let _ = control.send(Control::Disconnect);
        }
    }
}

impl SessionManager {
    pub fn sessions(&self) -> impl Iterator<Item = &Session> {
        self.sessions.values()
    }

    pub fn launch(
        &mut self,
        profile: &Profile,
        connection: ConnectionCommand,
        mut password: Option<String>,
        used_saved_credential: bool,
        show_controller: bool,
    ) -> Result<Uuid, SessionError> {
        let controller = if show_controller {
            Some(ControllerServer::start(&profile.name).map_err(SessionError::Controller)?)
        } else {
            None
        };
        let mut command = Command::new(&connection.program);
        command
            .args(&connection.arguments)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .stdin(if connection.password_via_stdin {
                Stdio::piped()
            } else {
                Stdio::null()
            });
        let mut child = command.spawn().map_err(SessionError::Spawn)?;
        if connection.password_via_stdin {
            let handoff = child
                .stdin
                .take()
                .ok_or_else(|| {
                    SessionError::CredentialHandoff(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "FreeRDP stdin was not available",
                    ))
                })
                .and_then(|mut stdin| {
                    let secret = password.as_deref().unwrap_or_default();
                    stdin
                        .write_all(secret.as_bytes())
                        .and_then(|_| stdin.write_all(b"\n"))
                        .and_then(|_| stdin.flush())
                        .map_err(SessionError::CredentialHandoff)
                });
            if let Some(secret) = password.as_mut() {
                secret.zeroize();
            }
            if let Err(error) = handoff {
                let _ = child.kill();
                return Err(error);
            }
        }
        if let Some(secret) = password.as_mut() {
            secret.zeroize();
        }

        let id = Uuid::new_v4();
        let pid = child.id();
        let (control_tx, control_rx) = unbounded();
        let events = self.events_tx.clone();
        let stderr = child.stderr.take();
        thread::Builder::new()
            .name(format!("rustrdp-session-{id}"))
            .spawn(move || watch_process(id, child, stderr, control_rx, events, controller))
            .map_err(SessionError::Spawn)?;
        self.controls.insert(id, control_tx);
        self.sessions.insert(
            id,
            Session {
                id,
                profile_id: profile.id,
                profile_name: profile.name.clone(),
                pid,
                state: SessionState::Connecting,
                profile: profile.clone(),
                used_saved_credential,
                credential_failure_handled: false,
            },
        );
        Ok(id)
    }

    pub fn disconnect(&mut self, session_id: Uuid) -> Result<(), SessionError> {
        let control = self
            .controls
            .get(&session_id)
            .ok_or(SessionError::NotFound)?;
        control
            .send(Control::Disconnect)
            .map_err(|_| SessionError::NotFound)?;
        if let Some(session) = self.sessions.get_mut(&session_id) {
            session.state = SessionState::Disconnecting;
        }
        Ok(())
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok(event) = self.events_rx.try_recv() {
            changed = true;
            match event {
                Event::Active(id) => {
                    if let Some(session) = self.sessions.get_mut(&id)
                        && session.state == SessionState::Connecting
                    {
                        session.state = SessionState::Active;
                    }
                }
                Event::Exited(id, exit) => {
                    self.controls.remove(&id);
                    if let Some(session) = self.sessions.get_mut(&id) {
                        session.state = SessionState::Exited(exit);
                    }
                }
            }
        }
        changed
    }

    pub fn dismiss_exited(&mut self, session_id: Uuid) {
        if self
            .sessions
            .get(&session_id)
            .is_some_and(|session| matches!(session.state, SessionState::Exited(_)))
        {
            self.sessions.remove(&session_id);
        }
    }

    pub fn take_saved_credential_failure(&mut self) -> Option<Profile> {
        self.sessions.values_mut().find_map(|session| {
            let rejected = session.used_saved_credential
                && !session.credential_failure_handled
                && matches!(
                    session.state,
                    SessionState::Exited(SessionExit {
                        kind: SessionExitKind::Authentication,
                        ..
                    })
                );
            rejected.then(|| {
                session.credential_failure_handled = true;
                session.profile.clone()
            })
        })
    }
}

fn watch_process(
    id: Uuid,
    mut child: std::process::Child,
    stderr: Option<ChildStderr>,
    controls: Receiver<Control>,
    events: Sender<Event>,
    mut controller: Option<ControllerServer>,
) {
    let output_reader = stderr.map(|stderr| thread::spawn(move || read_bounded(stderr)));
    let started = Instant::now();
    let mut announced_active = false;
    let mut requested_disconnect = false;
    let status = loop {
        if controls.try_recv().is_ok() {
            requested_disconnect = true;
            let _ = child.kill();
        }
        if let Some(controller) = controller.as_mut() {
            match controller.poll() {
                ControllerAction::None | ControllerAction::Ready => {}
                ControllerAction::Minimize => minimize_window(child.id()),
                ControllerAction::Disconnect => {
                    requested_disconnect = true;
                    let _ = child.kill();
                }
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if !announced_active && started.elapsed() >= ACTIVE_AFTER {
                    let _ = events.send(Event::Active(id));
                    announced_active = true;
                }
            }
            Err(_) => break None,
        }
        thread::sleep(POLL_INTERVAL);
    };
    let technical = output_reader
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default();
    let code = status.and_then(|status| status.code());
    let exit = classify_exit(code, &technical, requested_disconnect);
    let _ = events.send(Event::Exited(id, exit));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ControllerAction {
    None,
    Ready,
    Minimize,
    Disconnect,
}

struct ControllerServer {
    listener: TcpListener,
    process: Child,
    route_prefix: String,
}

impl ControllerServer {
    fn start(profile_name: &str) -> io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let token = Uuid::new_v4().simple().to_string();
        let route_prefix = format!("/{token}");
        let control_url = format!("http://{}{}", listener.local_addr()?, route_prefix);
        let qml_path = controller_qml_path().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "session-controller.qml was not found",
            )
        })?;
        let process = Command::new("qml6")
            .arg(qml_path)
            .arg("--")
            .arg(control_url)
            .arg(profile_name)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let mut controller = Self {
            listener,
            process,
            route_prefix,
        };
        controller.wait_until_ready()?;
        Ok(controller)
    }

    fn poll(&mut self) -> ControllerAction {
        if !matches!(self.process.try_wait(), Ok(None)) {
            return ControllerAction::Disconnect;
        }
        let Ok((mut stream, _)) = self.listener.accept() else {
            return ControllerAction::None;
        };
        let action = read_controller_action(&mut stream, &self.route_prefix);
        let _ = stream.write_all(
            b"HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        );
        action
    }

    fn wait_until_ready(&mut self) -> io::Result<()> {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if let Some(status) = self.process.try_wait()? {
                return Err(io::Error::other(format!(
                    "session controller exited during startup ({status})"
                )));
            }
            let Ok((mut stream, _)) = self.listener.accept() else {
                thread::sleep(Duration::from_millis(10));
                continue;
            };
            let action = read_controller_action(&mut stream, &self.route_prefix);
            let _ = stream.write_all(
                b"HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
            );
            if action == ControllerAction::Ready {
                return Ok(());
            }
        }
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "session controller did not become ready",
        ))
    }
}

impl Drop for ControllerServer {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

fn read_controller_action(stream: &mut TcpStream, route_prefix: &str) -> ControllerAction {
    let mut request = [0_u8; 2048];
    let Ok(count) = stream.read(&mut request) else {
        return ControllerAction::None;
    };
    controller_action_from_request(&String::from_utf8_lossy(&request[..count]), route_prefix)
}

fn controller_action_from_request(request: &str, route_prefix: &str) -> ControllerAction {
    let Some(path) = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
    else {
        return ControllerAction::None;
    };
    if path == format!("{route_prefix}/ready") {
        ControllerAction::Ready
    } else if path == format!("{route_prefix}/minimize") {
        ControllerAction::Minimize
    } else if path == format!("{route_prefix}/disconnect") {
        ControllerAction::Disconnect
    } else {
        ControllerAction::None
    }
}

fn controller_qml_path() -> Option<PathBuf> {
    let installed = std::env::current_exe()
        .ok()?
        .parent()?
        .parent()?
        .join("share/rustrdp/session-controller.qml");
    if installed.is_file() {
        Some(installed)
    } else {
        let development =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/session-controller.qml");
        development.is_file().then_some(development)
    }
}

fn minimize_window(pid: u32) {
    let plugin = format!("rustrdp-minimize-{}", Uuid::new_v4().simple());
    let script_path = std::env::temp_dir().join(format!("{plugin}.js"));
    let script = format!(
        "for (const window of workspace.windowList()) {{\n\
         if (window.pid === {pid}) {{ window.minimized = true; break; }}\n\
         }}\n\
         callDBus('org.kde.KWin', '/Scripting', 'org.kde.kwin.Scripting', \
         'unloadScript', '{plugin}');\n"
    );
    if fs::write(&script_path, script).is_err() {
        return;
    }
    let loaded = Command::new("qdbus6")
        .args([
            "org.kde.KWin",
            "/Scripting",
            "org.kde.kwin.Scripting.loadScript",
        ])
        .arg(&script_path)
        .arg(&plugin)
        .output()
        .is_ok_and(|output| output.status.success());
    if loaded {
        let _ = Command::new("qdbus6")
            .args(["org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.start"])
            .status();
    }
    let _ = fs::remove_file(script_path);
}

fn read_bounded(mut stderr: impl Read) -> String {
    let mut retained = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut truncated = false;
    loop {
        let Ok(count) = stderr.read(&mut buffer) else {
            break;
        };
        if count == 0 {
            break;
        }
        let remaining = MAX_TECHNICAL_OUTPUT.saturating_sub(retained.len());
        let keep = count.min(remaining);
        retained.extend_from_slice(&buffer[..keep]);
        truncated |= keep < count;
    }
    let mut output = redact_diagnostics(&String::from_utf8_lossy(&retained));
    if truncated {
        output.push_str("\n[additional FreeRDP diagnostics omitted]");
    }
    output
}

fn redact_diagnostics(text: &str) -> String {
    text.lines()
        .map(|line| {
            if line.to_ascii_lowercase().contains("password") || line.contains("/p:") {
                "[credential-related diagnostic redacted]".to_owned()
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn classify_exit(code: Option<i32>, technical: &str, requested_disconnect: bool) -> SessionExit {
    let lower = technical.to_ascii_lowercase();
    let (kind, title, message) = if requested_disconnect {
        (
            SessionExitKind::Disconnected,
            "Session disconnected",
            "The remote desktop session was closed.",
        )
    } else if lower.contains("logon_failure")
        || lower.contains("status_wrong_password")
        || lower.contains("authentication failed")
    {
        (
            SessionExitKind::Authentication,
            "Sign-in failed",
            "Check the username, domain, and password, then try again.",
        )
    } else if lower.contains("host key verification failed")
        || lower.contains("remote host identification has changed")
        || (lower.contains("host key for") && lower.contains("has changed"))
        || lower.contains("certificate not trusted, aborting")
        || lower.contains("certificate rejected")
    {
        (
            SessionExitKind::Certificate,
            "Remote identity changed",
            "The remote computer's identity changed. Verify it before reconnecting.",
        )
    } else if lower.contains("name or service not known") || lower.contains("getaddrinfo") {
        (
            SessionExitKind::Connectivity,
            "Remote computer not found",
            "Check the hostname or IP address and your network connection.",
        )
    } else if lower.contains("connect failed")
        || lower.contains("connection refused")
        || lower.contains("errconnect_connect_transport_failed")
        || lower.contains("failed to connect")
        || lower.contains("bio_read retries exceeded")
    {
        (
            SessionExitKind::Connectivity,
            "Could not reach the remote computer",
            "Check that it is online and accepts Remote Desktop connections.",
        )
    } else if code == Some(0) {
        (
            SessionExitKind::Normal,
            "Session ended",
            "The remote desktop session ended normally.",
        )
    } else {
        (
            SessionExitKind::Unexpected,
            "Session ended unexpectedly",
            "FreeRDP closed before the session completed.",
        )
    };
    SessionExit {
        kind,
        code,
        title: title.to_owned(),
        message: message.to_owned(),
        technical_details: technical.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn translates_authentication_failure_and_redacts_secret_diagnostics() {
        let technical = redact_diagnostics("ERRCONNECT_LOGON_FAILURE\npassword: hunter2");
        let exit = classify_exit(Some(131), &technical, false);
        assert_eq!(exit.title, "Sign-in failed");
        assert!(!exit.technical_details.contains("hunter2"));
    }

    #[test]
    fn routine_self_signed_warning_is_not_misclassified_as_certificate_rejection() {
        let technical = "Certificate verification failure 'self-signed certificate (18)'\n\
                         CN = workstation.example.com\n\
                         ERRCONNECT_CONNECT_TRANSPORT_FAILED";
        let exit = classify_exit(Some(1), technical, false);
        assert_eq!(exit.kind, SessionExitKind::Connectivity);
        assert_eq!(exit.title, "Could not reach the remote computer");
    }

    #[test]
    fn explicit_host_key_rejection_is_a_certificate_failure() {
        let technical = "The host key for workstation:3389 has changed\n\
                         REMOTE HOST IDENTIFICATION HAS CHANGED\n\
                         Host key verification failed\n\
                         certificate not trusted, aborting";
        let exit = classify_exit(Some(1), technical, false);
        assert_eq!(exit.kind, SessionExitKind::Certificate);
    }

    #[test]
    fn status_logon_failure_is_an_authentication_failure() {
        let exit = classify_exit(Some(1), "SPNEGO received STATUS_LOGON_FAILURE", false);
        assert_eq!(exit.kind, SessionExitKind::Authentication);
    }

    #[test]
    fn controller_accepts_only_its_private_routes() {
        let prefix = "/private-token";
        assert_eq!(
            controller_action_from_request("POST /private-token/ready HTTP/1.1\r\n", prefix),
            ControllerAction::Ready
        );
        assert_eq!(
            controller_action_from_request("POST /private-token/minimize HTTP/1.1\r\n", prefix),
            ControllerAction::Minimize
        );
        assert_eq!(
            controller_action_from_request("POST /private-token/disconnect HTTP/1.1\r\n", prefix),
            ControllerAction::Disconnect
        );
        assert_eq!(
            controller_action_from_request("POST /wrong/disconnect HTTP/1.1\r\n", prefix),
            ControllerAction::None
        );
    }

    #[test]
    fn controller_uses_the_wayland_overlay_layer() {
        let qml = include_str!("../assets/session-controller.qml");
        assert!(qml.contains("LayerShell.Window.LayerOverlay"));
        assert!(qml.contains("KeyboardInteractivityNone"));
        assert!(qml.contains("sendCommand(\"minimize\")"));
        assert!(qml.contains("sendCommand(\"disconnect\")"));
    }

    #[test]
    fn drains_large_diagnostics_while_bounding_retained_output() {
        let input = vec![b'x'; MAX_TECHNICAL_OUTPUT + 16_384];
        let output = read_bounded(std::io::Cursor::new(input));
        assert!(output.len() < MAX_TECHNICAL_OUTPUT + 100);
        assert!(output.ends_with("[additional FreeRDP diagnostics omitted]"));
    }

    #[test]
    fn tracks_a_real_child_process_to_exit() {
        let profile = Profile::default();
        let command = ConnectionCommand {
            program: PathBuf::from("/bin/sh"),
            arguments: vec![OsString::from("-c"), OsString::from("exit 0")],
            password_via_stdin: false,
        };
        let mut manager = SessionManager::default();
        let id = manager
            .launch(&profile, command, None, false, false)
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            manager.poll();
            if matches!(
                manager.sessions.get(&id).map(|session| &session.state),
                Some(SessionState::Exited(_))
            ) {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("child process exit was not observed");
    }

    #[test]
    fn reports_a_saved_credential_failure_only_once() {
        let profile = Profile::default();
        let id = Uuid::new_v4();
        let mut manager = SessionManager::default();
        manager.sessions.insert(
            id,
            Session {
                id,
                profile_id: profile.id,
                profile_name: profile.name.clone(),
                pid: 123,
                state: SessionState::Exited(classify_exit(Some(1), "STATUS_LOGON_FAILURE", false)),
                profile: profile.clone(),
                used_saved_credential: true,
                credential_failure_handled: false,
            },
        );

        assert_eq!(manager.take_saved_credential_failure(), Some(profile));
        assert!(manager.take_saved_credential_failure().is_none());
    }
}
