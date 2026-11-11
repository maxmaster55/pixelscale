//! The panel: header with the power switch, four tabs, and a footer for short messages.

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::api;
use crate::config::Config;
use crate::icons;
use crate::model::*;

const TABS: [(&str, &str); 4] = [
    ("devices", "Devices"),
    ("exit", "Exit node"),
    ("drop", "Taildrop"),
    ("set", "Settings"),
];
const CELLS: usize = 16;
/// How long the panel takes to slide open or shut, and tabs to slide across.
const OPEN_MS: u32 = 180;

type ChangeHook = Box<dyn Fn(&Snapshot)>;

/// One file on its way to another device.
struct Transfer {
    sent: Arc<AtomicU64>,
    total: u64,
    /// 0 running, 1 done, 2 failed
    outcome: Rc<Cell<u8>>,
    cells: Vec<gtk::Box>,
    meta: gtk::Label,
}

pub struct Ui {
    pub window: gtk::ApplicationWindow,
    pub panel_mode: bool,
    config: Config,
    state: gtk::Label,
    sub: gtk::Label,
    power: gtk::Switch,
    tabs: Vec<gtk::ToggleButton>,
    counts: Vec<gtk::Label>,
    stack: gtk::Stack,
    devices: gtk::Box,
    exits: gtk::Box,
    targets: gtk::DropDown,
    transfers_box: gtk::Box,
    waiting_box: gtk::Box,
    settings: gtk::Box,
    message_big: gtk::Label,
    message_small: gtk::Label,
    message_button: gtk::Button,
    toast: gtk::Label,
    summary: gtk::Label,
    revealer: gtk::Revealer,
    /// The 8-cell spark chase shown in the footer while a request is running.
    spin_cells: Vec<gtk::Box>,
    spin_pos: Cell<usize>,
    working: Cell<u32>,
    snapshot: RefCell<Snapshot>,
    /// Set while the code itself moves a switch, so its handler does not call Tailscale back.
    updating: Cell<bool>,
    /// Number of dialogs open; the panel must not close itself while one has the focus.
    pub busy: Cell<u32>,
    current_tab: RefCell<String>,
    transfers: RefCell<Vec<Transfer>>,
    toast_timer: RefCell<Option<glib::SourceId>>,
    auto_save: Cell<bool>,
    /// Names of received files already announced or being saved, so each is handled once.
    seen_files: RefCell<Vec<String>>,
    /// Called after every change of state, for the tray icon.
    pub on_change: RefCell<Option<ChangeHook>>,
}

fn label(text: &str, class: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.set_xalign(0.0);
    // Letter-spaced labels (pills, group headings) are short and must not be ellipsised:
    // Pango measures them too narrow and would cut them off.
    let spaced = class
        .split_whitespace()
        .any(|c| c == "pill" || c == "group");
    if !spaced {
        l.set_ellipsize(gtk::pango::EllipsizeMode::End);
    }
    for c in class.split_whitespace() {
        l.add_css_class(c);
    }
    l
}

fn row() -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    b.add_css_class("row");
    b
}

fn dot(class: &str) -> gtk::Box {
    let d = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    d.add_css_class("dot");
    if !class.is_empty() {
        d.add_css_class(class);
    }
    d.set_valign(gtk::Align::Center);
    d.set_size_request(12, 12);
    d
}

fn two_lines(title: &gtk::Widget, second: &gtk::Widget) -> gtk::Box {
    let v = gtk::Box::new(gtk::Orientation::Vertical, 1);
    v.set_hexpand(true);
    v.set_valign(gtk::Align::Center);
    v.append(title);
    v.append(second);
    v
}

fn text_button(text: &str, primary: bool) -> gtk::Button {
    let b = gtk::Button::with_label(&text.to_uppercase());
    b.add_css_class("text");
    if primary {
        b.add_css_class("primary");
    }
    b.set_valign(gtk::Align::Center);
    b
}

fn icon_button(bits: &'static [&'static str; 7], tooltip: &str) -> gtk::Button {
    let b = gtk::Button::new();
    b.set_child(Some(&icons::icon(bits, 2)));
    b.add_css_class("icon");
    b.set_tooltip_text(Some(tooltip));
    b.set_valign(gtk::Align::Center);
    b
}

fn clear(b: &gtk::Box) {
    while let Some(c) = b.first_child() {
        b.remove(&c);
    }
}

fn scrolled(child: &gtk::Box) -> gtk::ScrolledWindow {
    child.add_css_class("page");
    let s = gtk::ScrolledWindow::new();
    s.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    s.set_child(Some(child));
    s.set_vexpand(true);
    s
}

fn state_file() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| crate::config::home().join(".local/state"))
        .join("pixelscale/auto-save")
}

fn human_size(n: u64) -> String {
    match n {
        0..=1023 => format!("{n} B"),
        1024..=1_048_575 => format!("{:.0} KB", n as f64 / 1024.0),
        1_048_576..=1_073_741_823 => format!("{:.1} MB", n as f64 / 1_048_576.0),
        _ => format!("{:.1} GB", n as f64 / 1_073_741_824.0),
    }
}

/// Read everything the panel shows. Runs on a worker thread.
pub fn fetch() -> Snapshot {
    let mut snap = Snapshot::default();
    match api::status() {
        Ok(s) => snap.status = Some(s),
        Err(e) => {
            snap.error = Some(e.to_string());
            return snap;
        }
    }
    snap.prefs = api::prefs().unwrap_or_default();
    snap.profiles = api::profiles().unwrap_or_default();
    snap.current_profile = api::current_profile().map(|p| p.id).unwrap_or_default();
    if snap.running() {
        snap.targets = api::file_targets().unwrap_or_default();
        // Listing received files needs operator rights; without them this is simply empty.
        snap.waiting = api::waiting_files().unwrap_or_default();
    }
    snap
}

impl Ui {
    pub fn new(app: &gtk::Application, config: Config, panel_mode: bool) -> Rc<Ui> {
        let window = gtk::ApplicationWindow::new(app);
        window.set_title(Some("Pixelscale"));
        window.add_css_class("pixelscale");
        window.set_decorated(false);
        window.set_default_size(config.panel.width, config.panel.height);
        window.set_hide_on_close(true);

        let panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
        panel.add_css_class("panel");
        if panel_mode {
            // leave room inside the surface for the hard offset shadow
            panel.add_css_class("shadowed");
            panel.set_size_request(config.panel.width, config.panel.height);
        } else {
            window.add_css_class("plain");
        }

        // ---- header ----
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 14);
        head.add_css_class("head");
        let state = label("", "state");
        let sub = label("", "sub");
        let who = two_lines(state.upcast_ref(), sub.upcast_ref());
        let power = gtk::Switch::new();
        power.add_css_class("power");
        power.set_valign(gtk::Align::Center);
        power.set_tooltip_text(Some("Turn Tailscale on or off"));
        head.append(&who);
        head.append(&power);
        panel.append(&head);

        // ---- tabs ----
        let tabbar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        tabbar.add_css_class("tabs");
        tabbar.set_homogeneous(true);
        let mut tabs = Vec::new();
        let mut counts = Vec::new();
        for (_, title) in TABS {
            let b = gtk::ToggleButton::new();
            b.add_css_class("tab");
            let inner = gtk::Box::new(gtk::Orientation::Horizontal, 5);
            inner.set_halign(gtk::Align::Center);
            inner.append(&gtk::Label::new(Some(&title.to_uppercase())));
            let n = gtk::Label::new(None);
            n.add_css_class("count");
            inner.append(&n);
            b.set_child(Some(&inner));
            if let Some(first) = tabs.first() {
                b.set_group(Some::<&gtk::ToggleButton>(first));
            }
            tabbar.append(&b);
            tabs.push(b);
            counts.push(n);
        }
        panel.append(&tabbar);

        // ---- pages ----
        let stack = gtk::Stack::new();
        stack.set_vexpand(true);
        stack.set_transition_type(gtk::StackTransitionType::SlideLeftRight);
        stack.set_transition_duration(OPEN_MS);
        let devices = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let exits = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let settings = gtk::Box::new(gtk::Orientation::Vertical, 8);
        stack.add_named(&scrolled(&devices), Some("devices"));
        stack.add_named(&scrolled(&exits), Some("exit"));

        // Taildrop page: the top part is built once, the lists below are refreshed.
        let drop_page = gtk::Box::new(gtk::Orientation::Vertical, 8);
        let to_row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        to_row.append(&label("SEND TO", "group"));
        let targets = gtk::DropDown::from_strings(&[]);
        targets.set_hexpand(true);
        to_row.append(&targets);
        drop_page.append(&to_row);
        let zone = gtk::Box::new(gtk::Orientation::Vertical, 8);
        zone.add_css_class("dropzone");
        let zone_title = label("Drop files here to send", "big");
        zone_title.set_xalign(0.5);
        let choose = text_button("Choose files", false);
        choose.set_halign(gtk::Align::Center);
        zone.append(&zone_title);
        zone.append(&choose);
        drop_page.append(&zone);
        let transfers_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        drop_page.append(&transfers_box);
        let waiting_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        drop_page.append(&waiting_box);
        stack.add_named(&scrolled(&drop_page), Some("drop"));
        stack.add_named(&scrolled(&settings), Some("set"));

        // message page: shown instead of the tabs' content when Tailscale is not running
        let message = gtk::Box::new(gtk::Orientation::Vertical, 10);
        message.add_css_class("message");
        message.set_valign(gtk::Align::Center);
        let message_big = label("", "big");
        message_big.set_xalign(0.5);
        let message_small = label("", "");
        message_small.set_xalign(0.5);
        message_small.set_wrap(true);
        message_small.set_justify(gtk::Justification::Center);
        message_small.set_ellipsize(gtk::pango::EllipsizeMode::None);
        let message_button = text_button("Log in", true);
        message_button.set_halign(gtk::Align::Center);
        message.append(&message_big);
        message.append(&message_small);
        message.append(&message_button);
        stack.add_named(&message, Some("message"));
        panel.append(&stack);

        // ---- footer ----
        let foot = gtk::Box::new(gtk::Orientation::Horizontal, 10);
        foot.add_css_class("foot");
        let toast = label("", "toast");
        toast.set_hexpand(true);
        let summary = label("", "");
        summary.set_xalign(1.0);
        let spinner = gtk::Box::new(gtk::Orientation::Horizontal, 2);
        spinner.set_valign(gtk::Align::Center);
        let spin_cells: Vec<gtk::Box> = (0..8)
            .map(|_| {
                let c = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                c.add_css_class("spark");
                c.set_size_request(6, 6);
                c.set_visible(false);
                spinner.append(&c);
                c
            })
            .collect();
        foot.append(&toast);
        foot.append(&spinner);
        foot.append(&summary);
        panel.append(&foot);

        // The panel slides down out of the bar when opened and back up when closed. The
        // surface keeps its full size throughout, so the compositor never has to resize it.
        let revealer = gtk::Revealer::new();
        revealer.set_transition_type(gtk::RevealerTransitionType::SlideDown);
        revealer.set_transition_duration(OPEN_MS);
        revealer.set_valign(gtk::Align::Start);
        revealer.set_child(Some(&panel));
        revealer.set_reveal_child(!panel_mode);
        let frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
        if panel_mode {
            frame.set_size_request(config.panel.width + 8, config.panel.height + 8);
        }
        frame.append(&revealer);
        window.set_child(Some(&frame));

        let auto_save = config.taildrop.auto_save
            || std::fs::read_to_string(state_file()).is_ok_and(|s| s.trim() == "on");

        let ui = Rc::new(Ui {
            window,
            panel_mode,
            config,
            state,
            sub,
            power,
            tabs,
            counts,
            stack,
            devices,
            exits,
            targets,
            transfers_box,
            waiting_box,
            settings,
            message_big,
            message_small,
            message_button,
            toast,
            summary,
            revealer,
            spin_cells,
            spin_pos: Cell::new(0),
            working: Cell::new(0),
            snapshot: RefCell::new(Snapshot::default()),
            updating: Cell::new(false),
            busy: Cell::new(0),
            current_tab: RefCell::new("devices".into()),
            transfers: RefCell::new(Vec::new()),
            toast_timer: RefCell::new(None),
            auto_save: Cell::new(auto_save),
            seen_files: RefCell::new(Vec::new()),
            on_change: RefCell::new(None),
        });

        // ---- wiring ----
        for (i, b) in ui.tabs.iter().enumerate() {
            let u = ui.clone();
            b.connect_toggled(move |b| {
                if b.is_active() && !u.updating.get() {
                    *u.current_tab.borrow_mut() = TABS[i].0.to_string();
                    u.show_page();
                }
            });
        }
        // PIXELSCALE_TAB=exit|drop|set opens on that tab; handy for screenshots and key bindings.
        let first = std::env::var("PIXELSCALE_TAB")
            .ok()
            .and_then(|t| TABS.iter().position(|(id, _)| *id == t))
            .unwrap_or(0);
        *ui.current_tab.borrow_mut() = TABS[first].0.to_string();
        ui.tabs[first].set_active(true);

        let u = ui.clone();
        ui.power.connect_state_set(move |_, on| {
            if !u.updating.get() {
                u.act(move || {
                    api::set_running(on).map(|_| {
                        if on {
                            "Tailscale is on".to_string()
                        } else {
                            "Tailscale is off".to_string()
                        }
                    })
                });
            }
            glib::Propagation::Proceed
        });

        let u = ui.clone();
        ui.message_button.connect_clicked(move |_| u.login());

        let u = ui.clone();
        choose.connect_clicked(move |_| {
            if let Some((id, name)) = u.selected_target() {
                u.choose_and_send(id, name);
            } else {
                u.say("No device can receive files right now", false);
            }
        });
        let u = ui.clone();
        Ui::accept_drops(zone.upcast_ref(), move |paths| match u.selected_target() {
            Some((id, name)) => u.send_files(&id, &name, paths),
            None => u.say("No device can receive files right now", false),
        });

        // Esc closes; in panel mode so does clicking anywhere else.
        let keys = gtk::EventControllerKey::new();
        let u = ui.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Escape {
                u.close();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        ui.window.add_controller(keys);
        if panel_mode {
            let u = ui.clone();
            ui.window.connect_is_active_notify(move |w| {
                if !w.is_active() && u.busy.get() == 0 {
                    u.close();
                }
            });
        }

        // cell progress bars advance in steps, about eight a second
        let u = ui.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(125), move || {
            u.tick_transfers();
            glib::ControlFlow::Continue
        });

        // the spark chase steps at 12 frames a second while something is in flight
        let u = ui.clone();
        glib::timeout_add_local(std::time::Duration::from_millis(83), move || {
            u.tick_spinner();
            glib::ControlFlow::Continue
        });

        ui.rebuild();
        ui
    }

    // ------------------------------------------------------------------ showing and hiding

    pub fn is_open(&self) -> bool {
        self.window.is_visible() && self.revealer.reveals_child()
    }

    pub fn open(self: &Rc<Self>) {
        self.refresh();
        self.window.present();
        self.revealer.set_reveal_child(true);
        // rows rise into place once per opening, not on every data refresh
        self.stack.add_css_class("fresh");
        let u = self.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(700), move || {
            u.stack.remove_css_class("fresh")
        });
    }

    pub fn close(self: &Rc<Self>) {
        if !self.panel_mode {
            self.window.set_visible(false);
            return;
        }
        self.revealer.set_reveal_child(false);
        let u = self.clone();
        glib::timeout_add_local_once(
            std::time::Duration::from_millis(OPEN_MS as u64 + 20),
            move || {
                // still closed after the slide: it was not reopened in the meantime
                if !u.revealer.reveals_child() {
                    u.window.set_visible(false);
                }
            },
        );
    }

    fn tick_spinner(&self) {
        let busy = self.working.get() > 0;
        if busy {
            self.spin_pos
                .set((self.spin_pos.get() + 1) % self.spin_cells.len());
        }
        let head = self.spin_pos.get();
        let n = self.spin_cells.len();
        for (i, c) in self.spin_cells.iter().enumerate() {
            c.set_visible(busy);
            c.remove_css_class("hot");
            c.remove_css_class("warm");
            if busy && i == head {
                c.add_css_class("hot");
            } else if busy && (i + 1) % n == head {
                c.add_css_class("warm");
            }
        }
    }

    // ------------------------------------------------------------------ data

    pub fn refresh(self: &Rc<Self>) {
        let u = self.clone();
        glib::spawn_future_local(async move {
            if let Ok(snap) = gio::spawn_blocking(fetch).await {
                u.apply(snap);
            }
        });
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.borrow().clone()
    }

    fn apply(self: &Rc<Self>, snap: Snapshot) {
        if *self.snapshot.borrow() == snap {
            return;
        }
        *self.snapshot.borrow_mut() = snap;
        self.rebuild();
        self.handle_received();
        if let Some(f) = self.on_change.borrow().as_ref() {
            f(&self.snapshot.borrow());
        }
    }

    /// Run a change on a worker thread, show its result in the footer, then refresh.
    fn act<F>(self: &Rc<Self>, f: F)
    where
        F: FnOnce() -> api::Result<String> + Send + 'static,
    {
        let u = self.clone();
        self.working.set(self.working.get() + 1);
        glib::spawn_future_local(async move {
            let res = gio::spawn_blocking(f).await;
            u.working.set(u.working.get().saturating_sub(1));
            match res {
                Ok(Ok(msg)) => u.say(&msg, true),
                Ok(Err(e)) => u.say(&e.to_string(), false),
                Err(_) => u.say("Something went wrong", false),
            }
            u.refresh();
        });
    }

    pub fn say(self: &Rc<Self>, text: &str, good: bool) {
        self.toast.set_text(text);
        self.toast.set_tooltip_text(Some(text));
        if good {
            self.toast.remove_css_class("bad");
        } else {
            self.toast.add_css_class("bad");
        }
        if let Some(id) = self.toast_timer.borrow_mut().take() {
            id.remove();
        }
        let u = self.clone();
        let id = glib::timeout_add_local_once(
            std::time::Duration::from_secs(if good { 3 } else { 8 }),
            move || {
                u.toast.set_text("");
                u.toast.set_tooltip_text(None);
                u.toast_timer.borrow_mut().take();
            },
        );
        *self.toast_timer.borrow_mut() = Some(id);
    }

    // ------------------------------------------------------------------ building

    fn show_page(&self) {
        let snap = self.snapshot.borrow();
        let page = if snap.running() {
            self.current_tab.borrow().clone()
        } else {
            "message".to_string()
        };
        self.stack.set_visible_child_name(&page);
    }

    fn rebuild(self: &Rc<Self>) {
        let snap = self.snapshot();
        let running = snap.running();
        self.updating.set(true);

        // header
        self.state.remove_css_class("off");
        self.state.remove_css_class("warn");
        let backend = snap
            .status
            .as_ref()
            .map(|s| s.backend_state.as_str())
            .unwrap_or("");
        let (headline, class) = match backend {
            "Running" => ("Connected", ""),
            "Starting" => ("Starting", "off"),
            "NeedsLogin" => ("Signed out", "warn"),
            "NeedsMachineAuth" => ("Waiting for approval", "warn"),
            "" => ("Not available", "warn"),
            _ => ("Off", "off"),
        };
        self.state.set_text(headline);
        if !class.is_empty() {
            self.state.add_css_class(class);
        }
        if let Some(s) = &snap.status {
            let tailnet = s
                .current_tailnet
                .as_ref()
                .map(|t| t.name.clone())
                .unwrap_or_default();
            let mut parts = vec![format!(
                "<span foreground=\"#e9def0\">{}</span>",
                glib::markup_escape_text(&s.this.name())
            )];
            if !s.this.ipv4().is_empty() {
                parts.push(glib::markup_escape_text(&s.this.ipv4()).to_string());
            }
            if !tailnet.is_empty() {
                parts.push(glib::markup_escape_text(&tailnet).to_string());
            }
            self.sub.set_markup(&parts.join(" \u{b7} "));
        } else {
            self.sub.set_text("tailscaled is not reachable");
        }
        self.power
            .set_sensitive(snap.status.is_some() && backend != "NeedsLogin");
        self.power
            .set_state(snap.prefs.want_running && snap.status.is_some());
        self.power
            .set_active(snap.prefs.want_running && snap.status.is_some());

        for b in &self.tabs {
            b.set_sensitive(running);
        }

        // message page
        self.message_button.set_visible(backend == "NeedsLogin");
        let (big, small) = match backend {
            "NeedsLogin" => (
                "You are signed out",
                "Log in to connect this device to your tailnet.".to_string(),
            ),
            "NeedsMachineAuth" => (
                "Waiting for approval",
                "An admin of this tailnet has to approve this device.".to_string(),
            ),
            "Starting" => ("Starting", "Tailscale is connecting.".to_string()),
            "" => (
                "Tailscale is not available",
                snap.error.clone().unwrap_or_default(),
            ),
            _ => (
                "Tailscale is off",
                "Turn it on with the switch above to see your devices.".to_string(),
            ),
        };
        self.message_big.set_text(big);
        self.message_small.set_text(&small);

        self.build_devices(&snap);
        self.build_exits(&snap);
        self.build_taildrop(&snap);
        self.build_settings(&snap);

        // footer
        let online = snap.peers().iter().filter(|p| p.online).count();
        let text = if !running {
            "not connected".to_string()
        } else if let Some(e) = snap.exit_node() {
            format!("{online} online \u{b7} via {}", e.name())
        } else {
            format!("{online} online")
        };
        self.summary.set_text(&text);

        self.updating.set(false);
        self.show_page();
    }

    fn build_devices(self: &Rc<Self>, snap: &Snapshot) {
        clear(&self.devices);
        let Some(status) = &snap.status else { return };
        let peers = snap.peers();
        self.counts[0].set_text(&(peers.len() + 1).to_string());
        self.devices
            .append(&self.device_row(&status.this, true, snap));
        let mut offline_header = false;
        for p in &peers {
            if !p.online && !offline_header {
                self.devices.append(&label("OFFLINE", "group"));
                offline_header = true;
            }
            self.devices.append(&self.device_row(p, false, snap));
        }
    }

    fn device_row(self: &Rc<Self>, p: &Peer, me: bool, snap: &Snapshot) -> gtk::Box {
        let r = row();
        if me {
            r.add_css_class("me");
        }
        r.append(&dot(if p.exit_node {
            "exit"
        } else if p.online || me {
            "on"
        } else {
            ""
        }));

        let title = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        title.append(&label(&p.name(), "name"));
        let mut tag = if me {
            "This device".to_string()
        } else {
            p.os_label().to_string()
        };
        if p.exit_node {
            tag.push_str(" \u{b7} exit node in use");
        } else if p.exit_node_option {
            tag.push_str(" \u{b7} exit node");
        }
        // Not ellipsised: Pango measures letter-spaced text too narrow and would cut it short.
        // The name beside it gives way instead.
        let tag_label = gtk::Label::new(Some(&tag.to_uppercase()));
        tag_label.add_css_class("tag");
        if me {
            tag_label.add_css_class("me");
        }
        if p.exit_node_option && !p.exit_node {
            tag_label.set_tooltip_text(Some("This device offers itself as an exit node"));
        }
        tag_label.set_valign(gtk::Align::Center);
        title.append(&tag_label);
        let second = if p.online || me {
            label(&p.ipv4(), "addr")
        } else {
            let now = glib::DateTime::now_utc().map(|d| d.to_unix()).unwrap_or(0);
            let seen = relative_time(&p.last_seen, now);
            label(if seen.is_empty() { "offline" } else { &seen }, "addr dim")
        };
        r.append(&two_lines(title.upcast_ref(), second.upcast_ref()));

        let acts = gtk::Box::new(gtk::Orientation::Horizontal, 3);
        let ip = p.ipv4();
        let name = p.name();

        let copy = icon_button(&icons::COPY, "Copy address");
        let (u, ip2) = (self.clone(), ip.clone());
        copy.connect_clicked(move |b| {
            b.clipboard().set_text(&ip2);
            u.say(&format!("Copied {ip2}"), true);
        });
        copy.set_sensitive(!ip.is_empty());
        acts.append(&copy);

        if !me && p.online {
            if matches!(p.os.as_str(), "linux" | "macOS" | "freebsd" | "openbsd") {
                let ssh = icon_button(&icons::TERMINAL, "Open SSH in a terminal");
                let (u, host) = (self.clone(), p.address());
                ssh.connect_clicked(move |_| u.open_ssh(&host));
                acts.append(&ssh);
            }
            let ping = icon_button(&icons::PING, "Ping");
            let (u, ip2, name2) = (self.clone(), ip.clone(), name.clone());
            ping.connect_clicked(move |_| {
                u.say(&format!("Pinging {name2}\u{2026}"), true);
                let (ip3, name3) = (ip2.clone(), name2.clone());
                u.act(move || api::ping(&ip3).map(|r| r.describe(&name3)));
            });
            acts.append(&ping);

            let target = snap
                .targets
                .iter()
                .find(|t| t.node.stable_id == p.id)
                .map(|t| t.node.stable_id.clone());
            if let Some(id) = target {
                let send = icon_button(&icons::SEND, "Send a file");
                let (u, id2, name2) = (self.clone(), id.clone(), name.clone());
                send.connect_clicked(move |_| u.choose_and_send(id2.clone(), name2.clone()));
                acts.append(&send);
                let (u, name2) = (self.clone(), name.clone());
                Ui::accept_drops(r.upcast_ref(), move |paths| {
                    u.send_files(&id, &name2, paths)
                });
            }
        }
        r.append(&acts);
        r
    }

    fn build_exits(self: &Rc<Self>, snap: &Snapshot) {
        clear(&self.exits);
        let current = snap.prefs.exit_node_id.clone();
        let add = |id: String, title: &str, hint: &str, online: bool| {
            let r = row();
            let in_use = current == id;
            r.append(&dot(if in_use && !id.is_empty() {
                "exit"
            } else if in_use {
                "on"
            } else {
                ""
            }));
            r.append(&two_lines(
                label(title, "name").upcast_ref(),
                label(hint, "hint").upcast_ref(),
            ));
            if in_use {
                let pill = label("IN USE", "pill");
                pill.set_valign(gtk::Align::Center);
                r.append(&pill);
            } else {
                let b = text_button(if id.is_empty() { "Clear" } else { "Use" }, false);
                b.set_sensitive(online);
                let (u, name) = (self.clone(), title.to_string());
                b.connect_clicked(move |_| {
                    let (id2, name2) = (id.clone(), name.clone());
                    u.act(move || {
                        api::set_exit_node(&id2).map(|_| {
                            if id2.is_empty() {
                                "Exit node cleared".to_string()
                            } else {
                                format!("Exit node: {name2}")
                            }
                        })
                    });
                });
                r.append(&b);
            }
            self.exits.append(&r);
        };
        add(
            String::new(),
            "None",
            "Traffic leaves from this device as usual",
            true,
        );
        let options: Vec<Peer> = snap
            .peers()
            .into_iter()
            .filter(|p| p.exit_node_option)
            .collect();
        if options.is_empty() {
            let l = label(
                "No device on this tailnet offers itself as an exit node.",
                "hint",
            );
            l.set_wrap(true);
            l.set_ellipsize(gtk::pango::EllipsizeMode::None);
            self.exits.append(&l);
        } else {
            self.exits.append(&label("YOUR DEVICES", "group"));
            for p in options {
                let hint = if p.online {
                    p.os_label().to_string()
                } else {
                    "offline".to_string()
                };
                add(p.id.clone(), &p.name(), &hint, p.online);
            }
        }
        self.exits.append(&label("OPTIONS", "group"));
        self.exits.append(&self.pref_row(
            "Allow local network access",
            "Keep reaching printers and devices on your LAN while using an exit node",
            "ExitNodeAllowLANAccess",
            snap.prefs.exit_node_allow_lan_access,
        ));
    }

    fn pref_row(
        self: &Rc<Self>,
        title: &str,
        hint: &str,
        field: &'static str,
        value: bool,
    ) -> gtk::Box {
        let r = row();
        let h = label(hint, "hint");
        h.set_wrap(true);
        h.set_ellipsize(gtk::pango::EllipsizeMode::None);
        r.append(&two_lines(
            label(title, "name").upcast_ref(),
            h.upcast_ref(),
        ));
        let sw = gtk::Switch::new();
        sw.set_valign(gtk::Align::Center);
        sw.set_state(value);
        sw.set_active(value);
        let (u, title2) = (self.clone(), title.to_string());
        sw.connect_state_set(move |_, on| {
            if !u.updating.get() {
                let t = title2.clone();
                u.act(move || {
                    api::set_bool_pref(field, on)
                        .map(|_| format!("{t}: {}", if on { "on" } else { "off" }))
                });
            }
            glib::Propagation::Proceed
        });
        r.append(&sw);
        r
    }

    fn build_taildrop(self: &Rc<Self>, snap: &Snapshot) {
        // keep the chosen device selected across refreshes
        let chosen = self.selected_target().map(|(id, _)| id);
        let names: Vec<String> = snap.targets.iter().map(|t| t.node.short_name()).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        self.targets.set_model(Some(&gtk::StringList::new(&refs)));
        let online: Vec<String> = snap
            .peers()
            .into_iter()
            .filter(|p| p.online)
            .map(|p| p.id)
            .collect();
        let pick = chosen
            .and_then(|id| snap.targets.iter().position(|t| t.node.stable_id == id))
            // nothing chosen yet: start on a device that is online
            .or_else(|| {
                snap.targets
                    .iter()
                    .position(|t| online.contains(&t.node.stable_id))
            });
        if let Some(i) = pick {
            self.targets.set_selected(i as u32);
        }
        self.targets.set_sensitive(!names.is_empty());

        clear(&self.waiting_box);
        self.counts[2].set_text(&if snap.waiting.is_empty() {
            String::new()
        } else {
            snap.waiting.len().to_string()
        });
        if !snap.waiting.is_empty() {
            self.waiting_box.append(&label("RECEIVED", "group"));
        }
        for f in &snap.waiting {
            let r = row();
            r.append(&two_lines(
                label(&f.name, "name").upcast_ref(),
                label(&human_size(f.size), "meta").upcast_ref(),
            ));
            let save = icon_button(&icons::SAVE, "Save to your downloads folder");
            let (u, name) = (self.clone(), f.name.clone());
            save.connect_clicked(move |_| u.save_received(name.clone()));
            let del = icon_button(&icons::CLOSE, "Discard");
            let (u, name) = (self.clone(), f.name.clone());
            del.connect_clicked(move |_| {
                let n = name.clone();
                u.act(move || api::delete_file(&n).map(|_| format!("Discarded {n}")));
            });
            let acts = gtk::Box::new(gtk::Orientation::Horizontal, 3);
            acts.append(&save);
            acts.append(&del);
            r.append(&acts);
            self.waiting_box.append(&r);
        }

        let r = row();
        let dir = self.config.download_dir();
        let h = label(
            &format!("To {}, with a notification", dir.display()),
            "hint",
        );
        h.set_ellipsize(gtk::pango::EllipsizeMode::Middle);
        r.append(&two_lines(
            label("Save received files automatically", "name").upcast_ref(),
            h.upcast_ref(),
        ));
        let sw = gtk::Switch::new();
        sw.set_valign(gtk::Align::Center);
        sw.set_state(self.auto_save.get());
        sw.set_active(self.auto_save.get());
        let u = self.clone();
        sw.connect_state_set(move |_, on| {
            if !u.updating.get() {
                u.auto_save.set(on);
                let path = state_file();
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let _ = std::fs::write(&path, if on { "on\n" } else { "off\n" });
                u.handle_received();
            }
            glib::Propagation::Proceed
        });
        r.append(&sw);
        self.waiting_box.append(&r);
    }

    fn build_settings(self: &Rc<Self>, snap: &Snapshot) {
        clear(&self.settings);
        self.settings.append(&label("ACCOUNT", "group"));
        for p in &snap.profiles {
            let r = row();
            let active = p.id == snap.current_profile;
            r.append(&dot(if active { "on" } else { "" }));
            r.append(&two_lines(
                label(&p.title(), "name").upcast_ref(),
                label(&p.user_profile.login_name, "hint").upcast_ref(),
            ));
            if active {
                let pill = label("ACTIVE", "pill");
                pill.set_valign(gtk::Align::Center);
                r.append(&pill);
            } else {
                let b = text_button("Switch", false);
                let (u, id, title) = (self.clone(), p.id.clone(), p.title());
                b.connect_clicked(move |_| {
                    let (id2, t) = (id.clone(), title.clone());
                    u.act(move || api::switch_profile(&id2).map(|_| format!("Switched to {t}")));
                });
                r.append(&b);
            }
            self.settings.append(&r);
        }
        self.settings.append(&label("THIS DEVICE", "group"));
        self.settings.append(&self.pref_row(
            "Accept routes",
            "Use subnets that other devices advertise",
            "RouteAll",
            snap.prefs.route_all,
        ));
        self.settings.append(&self.pref_row(
            "Use Tailscale DNS",
            "MagicDNS names such as your devices' short names",
            "CorpDNS",
            snap.prefs.corp_dns,
        ));
        self.settings.append(&self.pref_row(
            "Shields up",
            "Block all incoming connections",
            "ShieldsUp",
            snap.prefs.shields_up,
        ));
        if snap.status.is_some() && snap.prefs.operator_user.is_empty() {
            let l = label("Changes need permission. Run once in a terminal:\nsudo tailscale set --operator=$USER", "hint");
            l.set_wrap(true);
            l.set_selectable(true);
            l.set_ellipsize(gtk::pango::EllipsizeMode::None);
            self.settings.append(&l);
        }
    }

    // ------------------------------------------------------------------ actions

    fn login(self: &Rc<Self>) {
        let u = self.clone();
        glib::spawn_future_local(async move {
            let res = gio::spawn_blocking(|| {
                api::login_interactive()?;
                // the login URL appears in the status shortly after
                for _ in 0..20 {
                    let s = api::status()?;
                    if !s.auth_url.is_empty() {
                        return Ok(s.auth_url);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(250));
                }
                Err(api::Error::Io("no login link was offered".into()))
            })
            .await;
            match res {
                Ok(Ok(url)) => {
                    gtk::UriLauncher::new(&url).launch(
                        Some(&u.window),
                        gio::Cancellable::NONE,
                        |_| {},
                    );
                    u.say("Finish logging in in your browser", true);
                }
                Ok(Err(e)) => u.say(&e.to_string(), false),
                Err(_) => u.say("Something went wrong", false),
            }
        });
    }

    fn open_ssh(self: &Rc<Self>, host: &str) {
        let Some(mut cmd) = self.config.terminal_command() else {
            self.say(
                "No terminal found. Set `terminal` in the settings file",
                false,
            );
            return;
        };
        cmd.extend(self.config.ssh_command(host));
        match std::process::Command::new(&cmd[0]).args(&cmd[1..]).spawn() {
            Ok(_) => {
                self.say(
                    &format!("Opening {}", self.config.ssh_command(host).join(" ")),
                    true,
                );
                if self.panel_mode {
                    self.close();
                }
            }
            Err(e) => self.say(&format!("Could not start {}: {e}", cmd[0]), false),
        }
    }

    fn selected_target(&self) -> Option<(String, String)> {
        let snap = self.snapshot.borrow();
        let i = self.targets.selected();
        if i == gtk::INVALID_LIST_POSITION {
            return None;
        }
        snap.targets
            .get(i as usize)
            .map(|t| (t.node.stable_id.clone(), t.node.short_name()))
    }

    /// Call `f` with the local file paths whenever files are dropped on `widget`.
    fn accept_drops<F: Fn(Vec<PathBuf>) + 'static>(widget: &gtk::Widget, f: F) {
        let target = gtk::DropTarget::new(gdk::FileList::static_type(), gdk::DragAction::COPY);
        target.connect_drop(move |_, value, _, _| match value.get::<gdk::FileList>() {
            Ok(list) => {
                let paths: Vec<PathBuf> = list.files().iter().filter_map(|f| f.path()).collect();
                if paths.is_empty() {
                    return false;
                }
                f(paths);
                true
            }
            Err(_) => false,
        });
        let w = widget.clone();
        target.connect_enter(move |_, _, _| {
            w.add_css_class("drop-over");
            gdk::DragAction::COPY
        });
        let w = widget.clone();
        target.connect_leave(move |_| w.remove_css_class("drop-over"));
        widget.add_controller(target);
    }

    fn choose_and_send(self: &Rc<Self>, id: String, name: String) {
        let dialog = gtk::FileDialog::builder()
            .title(format!("Send to {name}"))
            .modal(true)
            .build();
        self.busy.set(self.busy.get() + 1);
        let u = self.clone();
        dialog.open_multiple(Some(&self.window), gio::Cancellable::NONE, move |res| {
            u.busy.set(u.busy.get().saturating_sub(1));
            if let Ok(list) = res {
                let paths: Vec<PathBuf> = (0..list.n_items())
                    .filter_map(|i| list.item(i).and_downcast::<gio::File>())
                    .filter_map(|f| f.path())
                    .collect();
                u.send_files(&id, &name, paths);
            }
            // the dialog took the focus away; bring the panel back
            u.window.present();
        });
    }

    fn send_files(self: &Rc<Self>, id: &str, name: &str, paths: Vec<PathBuf>) {
        for path in paths {
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            if !meta.is_file() {
                self.say("Folders cannot be sent; pick files", false);
                continue;
            }
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file")
                .to_string();

            if self.transfers_box.first_child().is_none() {
                self.transfers_box.append(&label("SENDING", "group"));
            }
            let r = gtk::Box::new(gtk::Orientation::Vertical, 6);
            r.add_css_class("row");
            let top = gtk::Box::new(gtk::Orientation::Horizontal, 10);
            let title = label(&format!("{file_name} \u{2192} {name}"), "name");
            title.set_hexpand(true);
            let meta_label = label("0%", "meta");
            top.append(&title);
            top.append(&meta_label);
            r.append(&top);
            let bar = gtk::Box::new(gtk::Orientation::Horizontal, 2);
            bar.set_homogeneous(true);
            let cells: Vec<gtk::Box> = (0..CELLS)
                .map(|_| {
                    let c = gtk::Box::new(gtk::Orientation::Horizontal, 0);
                    c.add_css_class("cell");
                    bar.append(&c);
                    c
                })
                .collect();
            r.append(&bar);
            self.transfers_box.append(&r);

            let sent = Arc::new(AtomicU64::new(0));
            let outcome = Rc::new(Cell::new(0u8));
            self.transfers.borrow_mut().push(Transfer {
                sent: sent.clone(),
                total: meta.len(),
                outcome: outcome.clone(),
                cells,
                meta: meta_label,
            });

            let (u, id2, shown) = (self.clone(), id.to_string(), file_name.clone());
            glib::spawn_future_local(async move {
                let res = gio::spawn_blocking(move || api::send_file(&id2, &path, &sent)).await;
                match res {
                    Ok(Ok(())) => {
                        outcome.set(1);
                        u.say(&format!("Sent {shown}"), true);
                    }
                    Ok(Err(e)) => {
                        outcome.set(2);
                        u.say(&e.to_string(), false);
                    }
                    Err(_) => outcome.set(2),
                }
            });
        }
        *self.current_tab.borrow_mut() = "drop".into();
        self.tabs[2].set_active(true);
        self.show_page();
    }

    fn tick_transfers(&self) {
        for t in self.transfers.borrow().iter() {
            let frac = if t.total == 0 {
                1.0
            } else {
                t.sent.load(Ordering::Relaxed) as f64 / t.total as f64
            };
            let outcome = t.outcome.get();
            let full = if outcome == 1 {
                CELLS
            } else {
                ((frac * CELLS as f64).floor() as usize).min(CELLS)
            };
            for (i, c) in t.cells.iter().enumerate() {
                for class in ["full", "done", "failed"] {
                    c.remove_css_class(class);
                }
                if i < full {
                    c.add_css_class(match outcome {
                        1 => "done",
                        2 => "failed",
                        _ => "full",
                    });
                }
            }
            t.meta.set_text(&match outcome {
                1 => "done".to_string(),
                2 => "failed".to_string(),
                _ => format!("{}%", (frac * 100.0).floor().min(99.0) as u32),
            });
        }
    }

    fn save_received(self: &Rc<Self>, name: String) {
        let dir = self.config.download_dir();
        self.act(move || {
            let path = api::save_file(&name, &dir)?;
            api::delete_file(&name)?;
            Ok(format!("Saved {}", path.display()))
        });
    }

    /// Announce newly received files, and save them if the automatic option is on.
    fn handle_received(self: &Rc<Self>) {
        let waiting = self.snapshot.borrow().waiting.clone();
        let mut seen = self.seen_files.borrow_mut();
        seen.retain(|n| waiting.iter().any(|w| &w.name == n));
        let fresh: Vec<String> = waiting
            .iter()
            .map(|w| w.name.clone())
            .filter(|n| !seen.contains(n))
            .collect();
        for name in fresh {
            seen.push(name.clone());
            if self.auto_save.get() {
                self.save_received(name.clone());
                self.notify(
                    &format!("Received {name}"),
                    &format!("Saved to {}", self.config.download_dir().display()),
                );
            } else {
                self.notify(&format!("Received {name}"), "Open Pixelscale to save it");
            }
        }
    }

    fn notify(&self, title: &str, body: &str) {
        if let Some(app) = self.window.application() {
            let n = gio::Notification::new(title);
            n.set_body(Some(body));
            app.send_notification(None, &n);
        }
    }
}
