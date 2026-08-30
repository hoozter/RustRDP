use std::fs;
use std::process::Command;
use std::time::Duration;
use uuid::Uuid;

pub fn set_window_minimized(pid: u32, minimized: bool) -> bool {
    run_kwin_script("rustrdp-window-state", |plugin| {
        window_state_script(pid, minimized, plugin)
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
    let script_path = std::env::temp_dir().join(format!("{plugin}.js"));
    let script = make_script(&plugin);
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
        .arg(&plugin)
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
