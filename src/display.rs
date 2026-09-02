use crate::model::Resolution;
use std::collections::BTreeSet;
use std::process::Command;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DisplayCatalog {
    pub name: Option<String>,
    pub current: Option<Resolution>,
    pub scale_percent: Option<u16>,
    pub modes: Vec<Resolution>,
}

impl DisplayCatalog {
    pub fn detect() -> Self {
        Command::new("kscreen-doctor")
            .arg("-o")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| parse_kscreen_output(&String::from_utf8_lossy(&output.stdout)))
            .unwrap_or_default()
    }

    pub fn preferred_resolution(&self) -> Resolution {
        self.current
            .or_else(|| self.modes.first().copied())
            .unwrap_or(Resolution {
                width: 1920,
                height: 1080,
            })
    }
}

fn parse_kscreen_output(raw: &str) -> DisplayCatalog {
    let plain = strip_ansi(raw);
    let mut blocks = plain
        .split("Output: ")
        .filter(|block| !block.trim().is_empty());
    let all: Vec<_> = blocks.by_ref().collect();
    let block = all
        .iter()
        .find(|block| block.contains("\n\tpriority 1"))
        .or_else(|| all.iter().find(|block| block.contains("\n\tconnected")))
        .copied()
        .unwrap_or_default();
    let name = block
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .map(str::to_owned);
    let modes_line = block
        .lines()
        .find_map(|line| line.trim().strip_prefix("Modes:"))
        .unwrap_or_default();
    let scale_percent = block.lines().find_map(|line| {
        line.trim()
            .strip_prefix("Scale:")?
            .trim()
            .parse::<f64>()
            .ok()
            .map(|scale| (scale * 100.0).round() as u16)
    });
    let mut current = None;
    let mut unique = BTreeSet::new();
    for token in modes_line.split_whitespace() {
        let Some(mode) = token.split_once(':').map(|(_, mode)| mode) else {
            continue;
        };
        let is_current = mode.contains('*');
        let dimensions = mode
            .split('@')
            .next()
            .unwrap_or_default()
            .trim_end_matches(['!', '*']);
        let Some((width, height)) = dimensions.split_once('x') else {
            continue;
        };
        let (Ok(width), Ok(height)) = (width.parse::<u32>(), height.parse::<u32>()) else {
            continue;
        };
        let resolution = Resolution { width, height };
        unique.insert((width, height));
        if is_current {
            current = Some(resolution);
        }
    }
    let mut modes: Vec<_> = unique
        .into_iter()
        .map(|(width, height)| Resolution { width, height })
        .collect();
    modes.sort_by_key(|mode| {
        (
            std::cmp::Reverse(mode.width * mode.height),
            std::cmp::Reverse(mode.width),
        )
    });
    DisplayCatalog {
        name,
        current,
        scale_percent,
        modes,
    }
}

fn strip_ansi(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for code in chars.by_ref() {
                if code.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            result.push(character);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_primary_outputs_current_and_supported_pixel_modes() {
        let output = "\x1b[32mOutput: \x1b[0m1 eDP-1 uuid\n\tenabled\n\tconnected\n\tpriority 1\n\tModes: 1:3840x2160@60.00! 2:\x1b[32m2880x1800@90.00*\x1b[0m 3:2560x1600@60.00 4:1920x1080@60.00\n\tScale: 1.85\n";
        let catalog = parse_kscreen_output(output);
        assert_eq!(catalog.name.as_deref(), Some("eDP-1"));
        assert_eq!(catalog.scale_percent, Some(185));
        assert_eq!(
            catalog.current,
            Some(Resolution {
                width: 2880,
                height: 1800
            })
        );
        assert_eq!(
            catalog.modes[0],
            Resolution {
                width: 3840,
                height: 2160
            }
        );
        assert!(catalog.modes.contains(&Resolution {
            width: 2560,
            height: 1600
        }));
    }

    #[test]
    fn selects_priority_one_when_multiple_outputs_are_present() {
        let output = "Output: 1 HDMI-A-1 uuid\n\tconnected\n\tpriority 2\n\tModes: 1:1920x1080@60.00*\nOutput: 2 DP-1 uuid\n\tconnected\n\tpriority 1\n\tModes: 2:2560x1440@60.00*\n";
        let catalog = parse_kscreen_output(output);
        assert_eq!(catalog.name.as_deref(), Some("DP-1"));
        assert_eq!(
            catalog.current,
            Some(Resolution {
                width: 2560,
                height: 1440
            })
        );
    }
}
