//! UUOB's window on Linux: GTK 4 and Libadwaita through Relm4, straight on
//! the Rust core (crates/uuob-app), the same functions the Mac and Android
//! apps reach through UniFFI. Closing the window keeps UUOB running; opening
//! UUOB again brings it back, and "Quit UUOB" in the menu stops it.

mod dialogs;
mod graph;
mod ui;

use relm4::adw::prelude::*;
use relm4::{ComponentParts, ComponentSender, RelmApp, adw, gtk, gtk::gio, gtk::glib};
use serde_json::Value;

use ui::*;

/// Runs a method of the core as the UUOB app; the error is a message for the user.
pub async fn call(method: &str, params: Value) -> Result<Value, String> {
    let params = if params.is_null() { String::new() } else { params.to_string() };
    match uuob_app::ffi::call(method.into(), params).await {
        Ok(text) => Ok(serde_json::from_str(&text).unwrap_or(Value::Null)),
        Err(e) => Err(e.to_string()),
    }
}

/// Where UUOB keeps its data on Linux (the `uuob` command finds it there too).
fn data_dir() -> std::path::PathBuf {
    let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    std::env::var_os("XDG_DATA_HOME").map(Into::into).unwrap_or_else(|| home.join(".local/share")).join("app.uuob")
}

/// Everything the three pages show, loaded together.
#[derive(Default, Clone, Debug)]
pub struct Data {
    pub status: Value,
    /// The Network page's graph and the Access page's list (crates/uuob-app/src/screens.rs).
    pub graph: Value,
    pub access: Vec<Value>,
    pub version: String,
    pub devices: Vec<Value>,
    pub people: Vec<Value>,
    pub containers: Vec<Value>,
    pub apps: Value,
    pub clients: Value,
    pub asks: Vec<Value>,
    pub autostart: bool,
    pub mcp_url: String,
    pub server_error: Option<String>,
}

async fn load(width: f64, open: Vec<String>) -> Data {
    let list = |v: Result<Value, String>| v.ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    Data {
        status: call("status", Value::Null).await.unwrap_or_default(),
        graph: call("graph", serde_json::json!({ "width": width, "open": open })).await.unwrap_or_default(),
        access: list(call("access", Value::Null).await),
        version: call("version", Value::Null).await.ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default(),
        devices: list(call("devices", Value::Null).await),
        people: list(call("people", Value::Null).await),
        containers: list(call("containers", Value::Null).await),
        apps: call("apps", Value::Null).await.unwrap_or_default(),
        clients: call("clients", Value::Null).await.unwrap_or_default(),
        asks: list(call("asks", Value::Null).await),
        autostart: call("autostart", Value::Null).await.ok().and_then(|v| v.as_bool()).unwrap_or(false),
        mcp_url: call("mcp_url", Value::Null).await.ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default(),
        server_error: call("server_error", Value::Null).await.ok().and_then(|v| v.as_str().map(String::from)),
    }
}

#[derive(Debug)]
pub enum Msg {
    /// Something changed in the network (from the core), or a screen asked to reload.
    Changed(Value),
    Reload,
    /// Run a method, then reload; show `toast` on success, the error otherwise.
    Act(&'static str, Value, Option<String>),
    Toast(String),
    Dialog(dialogs::Open),
    InstallUpdate,
    /// The graph: open or fold a group; pick a node to see what it is.
    Toggle(String),
    Pick(String),
    /// Access: open one app, person or AI (its key), or go back to the list.
    Row(Option<String>),
    Filter(String),
    /// The window's width changed: the graph may need laying out again.
    Resized,
}

#[derive(Debug)]
pub enum Cmd {
    Loaded(Box<Data>),
    Done(Result<(), String>, Option<String>),
    Update(Option<String>),
    UpdateFailed(String),
}

pub struct App {
    data: Data,
    /// The question or offer shown right now, so it is shown once.
    shown: Option<String>,
    /// Offers put aside until UUOB is opened again.
    later: Vec<String>,
    update: Option<String>,
    /// The latest change, for a dialog waiting for one (a device joining).
    pub last_event: Value,
    /// The graph's open groups, its picked node and the width it was laid out for.
    open: std::collections::BTreeSet<String>,
    picked: Option<String>,
    width: f64,
    /// What Access shows: one entry (its key), and which kinds in the list.
    row: Option<String>,
    show: String,
}

pub struct Widgets {
    toasts: adw::ToastOverlay,
    banner: adw::Banner,
    update: gtk::Button,
    pages: [adw::PreferencesPage; 3],
    /// The rail's buttons (beside the pages, or under them when narrow): the
    /// screen each opens, the button, its dot.
    navs: Vec<(&'static str, gtk::Button, gtk::Box)>,
    groups: Vec<(usize, adw::PreferencesGroup)>,
    stack: adw::ViewStack,
}

impl relm4::Component for App {
    type Init = ();
    type Input = Msg;
    type Output = ();
    type CommandOutput = Cmd;
    type Root = adw::ApplicationWindow;
    type Widgets = Widgets;

    fn init_root() -> Self::Root {
        adw::ApplicationWindow::builder().title("UUOB").default_width(980).default_height(720).width_request(360).height_request(480).build()
    }

    fn init(_: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        style();
        let stack = adw::ViewStack::new();
        let pages: [adw::PreferencesPage; 3] = std::array::from_fn(|_| adw::PreferencesPage::new());
        for (page, (id, title, icon)) in pages.iter().zip([
            ("network", "Network", "network-workgroup-symbolic"),
            ("access", "Access", "security-high-symbolic"),
            ("settings", "Settings", "emblem-system-symbolic"),
        ]) {
            stack.add_titled_with_icon(page, Some(id), title, icon);
        }

        let header = adw::HeaderBar::new();
        header.set_title_widget(Some(&gtk::Label::new(None)));
        let update = gtk::Button::builder().css_classes(["suggested-action", "pill"]).visible(false).build();
        update.connect_clicked({
            let sender = sender.clone();
            move |_| sender.input(Msg::InstallUpdate)
        });
        header.pack_start(&update);
        let menu = gio::Menu::new();
        menu.append(Some("Quit UUOB"), Some("app.quit"));
        header.pack_end(&gtk::MenuButton::builder().icon_name("open-menu-symbolic").menu_model(&menu).tooltip_text("Menu").build());

        let banner = adw::Banner::new("");
        let toasts = adw::ToastOverlay::new();
        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&banner);
        content.append(&stack);
        stack.set_vexpand(true);
        content.set_hexpand(true);

        // UUOB's rail, as its screens always had: each screen an icon in a soft
        // tile with its name under it, the lock at the bottom. On a phone or
        // a small window it becomes a bar under the pages.
        let mut navs = Vec::new();
        let side = rail(&stack, gtk::Orientation::Vertical, &mut navs);
        let bottom = rail(&stack, gtk::Orientation::Horizontal, &mut navs);
        bottom.set_visible(false);
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        row.append(&side);
        row.append(&content);
        row.set_vexpand(true);
        let all = gtk::Box::new(gtk::Orientation::Vertical, 0);
        all.append(&row);
        all.append(&bottom);
        toasts.set_child(Some(&all));
        let mark = {
            let navs: Vec<(&'static str, gtk::Button)> = navs.iter().map(|(id, b, _)| (*id, b.clone())).collect();
            move |stack: &adw::ViewStack| {
                let on = stack.visible_child_name().unwrap_or_default();
                for (id, b) in &navs {
                    if on == *id { b.add_css_class("on") } else { b.remove_css_class("on") }
                }
            }
        };
        mark(&stack);
        stack.connect_visible_child_name_notify(mark);

        let view = adw::ToolbarView::new();
        view.add_top_bar(&header);
        view.set_content(Some(&toasts));
        root.set_content(Some(&view));

        let narrow = adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 550sp").expect("condition"));
        narrow.add_setter(&side, "visible", Some(&false.to_value()));
        narrow.add_setter(&bottom, "visible", Some(&true.to_value()));
        root.add_breakpoint(narrow);

        // Closing the window keeps UUOB running; opening UUOB again shows it.
        root.set_hide_on_close(true);
        let app = relm4::main_application();
        std::mem::forget(app.hold());
        app.connect_activate({
            let root = root.clone();
            move |_| root.present()
        });
        let quit = gio::SimpleAction::new("quit", None);
        quit.connect_activate(|_, _| relm4::main_application().quit());
        app.add_action(&quit);
        app.set_accels_for_action("app.quit", &["<Ctrl>q"]);
        if std::env::args().any(|a| a == "--hidden") {
            let root = root.clone();
            glib::idle_add_local_once(move || root.set_visible(false));
        }

        // Follow the core's changes for as long as UUOB runs.
        relm4::spawn({
            let sender = sender.clone();
            async move {
                loop {
                    let e = uuob_app::ffi::next_event().await;
                    sender.input(Msg::Changed(serde_json::from_str(&e).unwrap_or_default()));
                }
            }
        });
        sender.oneshot_command(async { Cmd::Update(call("update_check", Value::Null).await.ok().and_then(|v| v.as_str().map(String::from))) });
        sender.input(Msg::Reload);

        root.connect_default_width_notify({
            let sender = sender.clone();
            move |_| sender.input(Msg::Resized)
        });
        let width = graph_width(root.default_width());
        let model = App {
            data: Data::default(),
            shown: None,
            later: Vec::new(),
            update: None,
            last_event: Value::Null,
            open: Default::default(),
            picked: None,
            width,
            row: None,
            show: "all".into(),
        };
        ComponentParts { model, widgets: Widgets { toasts, banner, update, pages, navs, groups: Vec::new(), stack } }
    }

    fn update_with_view(&mut self, w: &mut Widgets, msg: Msg, sender: ComponentSender<Self>, root: &Self::Root) {
        match msg {
            Msg::Changed(e) => {
                let kind = e["type"].as_str().unwrap_or_default().to_string();
                self.last_event = e.clone();
                dialogs::changed(&e);
                match kind.as_str() {
                    "container_changed" => return,
                    "device_joined" => w.toasts.add_toast(adw::Toast::new(&format!("{} joined your network", e["name"].as_str().unwrap_or("A device")))),
                    "removed_from_network" => w.toasts.add_toast(adw::Toast::new("This device was removed from the network")),
                    _ => {}
                }
                sender.input(Msg::Reload);
            }
            Msg::Reload => {
                let (width, open) = (self.width, self.open.iter().cloned().collect());
                sender.oneshot_command(async move { Cmd::Loaded(Box::new(load(width, open).await)) })
            }
            Msg::Toggle(group) => {
                if !self.open.remove(&group) {
                    self.open.insert(group);
                }
                sender.input(Msg::Reload);
            }
            Msg::Pick(id) => {
                self.picked = Some(id);
                self.render(w, &sender);
            }
            Msg::Row(key) => {
                self.row = key;
                self.render(w, &sender);
            }
            Msg::Filter(show) => {
                if self.show != show {
                    self.show = show;
                    self.render(w, &sender);
                }
            }
            Msg::Resized => {
                let width = graph_width(root.default_width());
                if (width - self.width).abs() > 24.0 {
                    self.width = width;
                    sender.input(Msg::Reload);
                }
            }
            Msg::Act(method, params, toast) => sender.oneshot_command(async move { Cmd::Done(call(method, params).await.map(drop), toast) }),
            Msg::Toast(text) => w.toasts.add_toast(adw::Toast::new(&text)),
            Msg::Dialog(open) => dialogs::open(open, root, &sender, &self.data),
            Msg::InstallUpdate => {
                w.update.set_sensitive(false);
                w.update.set_label("Updating…");
                sender.oneshot_command(async {
                    match call("update_install", Value::Null).await {
                        Ok(_) => Cmd::UpdateFailed("Installed. Start UUOB again to use the new version.".into()),
                        Err(e) => Cmd::UpdateFailed(e),
                    }
                });
            }
        }
    }

    fn update_cmd_with_view(&mut self, w: &mut Widgets, cmd: Cmd, sender: ComponentSender<Self>, root: &Self::Root) {
        match cmd {
            Cmd::Loaded(data) => {
                self.data = *data;
                w.banner.set_title(&self.data.server_error.as_ref().map(|e| format!("{e} Close other copies of UUOB and open it again.")).unwrap_or_default());
                w.banner.set_revealed(self.data.server_error.is_some());
                self.render(w, &sender);
                self.ask(root, &sender);
            }
            Cmd::Done(result, toast) => {
                match (result, toast) {
                    (Err(e), _) => w.toasts.add_toast(adw::Toast::new(&e)),
                    (Ok(()), Some(t)) => w.toasts.add_toast(adw::Toast::new(&t)),
                    _ => {}
                }
                sender.input(Msg::Reload);
            }
            Cmd::Update(version) => {
                self.update = version;
                if let Some(v) = &self.update {
                    w.update.set_label(&format!("Update to {v}"));
                    w.update.set_visible(true);
                }
            }
            Cmd::UpdateFailed(e) => {
                w.update.set_sensitive(true);
                w.update.set_label("Try again");
                w.toasts.add_toast(adw::Toast::new(&e));
            }
        }
    }
}

impl App {
    /// Shows the next question from an app, or the next offer from a person, once.
    fn ask(&mut self, root: &adw::ApplicationWindow, sender: &ComponentSender<Self>) {
        if let Some(a) = self.data.asks.first() {
            let id = format!("ask:{}", a["id"].as_str().unwrap_or_default());
            if self.shown.as_deref() != Some(&id) {
                self.shown = Some(id);
                if a["here"].as_bool() == Some(true) {
                    root.present();
                }
                dialogs::open(dialogs::Open::Ask(a.clone()), root, sender, &self.data);
            }
            return;
        }
        let offer = self.data.people.iter().find_map(|p| {
            let s = p["receiving"].as_array()?.iter().find(|s| s["offered"].as_bool() == Some(true) && !self.later.iter().any(|l| l == s["container"].as_str().unwrap_or_default()))?;
            Some((s.clone(), p.clone()))
        });
        if let Some((s, p)) = offer {
            let container = s["container"].as_str().unwrap_or_default().to_string();
            if self.shown.as_deref() != Some(&format!("offer:{container}")) {
                self.shown = Some(format!("offer:{container}"));
                self.later.push(container);
                dialogs::open(dialogs::Open::Offer(s, p), root, sender, &self.data);
            }
        } else {
            self.shown = None;
        }
    }

    /// Rebuilds the three pages from the latest data.
    fn render(&self, w: &mut Widgets, sender: &ComponentSender<Self>) {
        for (page, group) in w.groups.drain(..) {
            w.pages[page].remove(&group);
        }
        let mut add = |page: usize, group: adw::PreferencesGroup| {
            w.pages[page].add(&group);
            w.groups.push((page, group));
        };
        for g in pages::network(&self.data, self.picked.as_deref(), sender) {
            add(0, g);
        }
        for g in pages::access(&self.data, self.row.as_deref(), &self.show, sender) {
            add(1, g);
        }
        for g in pages::settings(&self.data, sender) {
            add(2, g);
        }
        // The rail's dots: devices online (green) or not, and something waiting in Access.
        let total = self.data.status["devices_total"].as_u64().unwrap_or(0);
        let online = self.data.status["devices_online"].as_u64().unwrap_or(0);
        let offers = self.data.people.iter().flat_map(|p| p["receiving"].as_array().cloned().unwrap_or_default()).filter(|s| s["offered"].as_bool() == Some(true)).count();
        for (id, _, dot) in &w.navs {
            let class = match *id {
                "network" if total > 1 => Some(if online > 1 { "ok" } else { "off" }),
                "access" if self.data.asks.len() + offers > 0 => Some("warn"),
                _ => None,
            };
            for c in ["ok", "off", "warn"] {
                dot.remove_css_class(c);
            }
            dot.set_visible(class.is_some());
            if let Some(c) = class {
                dot.add_css_class(c);
            }
        }
        // UUOB_SCREEN=access opens on that page (for screenshots).
        if let Ok(screen) = std::env::var("UUOB_SCREEN") {
            w.stack.set_visible_child_name(&screen);
        }
    }
}

mod pages;

/// The rail: a button per screen (an icon in a tile, its name under it), and
/// the lock at the end when it stands beside the pages.
fn rail(stack: &adw::ViewStack, orientation: gtk::Orientation, navs: &mut Vec<(&'static str, gtk::Button, gtk::Box)>) -> gtk::Box {
    let vertical = orientation == gtk::Orientation::Vertical;
    let bar = gtk::Box::builder().orientation(orientation).spacing(4).css_classes(if vertical { vec!["rail"] } else { vec!["rail", "bottom"] }).build();
    if vertical {
        bar.set_width_request(84);
    } else {
        bar.set_homogeneous(true);
    }
    for (id, title, glyph) in [("network", "Network", "network"), ("access", "Access", "shield"), ("settings", "Settings", "settings")] {
        let tile = gtk::Overlay::builder().css_classes(["navtile"]).halign(gtk::Align::Center).width_request(40).height_request(40).build();
        tile.set_child(Some(&icon(glyph, 20)));
        let dot = gtk::Box::builder().css_classes(["navdot"]).halign(gtk::Align::End).valign(gtk::Align::Start).visible(false).build();
        tile.add_overlay(&dot);
        let inside = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(4).build();
        inside.append(&tile);
        inside.append(&gtk::Label::builder().label(title).css_classes(["navlabel"]).build());
        let b = gtk::Button::builder().child(&inside).css_classes(["nav"]).tooltip_text(title).build();
        let stack = stack.clone();
        b.connect_clicked(move |_| stack.set_visible_child_name(id));
        bar.append(&b);
        navs.push((id, b, dot));
    }
    if vertical {
        let spacer = gtk::Box::builder().vexpand(true).build();
        bar.append(&spacer);
        let lock = icon("lock", 16);
        lock.add_css_class("lock");
        lock.set_tooltip_text(Some("End-to-end encrypted. Your data is stored only on your own devices."));
        bar.append(&lock);
    }
    bar
}

/// How wide the graph is drawn in a window this wide (pages are at most about 600 wide).
fn graph_width(window: i32) -> f64 {
    (window as f64 - 48.0 - if window > 550 { 84.0 } else { 0.0 }).clamp(300.0, 600.0)
}

fn main() {
    let dir = data_dir();
    if let Err(e) = uuob_app::runtime().block_on(uuob_app::start(dir)) {
        eprintln!("UUOB could not start: {e}");
    }
    // UUOB's own flags (--hidden) are not GTK's.
    RelmApp::new("app.uuob").with_args(Vec::new()).run::<App>(());
}
