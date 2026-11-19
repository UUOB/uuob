//! Dialogs: codes for linking, questions from apps, the share sheet, confirmations.

use std::cell::RefCell;
use std::rc::Rc;

use relm4::adw::prelude::*;
use relm4::{ComponentSender, adw, gtk, gtk::glib};
use serde_json::{Value, json};

use crate::ui::*;
use crate::{App, Data, Msg, call};

type Sender = ComponentSender<App>;

#[derive(Debug, Clone)]
pub enum Open {
    /// Access → Add: an app, a person or an AI?
    Add,
    /// A person: show your code or enter theirs.
    PersonHow,
    AddDevice,
    Join,
    Leave,
    Rename(String),
    RemoveDevice(Value),
    /// Your name (with what it was), then maybe what to open next.
    Name(String, Option<Box<Open>>),
    AddPerson,
    Meet,
    RemovePerson(Value),
    AppCode,
    Export(String, String, Vec<(String, String)>),
    Uninstall(String, String, bool, u64),
    ConnectAi,
    RemoveAi(String, String),
    Ask(Value),
    Offer(Value, Value),
}

thread_local! {
    /// Dialogs waiting for a change (a device joining); each says whether it still waits.
    static WAITING: RefCell<Vec<Box<dyn FnMut(&Value) -> bool>>> = RefCell::new(Vec::new());
}

/// A change from the core, for the dialogs waiting for one.
pub fn changed(e: &Value) {
    let mut waiting = WAITING.with(|w| std::mem::take(&mut *w.borrow_mut()));
    waiting.retain_mut(|f| f(e));
    WAITING.with(|w| w.borrow_mut().extend(waiting));
}

fn wait(f: impl FnMut(&Value) -> bool + 'static) {
    WAITING.with(|w| w.borrow_mut().push(Box::new(f)));
}

fn alert(heading: &str, body: &str) -> adw::AlertDialog {
    adw::AlertDialog::new(Some(heading), Some(body))
}

/// Cancel and one destructive answer; `then` runs on that answer.
fn confirm(root: &adw::ApplicationWindow, heading: &str, body: &str, action: &str, then: impl Fn() + 'static) {
    let d = alert(heading, body);
    d.add_responses(&[("cancel", "Cancel"), ("go", action)]);
    d.set_response_appearance("go", adw::ResponseAppearance::Destructive);
    d.set_default_response(Some("cancel"));
    d.set_close_response("cancel");
    d.connect_response(None, move |_, r| {
        if r == "go" {
            then()
        }
    });
    d.present(Some(root));
}

/// A question with a text field; `then` gets the text.
fn ask_text(root: &adw::ApplicationWindow, heading: &str, body: &str, placeholder: &str, current: &str, action: &str, valid: fn(&str) -> bool, then: impl Fn(String) + 'static) {
    let d = alert(heading, body);
    let entry = gtk::Entry::builder().placeholder_text(placeholder).text(current).activates_default(true).build();
    d.set_extra_child(Some(&entry));
    d.add_responses(&[("cancel", "Cancel"), ("go", action)]);
    d.set_response_appearance("go", adw::ResponseAppearance::Suggested);
    d.set_default_response(Some("go"));
    d.set_close_response("cancel");
    d.set_response_enabled("go", valid(current));
    entry.connect_changed({
        let d = d.clone();
        move |e| d.set_response_enabled("go", valid(&e.text()))
    });
    d.connect_response(None, move |_, r| {
        if r == "go" {
            then(entry.text().to_string())
        }
    });
    d.present(Some(root));
}

fn not_blank(s: &str) -> bool {
    !s.trim().is_empty()
}

/// A dialog with a title bar; `content` is replaced as it goes (a code, then "joined").
fn sheet(root: &adw::ApplicationWindow, title: &str) -> (adw::Dialog, gtk::Box) {
    let body = gtk::Box::builder().orientation(gtk::Orientation::Vertical).spacing(18).margin_top(12).margin_bottom(24).margin_start(24).margin_end(24).build();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    view.set_content(Some(&body));
    let d = adw::Dialog::builder().title(title).content_width(460).child(&view).build();
    d.present(Some(root));
    (d, body)
}

fn clear(b: &gtk::Box) {
    while let Some(c) = b.first_child() {
        b.remove(&c);
    }
}

fn text(t: &str, classes: &[&str]) -> gtk::Label {
    gtk::Label::builder().label(t).wrap(true).justify(gtk::Justification::Center).css_classes(classes.to_vec()).build()
}

fn spinner(t: &str) -> gtk::Box {
    let b = gtk::Box::builder().spacing(8).halign(gtk::Align::Center).build();
    b.append(&gtk::Spinner::builder().spinning(true).build());
    b.append(&text(t, &["dim-label"]));
    b
}

/// An invite code (its words, maybe a QR code) and a countdown, into `body`.
fn show_code(body: &gtk::Box, invite: &Value, waiting: &str, with_qr: bool) {
    let words = gtk::FlowBox::builder().selection_mode(gtk::SelectionMode::None).halign(gtk::Align::Center).max_children_per_line(6).build();
    for (i, w) in s(invite, "code").split('-').enumerate() {
        words.append(&gtk::Label::builder().label(w).selectable(true).css_classes(if i == 0 { vec!["word", "number"] } else { vec!["word"] }).build());
    }
    body.append(&words);
    if with_qr {
        body.append(&qr(s(invite, "code"), 170));
    }
    let left = text("", &["dim-label"]);
    let row = gtk::Box::builder().spacing(8).halign(gtk::Align::Center).build();
    row.append(&gtk::Spinner::builder().spinning(true).build());
    row.append(&left);
    body.append(&row);
    let expires = invite["expires_at"].as_f64().unwrap_or(0.0) / 1000.0;
    let waiting = waiting.to_string();
    let tick = move || {
        let now = glib::real_time() as f64 / 1e6;
        let secs = (expires - now).max(0.0) as u64;
        left.set_label(&format!("{waiting} Code expires in {}:{:02}", secs / 60, secs % 60));
    };
    tick();
    let row = row.downgrade();
    glib::timeout_add_seconds_local(1, move || {
        if row.upgrade().and_then(|r| r.root()).is_none() {
            return glib::ControlFlow::Break;
        }
        tick();
        glib::ControlFlow::Continue
    });
}

fn success(body: &gtk::Box, dialog: &adw::Dialog, title: &str, description: &str) {
    clear(body);
    body.append(&adw::StatusPage::builder().icon_name("emblem-ok-symbolic").title(title).description(description).build());
    let done = gtk::Button::builder().label("Done").css_classes(["suggested-action", "pill"]).halign(gtk::Align::Center).build();
    let dialog = dialog.clone();
    done.connect_clicked(move |_| {
        dialog.close();
    });
    body.append(&done);
}

fn copy_text(t: &str) {
    if let Some(display) = gtk::gdk::Display::default() {
        display.clipboard().set_text(t);
    }
}

pub fn open(what: Open, root: &adw::ApplicationWindow, sender: &Sender, data: &Data) {
    let sender = sender.clone();
    let act = {
        let sender = sender.clone();
        move |m: &'static str, p: Value, t: Option<String>| sender.input(Msg::Act(m, p, t))
    };
    match what {
        Open::Add => {
            let d = alert("What are you adding?", "An app on one of your devices, someone with their own UUOB, or an AI assistant. Each sees only what you allow.");
            d.add_responses(&[("cancel", "Cancel"), ("app", "An App"), ("person", "A Person"), ("ai", "An AI")]);
            d.set_close_response("cancel");
            let sender = sender.clone();
            d.connect_response(None, move |_, r| match r {
                "app" => sender.input(Msg::Dialog(Open::AppCode)),
                "person" => sender.input(Msg::Dialog(Open::PersonHow)),
                "ai" => sender.input(Msg::Dialog(Open::ConnectAi)),
                _ => {}
            });
            d.present(Some(root));
        }
        Open::PersonHow => {
            let named = !s(&data.status["network"], "name").is_empty();
            let d = alert("Add a person", "Link with someone who has their own UUOB. Then share a list or a calendar with them from its app. They see only what you share.");
            d.add_responses(&[("cancel", "Cancel"), ("enter", "Enter Their Code"), ("show", "Show My Code")]);
            d.set_response_appearance("show", adw::ResponseAppearance::Suggested);
            d.set_close_response("cancel");
            let sender = sender.clone();
            d.connect_response(None, move |_, r| {
                let next = match r {
                    "show" => Open::AddPerson,
                    "enter" => Open::Meet,
                    _ => return,
                };
                // Linking needs a name for the other person to see.
                sender.input(Msg::Dialog(if named { next } else { Open::Name(String::new(), Some(Box::new(next))) }));
            });
            d.present(Some(root));
        }
        Open::Leave => confirm(root, "Leave this network?", "This device stops syncing with your other devices. Everything already on it stays here.", "Leave",
            move || act("leave_network", Value::Null, Some("This device now has its own network".into()))),
        Open::RemoveDevice(d) => {
            let (id, name) = (s(&d, "id").to_string(), s(&d, "name").to_string());
            confirm(root, &format!("Remove “{name}”?"), "It stops receiving updates right away. Data it already has stays on it. To use it again, add it back with a new code.", "Remove",
                move || act("remove_device", json!({ "id": id }), Some(format!("{name} was removed"))));
        }
        Open::RemovePerson(p) => {
            let (id, name) = (s(&p, "id").to_string(), s(&p, "name").to_string());
            confirm(root, &format!("Remove {name}?"), &format!("Everything you share with each other stops. What {name} shared disappears from your devices, and what you shared disappears from theirs the next time they connect."), "Remove",
                move || act("remove_person", json!({ "id": id }), Some(format!("{name} was removed"))));
        }
        Open::RemoveAi(id, name) => confirm(root, &format!("Disconnect “{name}”?"), "Its key stops working immediately on all your devices.", "Disconnect",
            move || act("delete_client", json!({ "id": id }), None)),
        Open::Rename(current) => ask_text(root, "This device’s name", "", "Name", &current, "Save", not_blank,
            move |name| act("set_device_name", json!({ "name": name }), None)),
        Open::Name(current, then) => ask_text(root, "Your name", "This is how you appear to people you share with.", "e.g. Alex", &current, if then.is_some() { "Continue" } else { "Save" }, not_blank,
            move |name| {
                let (sender, then) = (sender.clone(), then.clone());
                relm4::spawn_local(async move {
                    match call("set_name", json!({ "name": name })).await {
                        Ok(_) => {
                            sender.input(Msg::Reload);
                            if let Some(next) = then {
                                sender.input(Msg::Dialog(*next));
                            }
                        }
                        Err(e) => sender.input(Msg::Toast(e)),
                    }
                });
            }),
        Open::Join => ask_text(root, "Join a network", "On a device that’s already set up, open Network → Add a device and type the code it shows. Anything already on this device is kept and merged into the network.",
            "e.g. 42 amber river falcon", "", "Join", complete, move |code| {
                let sender = sender.clone();
                relm4::spawn_local(async move {
                    match call("join", json!({ "code": code })).await {
                        Ok(_) => sender.input(Msg::Toast("Linked! Your data will sync in a moment.".into())),
                        Err(e) => sender.input(Msg::Toast(e)),
                    }
                    sender.input(Msg::Reload);
                });
            }),
        Open::Meet => ask_text(root, "Enter a code", "Ask the other person to open Access → Add → A person → Show my code in their UUOB (or to share something with someone new), and type the code they see. Your devices and data stay your own.",
            "e.g. 42 amber river falcon", "", "Link", complete, move |code| {
                let sender = sender.clone();
                relm4::spawn_local(async move {
                    match call("meet", json!({ "code": code })).await {
                        Ok(_) => sender.input(Msg::Toast("Linked".into())),
                        Err(e) => sender.input(Msg::Toast(e)),
                    }
                    sender.input(Msg::Reload);
                });
            }),
        Open::AddDevice => code_sheet(root, sender, "Add a device", "On the other device, open UUOB, choose Join a network and enter this code.", "create_invite", Value::Null, "Waiting for your device…", Joined::Device),
        Open::AddPerson => code_sheet(root, sender, "Add a person", "They open UUOB on their device, go to Access → Add → A person → Enter their code, and type this. This links the two of you; it doesn’t share anything.", "create_person_invite", Value::Null, "Waiting for them…", Joined::Person(data.people.iter().map(|p| s(p, "id").to_string()).collect())),
        Open::AppCode => code_sheet(root, sender, "Connect with a code", "Type this code into the app. It also works in an app on a device that has no UUOB.", "create_app_code", Value::Null, "Works once.", Joined::Nothing),
        Open::Export(_, name, containers) => drop(relm4::spawn_local(async move {
            let mut all = serde_json::Map::new();
            for (i, (id, part)) in containers.iter().enumerate() {
                all.insert(if i == 0 { name.clone() } else { part.clone() }, call("export", json!({ "container": id })).await.unwrap_or_default());
            }
            copy_text(&serde_json::to_string_pretty(&all).unwrap_or_default());
            sender.input(Msg::Toast(format!("Copied all {name} data as JSON")));
        })),
        Open::Uninstall(id, name, installed, records) => {
            if !installed {
                return confirm(root, &format!("Delete all {name} data?"), &format!("{} permanently erased from every device. This can’t be undone.", plural(records, "item is", "items are")), "Delete",
                    move || act("uninstall_app", json!({ "id": id, "delete_data": true }), Some(format!("{name} data deleted on all devices"))));
            }
            let d = alert(&format!("Disconnect {name}?"), &format!("{name} stops working with your network on all your devices. Keep its {} to connect it again later, or erase them from every device.", plural(records, "item", "items")));
            d.add_responses(&[("cancel", "Cancel"), ("keep", "Keep the Data"), ("delete", "Delete the Data Too")]);
            d.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
            d.set_close_response("cancel");
            d.connect_response(None, move |_, r| match r {
                "keep" => act("uninstall_app", json!({ "id": id, "delete_data": false }), Some(format!("{name} was disconnected; its data is kept"))),
                "delete" => act("uninstall_app", json!({ "id": id, "delete_data": true }), Some(format!("{name} was disconnected and its data deleted"))),
                _ => {}
            });
            d.present(Some(root));
        }
        Open::ConnectAi => {
            let (root2, url) = (root.clone(), data.mcp_url.clone());
            ask_text(root, "Connect an AI", "Give it a name you’ll recognize. It starts with no access.", "e.g. Claude on my laptop", "", "Create", not_blank, move |name| {
                let (sender, root, url) = (sender.clone(), root2.clone(), url.clone());
                relm4::spawn_local(async move {
                    match call("create_client", json!({ "name": name })).await {
                        Ok(r) => {
                            ai_ready(&root, &name, &url, s(&r, "token"), &sender);
                            sender.input(Msg::Reload);
                        }
                        Err(e) => sender.input(Msg::Toast(e)),
                    }
                });
            });
        }
        Open::Ask(a) if s(&a, "kind") == "share" => share_sheet(root, sender, a, data),
        Open::Ask(a) => {
            let connect = s(&a, "kind") == "connect";
            let (heading, body) = if connect {
                (format!("“{}” wants to use your network", s(&a, "name")), "It gets its own place for its data, synced to all your devices. It can’t see anything else unless you allow it.".to_string())
            } else {
                let what = if s(&a, "access") == "write" { "read and change" } else { "read" };
                (format!("“{}” wants to {what} your {} data", s(&a, "app_name"), s(&a, "container_name")), "You can change this any time in Access.".to_string())
            };
            let d = alert(&heading, &body);
            d.add_responses(&[("deny", "Don’t Allow"), ("allow", "Allow")]);
            d.set_response_appearance("allow", adw::ResponseAppearance::Suggested);
            d.set_default_response(Some("allow"));
            d.set_close_response("deny");
            let (id, name) = (s(&a, "id").to_string(), s(&a, "name").to_string());
            d.connect_response(None, move |_, r| {
                let allow = r == "allow";
                act("answer", json!({ "id": id, "allow": allow }), (allow && connect).then(|| format!("{name} is connected")));
            });
            d.present(Some(root));
        }
        Open::Offer(o, p) => {
            let can = if s(&o, "access") == "write" { "You can both change it." } else { "You can view it, but not change it." };
            let d = alert(&format!("{} shared “{}” with you", s(&p, "name"), s(&o, "name")), &format!("It will appear in {} on all your devices. {can}", s(&o, "app_name")));
            d.add_responses(&[("later", "Later"), ("decline", "Decline"), ("accept", "Accept")]);
            d.set_response_appearance("accept", adw::ResponseAppearance::Suggested);
            d.set_close_response("later");
            let (c, toast) = (s(&o, "container").to_string(), format!("“{}” was added to {}", s(&o, "name"), s(&o, "app_name")));
            d.connect_response(None, move |_, r| match r {
                "accept" => act("accept_share", json!({ "container": c }), Some(toast.clone())),
                "decline" => act("delete_container", json!({ "id": c }), None),
                _ => {}
            });
            d.present(Some(root));
        }
    }
}

/// What a code sheet waits for.
enum Joined {
    Device,
    /// Someone new in the people list (those who were there before).
    Person(Vec<String>),
    Nothing,
}

/// Creates a code with `method` and shows it until it is used (or the sheet closed).
#[allow(clippy::too_many_arguments)]
fn code_sheet(root: &adw::ApplicationWindow, sender: Sender, title: &str, lead: &str, method: &'static str, params: Value, waiting: &str, joined: Joined) {
    let (dialog, body) = sheet(root, title);
    body.append(&text(lead, &[]));
    body.append(&spinner("Creating a code…"));
    let code: Rc<RefCell<Option<String>>> = Rc::default();
    let used = Rc::new(std::cell::Cell::new(false));
    // Closed before the code was used: it stops working.
    dialog.connect_closed({
        let (code, used, sender) = (code.clone(), used.clone(), sender.clone());
        move |_| {
            if let (Some(c), false) = (code.borrow().clone(), used.get()) {
                relm4::spawn_local(async move {
                    let _ = call("cancel_invite", json!({ "code": c })).await;
                });
            }
            sender.input(Msg::Reload);
        }
    });
    let waiting = waiting.to_string();
    let lead = lead.to_string();
    relm4::spawn_local(async move {
        match call(method, params).await {
            Err(e) => {
                clear(&body);
                body.append(&adw::StatusPage::builder().icon_name("dialog-error-symbolic").title("Couldn’t create a code").description(e).build());
            }
            Ok(invite) => {
                *code.borrow_mut() = Some(s(&invite, "code").to_string());
                clear(&body);
                body.append(&text(&lead, &[]));
                show_code(&body, &invite, &waiting, !matches!(joined, Joined::Nothing));
                let (dialog, body) = (dialog.downgrade(), body.downgrade());
                match joined {
                    Joined::Nothing => {}
                    Joined::Device => wait(move |e| {
                        let (Some(dialog), Some(body)) = (dialog.upgrade(), body.upgrade()) else { return false };
                        if s(e, "type") != "device_joined" {
                            return true;
                        }
                        used.set(true);
                        success(&body, &dialog, &format!("{} joined", s(e, "name")), "Your apps and data are syncing to it now.");
                        false
                    }),
                    Joined::Person(before) => wait(move |_| {
                        let (Some(dialog), Some(body)) = (dialog.upgrade(), body.upgrade()) else { return false };
                        let (before, used2) = (before.clone(), used.clone());
                        relm4::spawn_local(async move {
                            let people = call("people", Value::Null).await.unwrap_or_default();
                            if let Some(p) = people.as_array().into_iter().flatten().find(|p| !before.iter().any(|b| b == s(p, "id"))) {
                                if !used2.replace(true) {
                                    success(&body, &dialog, &format!("Linked with {}", s(p, "name")), "Nothing is shared yet. Open a list or a calendar in its app and choose Share.");
                                }
                            }
                        });
                        !used.get()
                    }),
                }
            }
        }
    });
}

/// "Claude is ready": how to add UUOB to the AI. The key is shown only once.
fn ai_ready(root: &adw::ApplicationWindow, name: &str, url: &str, token: &str, sender: &Sender) {
    let (_, body) = sheet(root, &format!("“{name}” is ready"));
    body.append(&text("Add UUOB to your AI with one of these. The key is shown only once. Then choose what it can access on this screen.", &[]));
    let command = format!("claude mcp add --transport http uuob {url} --header \"Authorization: Bearer {token}\"");
    let config = serde_json::to_string_pretty(&json!({ "mcpServers": { "uuob": { "type": "http", "url": url, "headers": { "Authorization": format!("Bearer {token}") } } } })).unwrap_or_default();
    for (label, snippet, what) in [("Claude Code", command, "Command"), ("Other MCP apps (JSON)", config, "Config")] {
        let head = gtk::Box::builder().spacing(8).build();
        head.append(&gtk::Label::builder().label(label).css_classes(["heading"]).hexpand(true).xalign(0.0).build());
        let copy = gtk::Button::builder().label("Copy").css_classes(["flat"]).build();
        let (sender, snippet2) = (sender.clone(), snippet.clone());
        copy.connect_clicked(move |_| {
            copy_text(&snippet2);
            sender.input(Msg::Toast(format!("{what} copied")));
        });
        head.append(&copy);
        body.append(&head);
        body.append(&gtk::Label::builder().label(&snippet).selectable(true).wrap(true).wrap_mode(gtk::pango::WrapMode::Char).xalign(0.0).css_classes(["monospace", "card"]).build());
    }
}

/// The share sheet: an app asked to share one of its lists or calendars; who gets it.
fn share_sheet(root: &adw::ApplicationWindow, sender: Sender, ask: Value, data: &Data) {
    let (dialog, body) = sheet(root, &format!("Share “{}”", s(&ask, "container_name")));
    let container = s(&ask, "container").to_string();
    let me = s(&data.status["network"], "name").to_string();
    let invite: Rc<RefCell<Option<String>>> = Rc::default();
    let id = s(&ask, "id").to_string();
    // Closing the sheet is Done.
    dialog.connect_closed({
        let (invite, sender) = (invite.clone(), sender.clone());
        move |_| {
            let (id, code) = (id.clone(), invite.borrow_mut().take());
            let sender = sender.clone();
            relm4::spawn_local(async move {
                if let Some(c) = code {
                    let _ = call("cancel_invite", json!({ "code": c })).await;
                }
                let _ = call("answer", json!({ "id": id, "allow": true })).await;
                sender.input(Msg::Reload);
            });
        }
    });
    let lead = format!("From {}. People you choose get this and nothing else.", s(&ask, "app_name"));
    let list = Rc::new(RefCell::new(None::<Box<dyn Fn()>>));
    let show_people = {
        let (body, container, sender, list2, invite, root) = (body.downgrade(), container.clone(), sender.clone(), list.clone(), invite.clone(), root.clone());
        move || {
            let Some(body) = body.upgrade() else { return };
            let (container, sender, list2, invite, root, me) = (container.clone(), sender.clone(), list2.clone(), invite.clone(), root.clone(), me.clone());
            let lead = lead.clone();
            relm4::spawn_local(async move {
                let people = call("people", Value::Null).await.unwrap_or_default();
                clear(&body);
                body.append(&text(&lead, &["dim-label"]));
                let group = adw::PreferencesGroup::new();
                for p in people.as_array().into_iter().flatten() {
                    let level = p["sharing"].as_array().into_iter().flatten().find(|x| s(x, "container") == container).map(|x| s(x, "access")).unwrap_or("none").to_string();
                    let row = adw::ActionRow::builder().title(s(p, "name")).build();
                    row.add_prefix(&avatar(s(p, "name"), 32));
                    let (c, pid, sender) = (container.clone(), s(p, "id").to_string(), sender.clone());
                    row.add_suffix(&choice(&[("none", "No access"), ("read", "Can view"), ("write", "Can edit")], &level, move |v| {
                        let access = if v == "none" { Value::Null } else { Value::String(v) };
                        sender.input(Msg::Act("share_with", json!({ "container": c, "person": pid, "access": access }), None));
                    }));
                    group.add(&row);
                }
                let new = adw::ActionRow::builder().title("Someone new…").activatable(true).build();
                new.add_prefix(&gtk::Image::from_icon_name("list-add-symbolic"));
                new.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
                let (body2, c2, list3, invite2) = (body.downgrade(), container.clone(), list2.clone(), invite.clone());
                new.connect_activated(move |_| {
                    if me.is_empty() {
                        sender.input(Msg::Toast("First set your name in People.".into()));
                        return;
                    }
                    if let Some(b) = body2.upgrade() {
                        someone_new(&b, &c2, "write", list3.clone(), invite2.clone());
                    }
                });
                group.add(&new);
                body.append(&group);
                let _ = &root;
            });
        }
    };
    *list.borrow_mut() = Some(Box::new(show_people));
    if let Some(show) = &*list.borrow() {
        show();
    }
}

/// "Share with someone new": a code that links the person and shares this with them.
fn someone_new(body: &gtk::Box, container: &str, access: &str, list: Rc<RefCell<Option<Box<dyn Fn()>>>>, invite: Rc<RefCell<Option<String>>>) {
    clear(body);
    body.append(&text("They open UUOB on their device, go to Access → Add → A person → Enter their code, and type this.", &[]));
    let (b2, c2, l2, i2) = (body.downgrade(), container.to_string(), list.clone(), invite.clone());
    body.append(&choice(&[("read", "Can view"), ("write", "Can edit")], access, move |v| {
        if let Some(b) = b2.upgrade() {
            someone_new(&b, &c2, &v, l2.clone(), i2.clone());
        }
    }));
    body.append(&spinner("Creating a code…"));
    let back = gtk::Button::builder().label("Back").css_classes(["pill"]).halign(gtk::Align::Center).build();
    let (l3, i3) = (list.clone(), invite.clone());
    back.connect_clicked(move |_| {
        if let Some(c) = i3.borrow_mut().take() {
            relm4::spawn_local(async move {
                let _ = call("cancel_invite", json!({ "code": c })).await;
            });
        }
        if let Some(show) = &*l3.borrow() {
            show();
        }
    });
    let (body, container, access) = (body.downgrade(), container.to_string(), access.to_string());
    relm4::spawn_local(async move {
        if let Some(c) = invite.borrow_mut().take() {
            let _ = call("cancel_invite", json!({ "code": c })).await;
        }
        let before: Vec<String> = call("people", Value::Null).await.unwrap_or_default().as_array().into_iter().flatten().map(|p| s(p, "id").to_string()).collect();
        let Ok(code) = call("create_person_invite", json!({ "container": container, "access": access })).await else { return };
        let Some(body) = body.upgrade() else { return };
        *invite.borrow_mut() = Some(s(&code, "code").to_string());
        if let Some(spin) = body.last_child() {
            body.remove(&spin);
        }
        show_code(&body, &code, "Waiting for them…", true);
        body.append(&back);
        // The person who used the code shows up in the list: back to it.
        wait(move |_| {
            let (list, invite, before) = (list.clone(), invite.clone(), before.clone());
            if invite.borrow().is_none() {
                return false;
            }
            relm4::spawn_local(async move {
                let people = call("people", Value::Null).await.unwrap_or_default();
                if people.as_array().into_iter().flatten().any(|p| !before.iter().any(|b| b == s(p, "id"))) && invite.borrow_mut().take().is_some() {
                    if let Some(show) = &*list.borrow() {
                        show();
                    }
                }
            });
            true
        });
    });
}
