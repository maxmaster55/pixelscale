//! The system tray icon (StatusNotifierItem), for bars that are not the Glowgrid bar.
//! It runs on its own thread; clicks are handed to the GTK main loop as `Action`s.

use crate::icons::{tray_pixmap, TrayState};
use crate::Action;
use ksni::blocking::TrayMethods;

pub struct Tray {
    pub state: TrayState,
    pub theme: String,
    pub summary: String,
}

impl ksni::Tray for Tray {
    fn id(&self) -> String {
        "pixelscale".into()
    }

    fn title(&self) -> String {
        "Pixelscale".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        [1usize, 2, 4]
            .into_iter()
            .map(|scale| {
                let (size, data) = tray_pixmap(self.state, &self.theme, scale);
                ksni::Icon {
                    width: size,
                    height: size,
                    data,
                }
            })
            .collect()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "Tailscale".into(),
            description: self.summary.clone(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        crate::post(Action::Toggle);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        let on = !matches!(self.state, TrayState::Off | TrayState::NeedsLogin);
        vec![
            StandardItem {
                label: "Open Pixelscale".into(),
                activate: Box::new(|_| crate::post(Action::Show)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: if on {
                    "Turn Tailscale off".into()
                } else {
                    "Turn Tailscale on".into()
                },
                enabled: self.state != TrayState::NeedsLogin,
                activate: Box::new(move |_| crate::post(Action::SetRunning(!on))),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|_| crate::post(Action::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub type Handle = ksni::blocking::Handle<Tray>;

/// Start the tray icon. Returns None when the session has no tray to show it in.
pub fn spawn(theme: &str) -> Option<Handle> {
    let tray = Tray {
        state: TrayState::Off,
        theme: theme.to_string(),
        summary: String::new(),
    };
    tray.spawn().ok()
}
