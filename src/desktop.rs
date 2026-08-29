use std::fs;
use std::process::Command;
use std::time::Duration;
use uuid::Uuid;

pub fn set_window_minimized(pid: u32, minimized: bool) -> bool {
    let plugin = format!("rustrdp-window-state-{}", Uuid::new_v4().simple());
    let script_path = std::env::temp_dir().join(format!("{plugin}.js"));
    let script = window_state_script(pid, minimized, &plugin);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_state_script_targets_only_the_requested_process() {
        let script = window_state_script(4242, true, "private-plugin");
        assert!(script.contains("window.pid === 4242"));
        assert!(script.contains("window.minimized = true"));
        assert!(!script.contains("workspace.activeWindow = window"));
        assert!(script.contains("unloadScript', 'private-plugin'"));
    }

    #[test]
    fn restore_script_activates_the_restored_window() {
        let script = window_state_script(99, false, "restore-plugin");
        assert!(script.contains("window.minimized = false"));
        assert!(script.contains("workspace.activeWindow = window"));
    }
}
