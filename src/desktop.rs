use std::fs;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default)]
pub struct WindowState {
    pub found: bool,
    pub active: bool,
    pub minimized: bool,
    pub width: i32,
    pub height: i32,
    pub screen_width: i32,
    pub screen_height: i32,
}

impl WindowState {
    pub fn size_percent(self) -> f64 {
        if !self.found || self.screen_width <= 0 || self.screen_height <= 0 {
            return 0.0;
        }
        100.0
            * (self.width as f64 / self.screen_width as f64)
                .max(self.height as f64 / self.screen_height as f64)
    }
}

struct WindowEvents {
    state: Arc<Mutex<WindowState>>,
    ready: std::sync::mpsc::SyncSender<()>,
}

#[zbus::interface(name = "com.hoozter.RustRDP.WindowEvents")]
impl WindowEvents {
    #[allow(clippy::too_many_arguments)]
    fn update(
        &self,
        found: bool,
        active: bool,
        minimized: bool,
        width: i32,
        height: i32,
        screen_width: i32,
        screen_height: i32,
    ) {
        *self.state.lock().unwrap() = WindowState {
            found,
            active,
            minimized,
            width,
            height,
            screen_width,
            screen_height,
        };
        let _ = self.ready.try_send(());
    }
}

/// A session-owned KWin observer. Events replace periodic compositor queries.
pub struct WindowMonitor {
    _connection: zbus::blocking::Connection,
    state: Arc<Mutex<WindowState>>,
    plugin: String,
}

impl WindowMonitor {
    pub fn start(pid: u32) -> Result<Self, Box<dyn std::error::Error>> {
        let state = Arc::new(Mutex::new(WindowState::default()));
        let (ready, received) = std::sync::mpsc::sync_channel(1);
        let connection = zbus::blocking::connection::Builder::session()?
            .serve_at(
                "/WindowEvents",
                WindowEvents {
                    state: state.clone(),
                    ready,
                },
            )?
            .build()?;
        let plugin = format!("rustrdp-observe-{}", Uuid::new_v4().simple());
        let bus = connection.unique_name().ok_or("No session bus identity")?;
        let script = window_monitor_script(pid, bus.as_str());
        let monitor = Self {
            _connection: connection,
            state,
            plugin,
        };
        if !load_kwin_script(&monitor.plugin, &script) {
            return Err("Could not observe the remote window through KWin".into());
        }
        received
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| "KWin did not confirm remote window observation")?;
        Ok(monitor)
    }

    pub fn state(&self) -> WindowState {
        *self.state.lock().unwrap()
    }
}

impl Drop for WindowMonitor {
    fn drop(&mut self) {
        let _ = Command::new("qdbus6")
            .args([
                "org.kde.KWin",
                "/Scripting",
                "org.kde.kwin.Scripting.unloadScript",
                &self.plugin,
            ])
            .output();
    }
}

fn window_monitor_script(pid: u32, bus: &str) -> String {
    format!(
        r#"
let target = null;
function publish() {{
    const frame = target ? target.frameGeometry : {{ width: 0, height: 0 }};
    const area = target ? workspace.clientArea(KWin.ScreenArea, target) : {{ width: 0, height: 0 }};
    callDBus('{bus}', '/WindowEvents', 'com.hoozter.RustRDP.WindowEvents', 'Update',
        target !== null, target !== null && workspace.activeWindow === target,
        target !== null && target.minimized,
        Math.round(frame.width), Math.round(frame.height), Math.round(area.width), Math.round(area.height));
}}
function observe(window) {{
    if (target || window.pid !== {pid} || !window.normalWindow) return;
    target = window;
    target.frameGeometryChanged.connect(publish);
    target.minimizedChanged.connect(publish);
    target.outputChanged.connect(publish);
    publish();
}}
workspace.windowAdded.connect(observe);
workspace.windowRemoved.connect(function(window) {{ if (window === target) {{ target = null; publish(); }} }});
workspace.windowActivated.connect(publish);
for (const window of workspace.windowList()) observe(window);
publish();
"#
    )
}

pub fn set_window_minimized(pid: u32, minimized: bool) -> bool {
    run_kwin_script("rustrdp-window-state", |plugin| {
        window_state_script(pid, minimized, plugin)
    })
}

pub fn begin_window_move(pid: u32) -> bool {
    run_kwin_script("rustrdp-window-move", |plugin| {
        window_move_script(pid, plugin)
    })
}

pub fn set_window_screen_percent(pid: u32, percent: u8) -> bool {
    let percent = percent.clamp(1, 100);
    run_kwin_script("rustrdp-window-size", |plugin| {
        window_size_script(pid, percent, plugin)
    })
}

/// Hide the application in its tray without leaving a taskbar entry behind.
/// Returns false outside KWin so the caller can use native minimization as a
/// portable fallback.
pub fn set_app_tray_hidden(pid: u32, hidden: bool) -> bool {
    run_kwin_script("rustrdp-tray-state", |plugin| {
        tray_state_script(pid, hidden, plugin)
    })
}

fn run_kwin_script(prefix: &str, make_script: impl FnOnce(&str) -> String) -> bool {
    let plugin = format!("{prefix}-{}", Uuid::new_v4().simple());
    load_kwin_script(&plugin, &make_script(&plugin))
}

fn load_kwin_script(plugin: &str, script: &str) -> bool {
    let script_path = std::env::temp_dir().join(format!("{plugin}.js"));
    if fs::write(&script_path, script).is_err() {
        return false;
    }
    let loaded = Command::new("qdbus6")
        .args([
            "org.kde.KWin",
            "/Scripting",
            "org.kde.kwin.Scripting.loadScript",
        ])
        .arg(&script_path)
        .arg(plugin)
        .output()
        .is_ok_and(|output| output.status.success());
    let started = loaded
        && Command::new("qdbus6")
            .args(["org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.start"])
            .status()
            .is_ok_and(|status| status.success());
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(2));
        let _ = fs::remove_file(script_path);
    });
    started
}

fn window_state_script(pid: u32, minimized: bool, plugin: &str) -> String {
    let activate = if minimized {
        String::new()
    } else {
        "workspace.activeWindow = window;".to_owned()
    };
    format!(
        "for (const window of workspace.windowList()) {{\n\
         if (window.pid === {pid}) {{ window.minimized = {minimized}; {activate} break; }}\n\
         }}\n\
         callDBus('org.kde.KWin', '/Scripting', 'org.kde.kwin.Scripting', \
         'unloadScript', '{plugin}');\n"
    )
}

fn window_move_script(pid: u32, plugin: &str) -> String {
    format!(
        "for (const window of workspace.windowList()) {{\n\
         if (window.pid === {pid} && window.moveable) {{\n\
         window.minimized = false; workspace.activeWindow = window; \
         workspace.slotWindowMove(); break;\n\
         }}\n\
         }}\n\
         callDBus('org.kde.KWin', '/Scripting', 'org.kde.kwin.Scripting', \
         'unloadScript', '{plugin}');\n"
    )
}

fn window_size_script(pid: u32, percent: u8, plugin: &str) -> String {
    format!(
        "for (const window of workspace.windowList()) {{\n\
         if (window.pid === {pid} && window.resizeable) {{\n\
         const area = workspace.clientArea(KWin.ScreenArea, window);\n\
         const frame = window.frameGeometry;\n\
         const ratio = {percent} / 100.0;\n\
         if (frame.width <= 0 || frame.height <= 0) break;\n\
         const scale = ratio / Math.max(frame.width / area.width, frame.height / area.height);\n\
         const width = Math.round(frame.width * scale);\n\
         const height = Math.round(frame.height * scale);\n\
         const x = Math.round(area.x + (area.width - width) / 2);\n\
         const y = Math.round(area.y + (area.height - height) / 2);\n\
         window.frameGeometry = {{ x: x, y: y, width: width, height: height }};\n\
         workspace.activeWindow = window; break;\n\
         }}\n\
         }}\n\
         callDBus('org.kde.KWin', '/Scripting', 'org.kde.kwin.Scripting', \
         'unloadScript', '{plugin}');\n"
    )
}

fn tray_state_script(pid: u32, hidden: bool, plugin: &str) -> String {
    let activate = if hidden {
        String::new()
    } else {
        "workspace.activeWindow = window;".to_owned()
    };
    format!(
        "for (const window of workspace.windowList()) {{\n\
         if (window.pid === {pid}) {{\n\
         window.skipTaskbar = {hidden}; window.skipPager = {hidden}; \
         window.skipSwitcher = {hidden}; window.minimized = {hidden}; \
         {activate} break;\n\
         }}\n\
         }}\n\
         callDBus('org.kde.KWin', '/Scripting', 'org.kde.kwin.Scripting', \
         'unloadScript', '{plugin}');\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_readout_matches_actual_geometry_including_small_and_portrait_windows() {
        let state = WindowState {
            found: true,
            width: 640,
            height: 360,
            screen_width: 2560,
            screen_height: 1440,
            ..WindowState::default()
        };
        assert_eq!(state.size_percent(), 25.0);
        assert_eq!(
            WindowState {
                width: 1920,
                height: 1080,
                ..state
            }
            .size_percent(),
            75.0
        );
        assert_eq!(
            WindowState {
                width: 360,
                height: 720,
                ..state
            }
            .size_percent(),
            50.0
        );
        assert_eq!(WindowState::default().size_percent(), 0.0);
    }

    #[test]
    #[ignore = "requires explicit RUSTRDP_KWIN_TEST=1 on a KDE desktop; creates its own test window"]
    fn live_kwin_window_geometry_and_monitor_cleanup() {
        assert_eq!(std::env::var("RUSTRDP_KWIN_TEST").as_deref(), Ok("1"));
        struct TestWindow(std::process::Child);
        impl Drop for TestWindow {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        fn wait_for(monitor: &WindowMonitor, check: impl Fn(WindowState) -> bool) -> WindowState {
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            loop {
                let state = monitor.state();
                if check(state) {
                    return state;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "KWin state timeout: {state:?}"
                );
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        let temp = tempfile::tempdir().unwrap();
        let qml = temp.path().join("window.qml");
        fs::write(&qml, "import QtQuick\nWindow { visible: true; width: 640; height: 360; title: 'RustRDP isolated geometry check'; flags: Qt.Window | Qt.FramelessWindowHint; color: '#242a30' }").unwrap();
        let mut window = TestWindow(
            Command::new("qml6")
                .arg(qml)
                .env("QT_QPA_PLATFORM", "wayland")
                .spawn()
                .unwrap(),
        );
        let monitor = WindowMonitor::start(window.0.id()).unwrap();
        let initial = wait_for(&monitor, |state| state.found);
        let aspect = initial.width as f64 / initial.height as f64;
        for percent in [75, 40] {
            assert!(set_window_screen_percent(window.0.id(), percent));
            let state = wait_for(&monitor, |state| {
                (state.size_percent() - percent as f64).abs() < 0.2
            });
            assert!((state.width as f64 / state.height as f64 - aspect).abs() < 0.01);
        }
        assert!(set_window_minimized(window.0.id(), true));
        wait_for(&monitor, |state| state.minimized);
        assert!(set_window_minimized(window.0.id(), false));
        wait_for(&monitor, |state| !state.minimized && state.active);
        window.0.kill().unwrap();
        window.0.wait().unwrap();
        wait_for(&monitor, |state| !state.found);
        let plugin = monitor.plugin.clone();
        drop(monitor);
        let loaded = Command::new("qdbus6")
            .args([
                "org.kde.KWin",
                "/Scripting",
                "org.kde.kwin.Scripting.isScriptLoaded",
                &plugin,
            ])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&loaded.stdout).trim(), "false");
    }

    #[test]
    fn window_state_script_targets_only_the_requested_process() {
        let script = window_state_script(4242, true, "private-plugin");
        assert!(script.contains("window.pid === 4242"));
        assert!(script.contains("window.minimized = true"));
        assert!(!script.contains("workspace.activeWindow = window"));
        assert_eq!(script.matches('{').count(), script.matches('}').count());
        assert!(script.contains("unloadScript', 'private-plugin'"));
    }

    #[test]
    fn restore_script_activates_the_restored_window() {
        let script = window_state_script(99, false, "restore-plugin");
        assert!(script.contains("window.minimized = false"));
        assert!(script.contains("workspace.activeWindow = window"));
    }

    #[test]
    fn move_script_starts_an_interactive_move_for_only_the_remote_process() {
        let script = window_move_script(77, "move-plugin");
        assert!(script.contains("window.pid === 77 && window.moveable"));
        assert!(script.contains("workspace.activeWindow = window"));
        assert!(script.contains("workspace.slotWindowMove()"));
        assert!(script.contains("unloadScript', 'move-plugin'"));
    }

    #[test]
    fn size_script_preserves_proportions_and_uses_the_same_screen_fraction_as_the_slider() {
        let script = window_size_script(88, 75, "size-plugin");
        assert!(script.contains("window.pid === 88 && window.resizeable"));
        assert!(script.contains("workspace.clientArea(KWin.ScreenArea, window)"));
        assert!(script.contains("Math.max(frame.width / area.width, frame.height / area.height)"));
        assert!(script.contains("Math.round(frame.width * scale)"));
        assert!(script.contains("Math.round(frame.height * scale)"));
        assert!(script.contains("const ratio = 75 / 100.0"));
        assert!(script.contains("window.frameGeometry ="));
        assert!(script.contains("(area.width - width) / 2"));
        assert!(script.contains("unloadScript', 'size-plugin'"));
    }

    #[test]
    fn tray_hide_script_removes_only_the_app_window_from_desktop_lists() {
        let script = tray_state_script(4242, true, "hide-plugin");
        assert!(script.contains("window.pid === 4242"));
        assert!(script.contains("window.skipTaskbar = true"));
        assert!(script.contains("window.skipPager = true"));
        assert!(script.contains("window.skipSwitcher = true"));
        assert!(script.contains("window.minimized = true"));
        assert!(!script.contains("workspace.activeWindow = window"));
    }

    #[test]
    fn tray_restore_script_rejoins_desktop_lists_and_activates() {
        let script = tray_state_script(99, false, "show-plugin");
        assert!(script.contains("window.skipTaskbar = false"));
        assert!(script.contains("window.skipPager = false"));
        assert!(script.contains("window.skipSwitcher = false"));
        assert!(script.contains("window.minimized = false"));
        assert!(script.contains("workspace.activeWindow = window"));
    }
}
