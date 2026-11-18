//! UUOB's window on Linux: GTK 4 and Libadwaita through Relm4, straight on
//! the Rust core (crates/uuob-app), the same functions the Mac and Android
//! apps reach through UniFFI. Closing the window keeps UUOB running; opening
//! UUOB again brings it back, and "Quit UUOB" in the menu stops it.

mod dialogs;
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

/// Everything the four pages show, loaded together.
#[derive(Default, Clone, Debug)]
pub struct Data {
    pub status: Value,
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

async fn load() -> Data {
    let list = |v: Result<Value, String>| v.ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    Data {
        status: call("status", Value::Null).await.unwrap_or_default(),
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
}

pub struct Widgets {
    toasts: adw::ToastOverlay,
    banner: adw::Banner,
    update: gtk::Button,
    pages: [adw::PreferencesPage; 4],
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
        let pages: [adw::PreferencesPage; 4] = std::array::from_fn(|_| adw::PreferencesPage::new());
        for (page, (id, title, icon)) in pages.iter().zip([
            ("devices", "Devices", "computer-symbolic"),
            ("people", "People", "system-users-symbolic"),
            ("apps", "Apps", "view-app-grid-symbolic"),
            ("ai", "AI access", "starred-symbolic"),
        ]) {
            stack.add_titled_with_icon(page, Some(id), title, icon);
        }

        let header = adw::HeaderBar::new();
        let switcher = adw::ViewSwitcher::builder().stack(&stack).policy(adw::ViewSwitcherPolicy::Wide).build();
        header.set_title_widget(Some(&switcher));
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
        toasts.set_child(Some(&content));

        let bar = adw::ViewSwitcherBar::builder().stack(&stack).build();
        let view = adw::ToolbarView::new();
        view.add_top_bar(&header);
        view.set_content(Some(&toasts));
        view.add_bottom_bar(&bar);
        root.set_content(Some(&view));

        // Narrow (a phone, a small window): the switcher moves to the bottom.
        let narrow = adw::Breakpoint::new(adw::BreakpointCondition::parse("max-width: 550sp").expect("condition"));
        narrow.add_setter(&bar, "reveal", Some(&true.to_value()));
        narrow.add_setter(&header, "title-widget", Some(&None::<gtk::Widget>.to_value()));
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

        let model = App { data: Data::default(), shown: None, later: Vec::new(), update: None, last_event: Value::Null };
        ComponentParts { model, widgets: Widgets { toasts, banner, update, pages, groups: Vec::new(), stack } }
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
            Msg::Reload => sender.oneshot_command(async { Cmd::Loaded(Box::new(load().await)) }),
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

    /// Rebuilds the four pages from the latest data.
    fn render(&self, w: &mut Widgets, sender: &ComponentSender<Self>) {
        for (page, group) in w.groups.drain(..) {
            w.pages[page].remove(&group);
        }
        let mut add = |page: usize, group: adw::PreferencesGroup| {
            w.pages[page].add(&group);
            w.groups.push((page, group));
        };
        for g in pages::devices(&self.data, sender) {
            add(0, g);
        }
        for g in pages::people(&self.data, sender) {
            add(1, g);
        }
        for g in pages::apps(&self.data, sender) {
            add(2, g);
        }
        for g in pages::ai(&self.data, sender) {
            add(3, g);
        }
        // UUOB_SCREEN=people opens on that page (for screenshots).
        if let Ok(screen) = std::env::var("UUOB_SCREEN") {
            w.stack.set_visible_child_name(&screen);
        }
    }
}

mod pages;

fn main() {
    let dir = data_dir();
    if let Err(e) = uuob_app::runtime().block_on(uuob_app::start(dir)) {
        eprintln!("UUOB could not start: {e}");
    }
    // UUOB's own flags (--hidden) are not GTK's.
    RelmApp::new("app.uuob").with_args(Vec::new()).run::<App>(());
}
