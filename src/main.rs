//! Pixelscale: a Tailscale panel in the Glowgrid pixel style.
//!
//! One background process owns the window and the tray icon. Running `pixelscale toggle`
//! (or `show`, `hide`, `quit`) again talks to that process, so a bar button or a key binding
//! can open and close the panel instantly.

mod api;
mod config;
mod icons;
mod model;
mod theme;
mod tray;
mod ui;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use config::Config;
use icons::TrayState;

const APP_ID: &str = "io.github.maxmaster55.Pixelscale";

const HELP: &str = "\
Pixelscale: a Tailscale panel in the Glowgrid pixel style

Usage: pixelscale [COMMAND] [OPTIONS]

Commands:
  toggle        Show the panel if hidden, hide it if shown (the default)
  show          Show the panel
  hide          Hide the panel
  quit          Stop the background process
  status        Print the Tailscale state as one line of JSON and exit

Options (they apply when the background process starts):
  --window      Open as an ordinary window
  --panel       Open as a panel attached to a screen corner
  --background  Start without showing anything (for autostart)
  --no-tray     Do not show a tray icon
  -h, --help    Show this help
  -V, --version Show the version

Settings: ~/.config/pixelscale/config.toml (see the README)
";

/// Things other threads (the tray) and other invocations can ask the main loop to do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    Toggle,
    Show,
    Hide,
    Quit,
    SetRunning(bool),
}

struct App {
    app: gtk::Application,
    config: Config,
    config_error: Option<String>,
    /// Created at startup: GTK objects cannot exist before the application has started.
    css: RefCell<Option<gtk::CssProvider>>,
    theme: RefCell<String>,
    ui: RefCell<Option<Rc<ui::Ui>>>,
    tray: RefCell<Option<tray::Handle>>,
    force_window: Cell<bool>,
    force_panel: Cell<bool>,
    no_tray: Cell<bool>,
    _monitor: RefCell<Option<gio::FileMonitor>>,
}

thread_local! {
    static APP: RefCell<Option<Rc<App>>> = const { RefCell::new(None) };
}

/// Hand an action to the GTK main loop from any thread.
pub fn post(action: Action) {
    glib::idle_add_once(move || {
        let app = APP.with(|a| a.borrow().clone());
        if let Some(app) = app {
            app.perform(action);
        }
    });
}

fn current_theme(config: &Config) -> String {
    std::fs::read_to_string(config::glowgrid_theme_file())
        .ok()
        .and_then(|t| theme::parse_name(&t))
        .or_else(|| theme::parse_name(&config.theme))
        .unwrap_or("ember")
        .to_string()
}

fn tray_state(snap: &model::Snapshot) -> TrayState {
    match snap.status.as_ref().map(|s| s.backend_state.as_str()) {
        Some("Running") if snap.exit_node().is_some() => TrayState::ExitNode,
        Some("Running") => TrayState::Connected,
        Some("NeedsLogin") | Some("NeedsMachineAuth") => TrayState::NeedsLogin,
        _ => TrayState::Off,
    }
}

fn summary(snap: &model::Snapshot) -> String {
    match snap.status.as_ref().map(|s| s.backend_state.as_str()) {
        Some("Running") => {
            let online = snap.peers().iter().filter(|p| p.online).count();
            match snap.exit_node() {
                Some(e) => format!("Connected, {online} online, exit node {}", e.name()),
                None => format!("Connected, {online} online"),
            }
        }
        Some("NeedsLogin") => "Signed out".into(),
        Some(_) => "Off".into(),
        None => "Tailscale is not running".into(),
    }
}

impl App {
    fn apply_theme(&self) {
        let name = current_theme(&self.config);
        if let Some(css) = self.css.borrow().as_ref() {
            css.load_from_string(&theme::css(&name));
        }
        *self.theme.borrow_mut() = name.clone();
        if let Some(t) = self.tray.borrow().as_ref() {
            t.update(|t| t.theme = name);
        }
    }

    /// Build the window on first use.
    fn ui(self: &Rc<Self>) -> Rc<ui::Ui> {
        if let Some(ui) = self.ui.borrow().as_ref() {
            return ui.clone();
        }
        let want_panel = if self.force_window.get() {
            false
        } else if self.force_panel.get() {
            true
        } else {
            self.config.mode != "window"
        };
        let panel_mode = want_panel && gtk4_layer_shell::is_supported();
        let ui = ui::Ui::new(&self.app, self.config.clone(), panel_mode);
        if panel_mode {
            use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
            let w = &ui.window;
            w.init_layer_shell();
            w.set_namespace(Some("pixelscale"));
            w.set_layer(Layer::Overlay);
            let p = &self.config.panel;
            let (vertical, horizontal) = match p.anchor.as_str() {
                "top-left" => (Edge::Top, Edge::Left),
                "bottom-right" => (Edge::Bottom, Edge::Right),
                "bottom-left" => (Edge::Bottom, Edge::Left),
                _ => (Edge::Top, Edge::Right),
            };
            w.set_anchor(vertical, true);
            w.set_anchor(horizontal, true);
            w.set_margin(vertical, p.margin_y);
            w.set_margin(horizontal, p.margin_x);
            w.set_keyboard_mode(KeyboardMode::OnDemand);
        }
        let me = Rc::downgrade(self);
        *ui.on_change.borrow_mut() = Some(Box::new(move |snap| {
            if let Some(app) = me.upgrade() {
                if let Some(t) = app.tray.borrow().as_ref() {
                    let (state, text) = (tray_state(snap), summary(snap));
                    t.update(move |t| {
                        t.state = state;
                        t.summary = text;
                    });
                }
            }
        }));
        if let Some(msg) = &self.config_error {
            ui.say(&format!("Settings file ignored: {msg}"), false);
        }
        *self.ui.borrow_mut() = Some(ui.clone());
        ui
    }

    fn perform(self: &Rc<Self>, action: Action) {
        match action {
            Action::Quit => {
                if let Some(t) = self.tray.borrow_mut().take() {
                    t.shutdown();
                }
                self.app.quit();
            }
            Action::Hide => {
                if let Some(ui) = self.ui.borrow().as_ref() {
                    ui.close();
                }
            }
            Action::Show => self.ui().open(),
            Action::Toggle => {
                let ui = self.ui();
                if ui.is_open() {
                    ui.close();
                } else {
                    ui.open();
                }
            }
            Action::SetRunning(on) => {
                let ui = self.ui();
                glib::spawn_future_local(async move {
                    let res = gio::spawn_blocking(move || api::set_running(on)).await;
                    if let Ok(Err(e)) = res {
                        ui.say(&e.to_string(), false);
                    }
                    ui.refresh();
                });
            }
        }
    }

    fn startup(self: &Rc<Self>) {
        let css = gtk::CssProvider::new();
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &css,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        *self.css.borrow_mut() = Some(css);
        self.apply_theme();

        // follow the Glowgrid desktop's theme switch, if this machine has one
        let file = gio::File::for_path(config::glowgrid_theme_file());
        if let Ok(monitor) =
            file.monitor_file(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE)
        {
            let me = Rc::downgrade(self);
            monitor.connect_changed(move |_, _, _, _| {
                if let Some(app) = me.upgrade() {
                    app.apply_theme();
                }
            });
            *self._monitor.borrow_mut() = Some(monitor);
        }

        if self.config.tray && !self.no_tray.get() {
            *self.tray.borrow_mut() = tray::spawn(&self.theme.borrow());
        }

        // Refresh every 3 s while the panel is open, every 15 s otherwise (tray, received files).
        let me = Rc::downgrade(self);
        let ticks = Cell::new(0u32);
        glib::timeout_add_seconds_local(3, move || {
            let Some(app) = me.upgrade() else {
                return glib::ControlFlow::Break;
            };
            ticks.set(ticks.get().wrapping_add(1));
            let ui = app.ui();
            if ui.window.is_visible() || ticks.get().is_multiple_of(5) {
                ui.refresh();
            }
            glib::ControlFlow::Continue
        });
        self.ui().refresh();
    }
}

fn print_status() -> i32 {
    let snap = ui::fetch();
    let state = snap
        .status
        .as_ref()
        .map(|s| s.backend_state.clone())
        .unwrap_or_else(|| "Unavailable".into());
    let out = serde_json::json!({
        "state": state,
        "running": snap.running(),
        "online": snap.peers().iter().filter(|p| p.online).count(),
        "exit_node": snap.exit_node().map(|p| p.name()),
        "text": summary(&snap),
    });
    println!("{out}");
    if snap.status.is_some() {
        0
    } else {
        1
    }
}

fn main() -> glib::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args
        .iter()
        .any(|a| a == "-h" || a == "--help" || a == "help")
    {
        print!("{HELP}");
        return glib::ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        println!("pixelscale {}", env!("CARGO_PKG_VERSION"));
        return glib::ExitCode::SUCCESS;
    }
    const KNOWN: [&str; 9] = [
        "toggle",
        "show",
        "hide",
        "quit",
        "status",
        "--window",
        "--panel",
        "--background",
        "--no-tray",
    ];
    if let Some(bad) = args.iter().find(|a| !KNOWN.contains(&a.as_str())) {
        eprintln!("pixelscale: unknown argument '{bad}'\nTry: pixelscale --help");
        return glib::ExitCode::from(2);
    }
    if args.iter().any(|a| a == "status") {
        return glib::ExitCode::from(print_status() as u8);
    }

    let (config, config_error) = Config::load();
    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
        .build();
    let state = Rc::new(App {
        app: app.clone(),
        config,
        config_error,
        css: RefCell::new(None),
        theme: RefCell::new("ember".into()),
        ui: RefCell::new(None),
        tray: RefCell::new(None),
        force_window: Cell::new(false),
        force_panel: Cell::new(false),
        no_tray: Cell::new(false),
        _monitor: RefCell::new(None),
    });
    APP.with(|a| *a.borrow_mut() = Some(state.clone()));

    // The process stays alive with no window open, until `pixelscale quit`.
    let hold = RefCell::new(None);
    let s = state.clone();
    let started = Cell::new(false);
    app.connect_command_line(move |app, cmd| {
        let args: Vec<String> = cmd
            .arguments()
            .iter()
            .skip(1)
            .filter_map(|a| a.to_str().map(str::to_string))
            .collect();
        let has = |name: &str| args.iter().any(|a| a == name);
        let first = !started.replace(true);
        if first {
            // options only matter for the process that owns the window
            s.force_window.set(has("--window"));
            s.force_panel.set(has("--panel"));
            s.no_tray.set(has("--no-tray"));
            *hold.borrow_mut() = Some(app.hold());
            s.startup();
        }
        let action = if has("quit") {
            Some(Action::Quit)
        } else if has("hide") {
            Some(Action::Hide)
        } else if has("show") {
            Some(Action::Show)
        } else if has("toggle") {
            Some(Action::Toggle)
        } else if first && has("--background") {
            None
        } else {
            Some(Action::Toggle)
        };
        if let Some(a) = action {
            s.perform(a);
        }
        glib::ExitCode::SUCCESS
    });

    let code = app.run();
    APP.with(|a| a.borrow_mut().take());
    code
}
