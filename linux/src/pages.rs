//! The three pages (Network, Access, Settings), rebuilt from the latest data
//! each time something changes. Access opens one app, person or AI at a time,
//! with what the four pages before them showed for it.

use relm4::adw::prelude::*;
use relm4::{ComponentSender, adw, gtk};
use serde_json::{Value, json};

use crate::dialogs::Open;
use crate::ui::*;
use crate::{App, Data, Msg};

type Sender = ComponentSender<App>;

fn group(title: &str, description: &str) -> adw::PreferencesGroup {
    let g = adw::PreferencesGroup::new();
    if !title.is_empty() {
        g.set_title(title);
    }
    if !description.is_empty() {
        g.set_description(Some(description));
    }
    g
}

fn icon_button(icon: &str, tip: &str, sender: &Sender, msg: impl Fn() -> Msg + 'static) -> gtk::Button {
    let b = gtk::Button::builder().icon_name(icon).tooltip_text(tip).valign(gtk::Align::Center).css_classes(["flat"]).build();
    let sender = sender.clone();
    b.connect_clicked(move |_| sender.input(msg()));
    b
}

fn button(label: &str, classes: &[&str], sender: &Sender, msg: impl Fn() -> Msg + 'static) -> gtk::Button {
    let b = gtk::Button::builder().label(label).css_classes(classes.to_vec()).valign(gtk::Align::Center).build();
    let sender = sender.clone();
    b.connect_clicked(move |_| sender.input(msg()));
    b
}

/// A row of buttons under a page's groups.
fn actions(buttons: &[gtk::Button]) -> adw::PreferencesGroup {
    let row = gtk::Box::builder().spacing(12).halign(gtk::Align::Start).build();
    for b in buttons {
        b.add_css_class("pill");
        row.append(b);
    }
    let g = adw::PreferencesGroup::new();
    g.add(&row);
    g
}

fn platform(p: &str) -> &str {
    match p {
        "linux" => "Linux",
        "macos" => "macOS",
        "windows" => "Windows",
        "android" => "Android",
        "ios" => "iOS",
        p => p,
    }
}

/// Network: the graph, what is picked in it, then your devices.
pub fn network(d: &Data, picked: Option<&str>, sender: &Sender) -> Vec<adw::PreferencesGroup> {
    let mut out = Vec::new();
    let top = group("", "Your devices in the middle, each app on the device it runs on, people at the edge.");
    if d.graph["nodes"].is_array() {
        top.add(&crate::graph::graph(&d.graph, picked, sender));
    }
    out.push(top);
    let nodes = d.graph["nodes"].as_array().cloned().unwrap_or_default();
    let shown = nodes.iter().find(|n| Some(s(n, "id")) == picked).or_else(|| nodes.iter().find(|n| n["self"].as_bool() == Some(true)));
    if let Some(about) = shown.map(|n| &n["about"]) {
        let g = group(s(about, "kind"), "");
        g.add(&adw::ActionRow::builder().title(s(about, "title")).subtitle(s(about, "detail")).css_classes(["property"]).build());
        out.push(g);
    }

    let list = group("Devices", "");
    for dev in &d.devices {
        let me = dev["is_self"].as_bool() == Some(true);
        let online = dev["online"].as_bool() == Some(true);
        let status = if me { "This device" } else if online { "Online" } else { "Offline" };
        let keeps = if dev["keeper"].as_bool() == Some(true) { " · keeps everything" } else { "" };
        let row = adw::ActionRow::builder().title(s(dev, "name")).subtitle(format!("{status} · {}{keeps}", platform(s(dev, "platform")))).build();
        let phone = matches!(s(dev, "platform"), "android" | "ios");
        row.add_prefix(&gtk::Image::builder().icon_name(if phone { "phone-symbolic" } else { "computer-symbolic" }).pixel_size(28).build());
        if me {
            let name = s(dev, "name").to_string();
            row.add_suffix(&icon_button("document-edit-symbolic", "Rename", sender, move || Msg::Dialog(Open::Rename(name.clone()))));
        } else {
            let dev = dev.clone();
            row.add_suffix(&icon_button("user-trash-symbolic", "Remove from network", sender, move || Msg::Dialog(Open::RemoveDevice(dev.clone()))));
        }
        list.add(&row);
    }
    out.push(list);
    let add = button("Add a Device…", &["suggested-action"], sender, || Msg::Dialog(Open::AddDevice));
    out.push(if d.devices.len() <= 1 { actions(&[add, button("Join a Network…", &[], sender, || Msg::Dialog(Open::Join))]) } else { actions(&[add]) });
    out
}

/// Settings: your name, this computer, storage, updates, leaving the network.
pub fn settings(d: &Data, sender: &Sender) -> Vec<adw::PreferencesGroup> {
    let mut out = Vec::new();
    let me = s(&d.status["network"], "name").to_string();
    let you = group("You", "");
    let row = adw::ActionRow::builder().title(if me.is_empty() { "No name yet" } else { &me }).subtitle("The name people you link with see").build();
    row.add_prefix(&avatar(&me, 40));
    let current = me.clone();
    row.add_suffix(&icon_button("document-edit-symbolic", "Change name", sender, move || Msg::Dialog(Open::Name(current.clone(), None))));
    you.add(&row);
    out.push(you);

    let this = group("This computer", "");
    let row = adw::SwitchRow::builder().title("Start at login").subtitle("UUOB starts in the background when you log in, so your apps and devices stay in sync.").active(d.autostart).build();
    let s2 = sender.clone();
    row.connect_active_notify(move |r| s2.input(Msg::Act("set_autostart", json!({ "on": r.is_active() }), None)));
    this.add(&row);
    let ok = d.server_error.is_none();
    let row = adw::ActionRow::builder()
        .title("Apps can connect")
        .subtitle(d.server_error.clone().unwrap_or_else(|| format!("Only from this computer, and only the ones you allow in Access. AI server: {}", d.mcp_url)))
        .build();
    row.add_suffix(&gtk::Label::builder().label(if ok { "Running" } else { "Stopped" }).css_classes([if ok { "success" } else { "error" }]).build());
    this.add(&row);
    out.push(this);

    let alone = d.devices.len() <= 1;
    if !alone {
        let keepers = d.devices.iter().filter(|v| v["keeper"].as_bool() == Some(true)).count();
        let copies = d.status["network"]["copies"].as_u64().unwrap_or(1);
        let note = if keepers > 0 {
            let who = if copies == 1 { "a storage device has".to_string() } else { format!("{copies} storage devices have") };
            format!("Apps with large data, like photos, can keep less on your other devices. Nothing is removed from a device until {who} it.")
        } else {
            "Turn this on for a device with plenty of space that is usually on. Apps with large data, like photos, can then keep less on your other devices.".into()
        };
        let storage = group("Storage devices", &note);
        for dev in &d.devices {
            let keeper = dev["keeper"].as_bool() == Some(true);
            let row = adw::SwitchRow::builder().title(s(dev, "name")).subtitle(if keeper { "Keeps everything" } else { "Keeps what its apps ask for" }).active(keeper).build();
            let id = s(dev, "id").to_string();
            let sender = sender.clone();
            row.connect_active_notify(move |r| sender.input(Msg::Act("set_keeper", json!({ "device": id, "on": r.is_active() }), None)));
            storage.add(&row);
        }
        if keepers > 0 {
            let row = adw::ActionRow::builder().title("Copies").subtitle("Storage devices that must have something before another device frees its own copy.").build();
            let sender = sender.clone();
            row.add_suffix(&choice(&[("1", "1"), ("2", "2")], &copies.to_string(), move |n| {
                sender.input(Msg::Act("set_copies", json!({ "copies": n.parse::<u64>().unwrap_or(1) }), None))
            }));
            storage.add(&row);
        }
        out.push(storage);
    }

    let updates = group("Updates", "");
    updates.add(&adw::ActionRow::builder().title(format!("UUOB {}", d.version)).subtitle("When a new version is out, an Update button appears at the top.").build());
    out.push(updates);

    if !alone {
        let net = group("Network", "");
        let row = adw::ActionRow::builder().title("Leave this network").subtitle("This computer stops syncing with your other devices.").build();
        row.add_suffix(&button("Leave…", &["destructive-action"], sender, || Msg::Dialog(Open::Leave)));
        net.add(&row);
        out.push(net);
    }
    out
}

/// Access: every app, person and AI, filtered; or the one that is open.
pub fn access(d: &Data, open: Option<&str>, show: &str, sender: &Sender) -> Vec<adw::PreferencesGroup> {
    if let Some(key) = open {
        let (kind, id) = key.split_once(':').unwrap_or_default();
        let detail = match kind {
            "app" => apps(d, id, sender),
            "person" => people(d, id, sender),
            _ => ai(d, id, sender),
        };
        // Gone (disconnected, removed): back to the list.
        if !detail.is_empty() {
            let back = gtk::Button::builder().icon_name("go-previous-symbolic").label("Access").css_classes(["flat"]).halign(gtk::Align::Start).build();
            back.set_child(Some(&adw::ButtonContent::builder().icon_name("go-previous-symbolic").label("Access").build()));
            let s2 = sender.clone();
            back.connect_clicked(move |_| s2.input(Msg::Row(None)));
            let top = adw::PreferencesGroup::new();
            top.add(&back);
            return std::iter::once(top).chain(detail).collect();
        }
    }
    let list = group("", "Everything that can reach your data, and what each one can read or write.");
    let head = gtk::Box::builder().spacing(12).build();
    let s2 = sender.clone();
    head.append(&choice(&[("all", "All"), ("app", "Apps"), ("person", "People"), ("ai", "AI")], show, move |v| s2.input(Msg::Filter(v))));
    let add = button("Add…", &["suggested-action", "pill"], sender, || Msg::Dialog(Open::Add));
    add.set_hexpand(true);
    add.set_halign(gtk::Align::End);
    head.append(&add);
    head.set_margin_bottom(12);
    list.add(&head);
    let rows: Vec<&Value> = d.access.iter().filter(|r| show == "all" || s(r, "kind") == show).collect();
    if d.access.is_empty() {
        list.add(&adw::StatusPage::builder().icon_name("security-high-symbolic").title("Nothing has access yet")
            .description("Install an app that works with UUOB and open it: it asks to connect. You can also link with a person or connect an AI with Add.").build());
    }
    let items = gtk::ListBox::builder().css_classes(["boxed-list"]).selection_mode(gtk::SelectionMode::None).build();
    for r in rows {
        let kind = s(r, "kind");
        let badge = match kind { "app" => "App", "person" => "Person", _ => "AI" };
        let mut sub = s(r, "summary").to_string();
        if !s(r, "where").is_empty() && kind != "person" {
            sub += &format!("\n{}", s(r, "where"));
        }
        let row = adw::ActionRow::builder().title(format!("{} · {badge}", s(r, "name"))).subtitle(sub).activatable(true).build();
        row.add_prefix(&match kind {
            "app" => app_tile(s(r, "id"), 40),
            "person" => avatar(s(r, "name"), 40),
            _ => {
                let i = gtk::Image::builder().icon_name("starred-symbolic").pixel_size(20).width_request(40).height_request(40).css_classes(["tile", "purple"]).build();
                i.upcast()
            }
        });
        row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
        let (s2, key) = (sender.clone(), s(r, "key").to_string());
        row.connect_activated(move |_| s2.input(Msg::Row(Some(key.clone()))));
        items.append(&row);
    }
    list.add(&items);
    vec![list]
}

pub fn people(d: &Data, only: &str, sender: &Sender) -> Vec<adw::PreferencesGroup> {
    let mut out = Vec::new();
    for p in d.people.iter().filter(|p| s(p, "id") == only) {
        let name = s(p, "name").to_string();
        let g = group("", "");
        let online = p["online"].as_bool() == Some(true);
        let head = adw::ActionRow::builder().title(&name).subtitle(if online { "Online" } else { "Offline" }).build();
        head.add_prefix(&avatar(&name, 40));
        let person = p.clone();
        head.add_suffix(&icon_button("user-trash-symbolic", &format!("Remove {name}"), sender, move || Msg::Dialog(Open::RemovePerson(person.clone()))));
        g.add(&head);
        let receiving = p["receiving"].as_array().cloned().unwrap_or_default();
        let sharing = p["sharing"].as_array().cloned().unwrap_or_default();
        for r in &receiving {
            let can = if s(r, "access") == "write" { "Can edit" } else { "Can view" };
            let row = adw::ActionRow::builder().title(s(r, "name")).subtitle(format!("{name} shares · {} · {can}", s(r, "app_name"))).build();
            row.add_prefix(&app_tile(s(r, "app"), 32));
            let c = s(r, "container").to_string();
            if r["offered"].as_bool() == Some(true) {
                let c2 = c.clone();
                row.add_suffix(&button("Decline", &["flat"], sender, move || Msg::Act("delete_container", json!({ "id": c2 }), None)));
                let toast = format!("“{}” was added to {}", s(r, "name"), s(r, "app_name"));
                row.add_suffix(&button("Accept", &["suggested-action"], sender, move || Msg::Act("accept_share", json!({ "container": c }), Some(toast.clone()))));
            } else {
                let toast = format!("You left “{}”", s(r, "name"));
                row.add_suffix(&button("Leave", &["flat", "error"], sender, move || Msg::Act("delete_container", json!({ "id": c }), Some(toast.clone()))));
            }
            g.add(&row);
        }
        for sh in &sharing {
            let row = adw::ActionRow::builder().title(s(sh, "name")).subtitle(format!("You share · {}", s(sh, "app_name"))).build();
            row.add_prefix(&app_tile(s(sh, "app"), 32));
            let (c, pid) = (s(sh, "container").to_string(), s(p, "id").to_string());
            let sender2 = sender.clone();
            let (c2, pid2) = (c.clone(), pid.clone());
            row.add_suffix(&choice(&[("read", "Can view"), ("write", "Can edit")], s(sh, "access"), move |v| {
                sender2.input(Msg::Act("share_with", json!({ "container": c2, "person": pid2, "access": v }), None))
            }));
            let toast = format!("Stopped sharing “{}” with {name}", s(sh, "name"));
            row.add_suffix(&icon_button("window-close-symbolic", "Stop sharing", sender, move || {
                Msg::Act("share_with", json!({ "container": c, "person": pid, "access": null }), Some(toast.clone()))
            }));
            g.add(&row);
        }
        if receiving.is_empty() && sharing.is_empty() {
            g.add(&adw::ActionRow::builder().title("Nothing shared yet").subtitle("Open a list or a calendar in its app and choose Share.").build());
        }
        out.push(g);
    }
    out
}

pub fn apps(d: &Data, only: &str, sender: &Sender) -> Vec<adw::PreferencesGroup> {
    let mut out = Vec::new();
    let keepers = d.devices.iter().filter(|v| v["keeper"].as_bool() == Some(true)).count() as u64;
    let needed = d.status["network"]["copies"].as_u64().unwrap_or(1);
    let mut ids: Vec<&str> = Vec::new();
    for c in &d.containers {
        if s(c, "owner_app") == only && !ids.contains(&s(c, "owner_app")) {
            ids.push(s(c, "owner_app"));
        }
    }
    for id in ids {
        let cs: Vec<&Value> = d.containers.iter().filter(|c| s(c, "owner_app") == id).collect();
        let name = d.apps[id]["name"].as_str().unwrap_or(id).to_string();
        let installed = d.apps[id]["installed"].as_bool() == Some(true);
        let sum = |k: &str| cs.iter().map(|c| c[k].as_u64().unwrap_or(0)).sum::<u64>();
        let (records, files, here) = (sum("records"), sum("files"), sum("files_here"));
        let bytes = sum("size_bytes") + sum("file_bytes");
        let safe = cs.iter().filter(|c| c["here"].as_bool() == Some(true) && c["records"].as_u64().unwrap_or(0) + c["files"].as_u64().unwrap_or(0) > 0).filter_map(|c| c["copies"].as_u64()).min();
        let mut notes = vec![format!("{} · {} · {}", plural(records, "item", "items"), size(bytes), if installed { "Connected" } else { "Not connected, data kept" })];
        if files > 0 {
            notes.push(if here == files { plural(files, "file", "files") } else { format!("{here} of {files} files on this device") });
        }
        if let (true, Some(safe)) = (keepers > 0, safe) {
            notes.push(if safe >= needed { format!("On {}", plural(safe, "storage device", "storage devices")) } else { "Waiting for a storage device".into() });
        }
        let g = group(&name, "");
        let head = adw::ActionRow::builder().title(&name).subtitle(notes.join("\n")).build();
        head.add_prefix(&app_tile(id, 40));
        let (cs_ids, gname) = (cs.iter().map(|c| (s(c, "id").to_string(), s(c, "name").to_string())).collect::<Vec<_>>(), name.clone());
        let app_id = id.to_string();
        head.add_suffix(&icon_button("edit-copy-symbolic", "Copy as JSON", sender, move || Msg::Dialog(Open::Export(app_id.clone(), gname.clone(), cs_ids.clone()))));
        g.add(&head);
        for part in cs.iter().filter(|c| s(c, "id") != id) {
            let mut sub = if part["here"].as_bool() == Some(true) { plural(part["records"].as_u64().unwrap_or(0), "item", "items") } else { "Not on this device".into() };
            if let Some(from) = part["from"].as_object() {
                sub += &format!(" · From {}", from.get("name").and_then(|n| n.as_str()).unwrap_or_default());
            } else if let Some(with) = part["shared_with"].as_array().filter(|w| !w.is_empty()) {
                sub += &format!(" · Shared with {}", with.iter().map(|p| s(p, "name")).collect::<Vec<_>>().join(", "));
            }
            g.add(&adw::ActionRow::builder().title(s(part, "name")).subtitle(sub).build());
        }
        for a in cs.first().and_then(|c| c["access"].as_array()).into_iter().flatten() {
            let client = s(a, "client").to_string();
            let owner = client == format!("app:{id}");
            let row = adw::ActionRow::builder().title(s(a, "client_name")).subtitle(format!("{}{}", if s(a, "access") == "write" { "Read & write" } else { "Read only" }, if owner { " · owner" } else { "" })).build();
            if !owner {
                let (app_id, toast) = (id.to_string(), format!("{} can no longer access {name}", s(a, "client_name")));
                row.add_suffix(&icon_button("window-close-symbolic", "Remove access", sender, move || {
                    Msg::Act("set_grant", json!({ "client": client, "app": app_id, "access": null }), Some(toast.clone()))
                }));
            }
            g.add(&row);
        }
        if let Some(clients) = d.clients.as_object() {
            for (cid, copy) in clients.iter().filter(|(_, c)| s(c, "app") == id && !c["endpoint"].is_null()) {
                let device = copy["device"].as_str().unwrap_or("Another device").to_string();
                let row = adw::ActionRow::builder().title(&device).subtitle(format!("Without UUOB · connected {}", day(copy["created_at"].as_f64().unwrap_or(0.0)))).build();
                let (cid, toast) = (cid.clone(), format!("{name} on {device} was disconnected"));
                row.add_suffix(&icon_button("window-close-symbolic", "Disconnect", sender, move || Msg::Act("delete_client", json!({ "id": cid }), Some(toast.clone()))));
                g.add(&row);
            }
        }
        let (app_id, gname) = (id.to_string(), name.clone());
        let label = if installed { format!("Disconnect {name}…") } else { "Delete Data…".into() };
        let b = button(&label, &["destructive-action"], sender, move || Msg::Dialog(Open::Uninstall(app_id.clone(), gname.clone(), installed, records)));
        b.set_halign(gtk::Align::End);
        b.set_margin_top(8);
        g.add(&b);
        out.push(g);
    }
    out
}

pub fn ai(d: &Data, only: &str, sender: &Sender) -> Vec<adw::PreferencesGroup> {
    let mut out = vec![];
    let clients: Vec<(&String, &Value)> = d.clients.as_object().map(|m| m.iter().filter(|(id, c)| c["app"].is_null() && id.as_str() == only).collect()).unwrap_or_default();
    let mut apps: Vec<&Value> = Vec::new();
    for c in &d.containers {
        if !apps.iter().any(|a| s(a, "owner_app") == s(c, "owner_app")) {
            apps.push(c);
        }
    }
    for (cid, c) in clients {
        let g = group(s(c, "name"), &format!("Connected {}", day(c["created_at"].as_f64().unwrap_or(0.0))));
        let (cid2, name) = (cid.clone(), s(c, "name").to_string());
        g.set_header_suffix(Some(&icon_button("user-trash-symbolic", "Disconnect", sender, move || Msg::Dialog(Open::RemoveAi(cid2.clone(), name.clone())))));
        for k in &apps {
            let app = s(k, "owner_app").to_string();
            let title = d.containers.iter().find(|c| s(c, "id") == app).map(|c| s(c, "name")).unwrap_or(app.as_str()).to_string();
            let level = k["access"].as_array().into_iter().flatten().find(|a| s(a, "client") == format!("ai:{cid}")).map(|a| s(a, "access")).unwrap_or("none");
            let row = adw::ActionRow::builder().title(&title).build();
            let (sender2, cid3) = (sender.clone(), cid.clone());
            row.add_suffix(&choice(&[("none", "No access"), ("read", "Read"), ("write", "Read & write")], level, move |v| {
                let access = if v == "none" { Value::Null } else { Value::String(v) };
                sender2.input(Msg::Act("set_grant", json!({ "client": format!("ai:{cid3}"), "app": app, "access": access }), None))
            }));
            g.add(&row);
        }
        out.push(g);
    }
    out
}
