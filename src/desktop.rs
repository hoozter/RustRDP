use std::fs;
use std::process::Command;
use std::time::Duration;
use uuid::Uuid;

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

pub fn set_window_work_area_percent(pid: u32, percent: u8) -> bool {
    let percent = percent.clamp(55, 100);
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
         const area = workspace.clientArea(KWin.MaximizeArea, window);\n\
         const ratio = {percent} / 100.0;\n\
         const width = Math.round(area.width * ratio);\n\
         const height = Math.round(area.height * ratio);\n\
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
    fn size_script_centers_the_requested_fraction_in_the_work_area() {
        let script = window_size_script(88, 75, "size-plugin");
        assert!(script.contains("window.pid === 88 && window.resizeable"));
        assert!(script.contains("workspace.clientArea(KWin.MaximizeArea, window)"));
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
