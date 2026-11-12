//! Settings file: ~/.config/pixelscale/config.toml. Every key is optional.

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    /// "ember", "forge", "frost" or "geode". When a Glowgrid desktop is present
    /// (~/.local/state/glowgrid/theme exists) that file wins and is followed live.
    pub theme: String,
    /// Show an icon in the system tray.
    pub tray: bool,
    /// "auto" (panel where the compositor supports it, else a window), "panel" or "window".
    pub mode: String,
    pub panel: Panel,
    /// Program and arguments used to open a terminal for SSH. The ssh command is appended.
    /// Empty means: $TERMINAL, then the first of kitty, foot, alacritty, wezterm,
    /// gnome-terminal, konsole, xterm that is installed.
    pub terminal: Vec<String>,
    /// The command run in that terminal. "{host}" is replaced by the device's name.
    pub ssh: Vec<String>,
    pub taildrop: Taildrop,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Panel {
    /// Corner the panel hangs from: "top-right", "top-left", "bottom-right", "bottom-left".
    pub anchor: String,
    /// Gap from the screen edges, in pixels.
    pub margin_x: i32,
    pub margin_y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Taildrop {
    /// Where received files are saved. Empty means the Downloads folder.
    pub directory: String,
    /// Save received files as they arrive instead of waiting for a click.
    pub auto_save: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            theme: "ember".into(),
            tray: true,
            mode: "auto".into(),
            panel: Panel::default(),
            terminal: Vec::new(),
            ssh: vec!["ssh".into(), "{host}".into()],
            taildrop: Taildrop::default(),
        }
    }
}

impl Default for Panel {
    fn default() -> Self {
        Panel {
            anchor: "top-right".into(),
            margin_x: 12,
            margin_y: 8,
            width: 440,
            height: 600,
        }
    }
}

pub fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(".config"))
        .join("pixelscale")
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

/// The Glowgrid desktop's theme file, if this machine has one.
pub fn glowgrid_theme_file() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| home().join(".local/state"))
        .join("glowgrid/theme")
}

impl Config {
    /// Load the settings file. A missing file gives the defaults; a broken one gives the
    /// defaults plus a message to show the user.
    pub fn load() -> (Config, Option<String>) {
        let path = config_dir().join("config.toml");
        match std::fs::read_to_string(&path) {
            Err(_) => (Config::default(), None),
            Ok(text) => match toml::from_str::<Config>(&text) {
                Ok(c) => (c, None),
                Err(e) => (
                    Config::default(),
                    Some(format!("{}: {}", path.display(), e.message())),
                ),
            },
        }
    }

    pub fn download_dir(&self) -> PathBuf {
        if !self.taildrop.directory.is_empty() {
            let d = &self.taildrop.directory;
            return match d.strip_prefix("~/") {
                Some(rest) => home().join(rest),
                None => PathBuf::from(d),
            };
        }
        gtk::glib::user_special_dir(gtk::glib::UserDirectory::Downloads)
            .unwrap_or_else(|| home().join("Downloads"))
    }

    /// The terminal command to prefix an SSH invocation with, e.g. ["kitty", "--"].
    pub fn terminal_command(&self) -> Option<Vec<String>> {
        if !self.terminal.is_empty() {
            return Some(self.terminal.clone());
        }
        let known: [(&str, &[&str]); 7] = [
            ("kitty", &[]),
            ("foot", &[]),
            ("alacritty", &["-e"]),
            ("wezterm", &["start", "--"]),
            ("gnome-terminal", &["--"]),
            ("konsole", &["-e"]),
            ("xterm", &["-e"]),
        ];
        if let Ok(t) = std::env::var("TERMINAL") {
            if !t.is_empty() && on_path(&t) {
                let extra = known
                    .iter()
                    .find(|(n, _)| *n == t)
                    .map(|(_, a)| *a)
                    .unwrap_or(&["-e"]);
                return Some(
                    std::iter::once(t.clone())
                        .chain(extra.iter().map(|s| s.to_string()))
                        .collect(),
                );
            }
        }
        known
            .iter()
            .find(|(name, _)| on_path(name))
            .map(|(name, args)| {
                std::iter::once(name.to_string())
                    .chain(args.iter().map(|s| s.to_string()))
                    .collect()
            })
    }

    pub fn ssh_command(&self, host: &str) -> Vec<String> {
        self.ssh.iter().map(|a| a.replace("{host}", host)).collect()
    }
}

fn on_path(program: &str) -> bool {
    if program.contains('/') {
        return std::path::Path::new(program).exists();
    }
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(program).is_file()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_when_empty() {
        let c: Config = toml::from_str("").unwrap();
        assert_eq!(c.theme, "ember");
        assert!(c.tray);
        assert_eq!(c.panel.width, 440);
        assert_eq!(c.ssh_command("home"), vec!["ssh", "home"]);
    }

    #[test]
    fn partial_file_keeps_other_defaults() {
        let c: Config = toml::from_str(
            "tray = false\nssh = [\"tailscale\", \"ssh\", \"me@{host}\"]\n[panel]\nmargin_x = 40\n",
        )
        .unwrap();
        assert!(!c.tray);
        assert_eq!(c.panel.margin_x, 40);
        assert_eq!(c.panel.margin_y, 8);
        assert_eq!(c.ssh_command("vps"), vec!["tailscale", "ssh", "me@vps"]);
    }
}
