//! What two of the three tabs show, worked out once for the Mac, Android and
//! Linux apps, which only draw it:
//! - [`graph`], the Network tab: your devices in the middle, each app and AI
//!   on the device it runs on, people at the edge, laid out for a width.
//! - [`access`], the Access tab: every app, person and AI, with what each can
//!   read and write.
//!
//! Both read what the node already says (`devices`, `clients`, `apps`,
//! `people`, `containers`), so they are plain functions over that, tested
//! with made-up networks.

use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::PI;

use serde::Deserialize;
use serde_json::{Value as Json, json};
use uuob_core::{Client, Node};

/// What the node says, as far as the two tabs need it.
#[derive(Deserialize, Default)]
pub struct Input {
    #[serde(default)]
    devices: Vec<Dev>,
    #[serde(default)]
    clients: BTreeMap<String, Conn>,
    #[serde(default)]
    apps: BTreeMap<String, App>,
    #[serde(default)]
    people: Vec<Person>,
    #[serde(default)]
    containers: Vec<Container>,
}

#[derive(Deserialize)]
struct Dev {
    id: String,
    name: String,
    #[serde(default)]
    platform: String,
    #[serde(default)]
    is_self: bool,
    #[serde(default)]
    online: bool,
    #[serde(default)]
    keeper: bool,
}

#[derive(Deserialize)]
struct Conn {
    name: String,
    #[serde(default)]
    app: Option<String>,
    #[serde(default)]
    endpoint: Option<String>,
    #[serde(default)]
    device: Option<String>,
    #[serde(default)]
    on: Option<String>,
}

#[derive(Deserialize)]
struct App {
    name: String,
    #[serde(default)]
    installed: bool,
}

#[derive(Deserialize)]
struct Person {
    id: String,
    name: String,
    #[serde(default)]
    online: bool,
    #[serde(default)]
    sharing: Vec<Share>,
    #[serde(default)]
    receiving: Vec<Share>,
}

#[derive(Deserialize)]
struct Share {
    name: String,
    access: String,
    #[serde(default)]
    offered: bool,
}

#[derive(Deserialize)]
struct Container {
    name: String,
    #[serde(default)]
    owner_app: String,
    #[serde(default)]
    access: Vec<Grant>,
}

#[derive(Deserialize)]
struct Grant {
    client: String,
    access: String,
}

impl Input {
    pub fn from_node(node: &Node) -> Input {
        let all = json!({
            "devices": node.devices(),
            "clients": node.clients(),
            "apps": node.apps(),
            "people": node.people(),
            "containers": node.containers(&Client::Shell).unwrap_or_default(),
        });
        serde_json::from_value(all).unwrap_or_default()
    }

    /// Where a connection runs.
    fn place(&self, c: &Conn) -> Place {
        if c.endpoint.is_some() {
            return Place::Linked(c.device.clone().unwrap_or_else(|| "Another device".into()));
        }
        match &c.on {
            Some(id) if self.devices.iter().any(|d| &d.id == id) => Place::Device(id.clone()),
            _ => Place::Unseen,
        }
    }

    /// Places in the order the devices are listed (this device first).
    fn rank(&self, p: &Place) -> (usize, String) {
        match p {
            Place::Device(id) => (self.devices.iter().position(|d| &d.id == id).unwrap_or(0), String::new()),
            Place::Linked(name) => (self.devices.len(), name.clone()),
            Place::Unseen => (self.devices.len() + 1, String::new()),
        }
    }

    fn place_name(&self, p: &Place) -> String {
        match p {
            Place::Device(id) => {
                let d = self.devices.iter().find(|d| &d.id == id).expect("a device");
                if d.is_self { "This device".into() } else { d.name.clone() }
            }
            Place::Linked(name) => name.clone(),
            Place::Unseen => "Not seen yet".into(),
        }
    }

    /// Container names each client (`app:<id>`, `ai:<id>`) reads and writes.
    fn grants(&self) -> BTreeMap<&str, (BTreeSet<&str>, BTreeSet<&str>)> {
        let mut out: BTreeMap<&str, (BTreeSet<&str>, BTreeSet<&str>)> = BTreeMap::new();
        for c in &self.containers {
            for g in &c.access {
                let e = out.entry(g.client.as_str()).or_default();
                if g.access == "write" { e.1.insert(&c.name) } else { e.0.insert(&c.name) };
            }
        }
        out
    }

    /// AI clients: connections that are not an app's.
    fn ais(&self) -> impl Iterator<Item = (&String, &Conn)> {
        self.clients.iter().filter(|(_, c)| c.app.is_none())
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Place {
    Device(String),
    Linked(String),
    Unseen,
}

/// "A", "A and B", "A, B and C".
fn names<'a>(list: impl IntoIterator<Item = &'a str>) -> String {
    let v: Vec<&str> = list.into_iter().collect();
    match v.len() {
        0 => String::new(),
        1 => v[0].into(),
        n => format!("{} and {}", v[..n - 1].join(", "), v[n - 1]),
    }
}

/// "Reads and writes Calendar · reads Tasks".
fn can(reads: &BTreeSet<&str>, writes: &BTreeSet<&str>) -> String {
    let mut parts = vec![];
    if !writes.is_empty() {
        parts.push(format!("Reads and writes {}", names(writes.iter().copied())));
    }
    let only: Vec<&str> = reads.difference(writes).copied().collect();
    if !only.is_empty() {
        parts.push(format!("{} {}", if parts.is_empty() { "Reads" } else { "reads" }, names(only)));
    }
    if parts.is_empty() { "No access yet".into() } else { parts.join(" · ") }
}

/// "Can edit Shopping · can view Trips · shares Book club with you".
fn shares(p: &Person) -> String {
    let with = |a: &str| names(p.sharing.iter().filter(|s| s.access == a).map(|s| s.name.as_str()));
    let mut parts = vec![];
    for (label, list) in [
        ("can edit", with("write")),
        ("can view", with("read")),
        ("shares", names(p.receiving.iter().filter(|s| !s.offered).map(|s| s.name.as_str()))),
        ("offers you", names(p.receiving.iter().filter(|s| s.offered).map(|s| s.name.as_str()))),
    ] {
        if !list.is_empty() {
            let tail = if label == "shares" { " with you" } else { "" };
            parts.push(format!("{label} {list}{tail}"));
        }
    }
    let text = parts.join(" · ");
    let mut chars = text.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => "Nothing shared yet".into(),
    }
}

/// Which of the 8 app colours an app gets, the same on every platform (each
/// has its own 8, in this order: blue, red, orange, green, purple, pink,
/// teal, indigo).
pub fn tint(app: &str) -> u32 {
    app.chars().fold(0u32, |h, c| (h.wrapping_mul(31).wrapping_add(c as u32)) & 0x7fff_ffff) % 8
}

fn letter(name: &str) -> String {
    name.trim().chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "?".into())
}

/// The Access tab: one row per app, AI and person, in that order.
pub fn access(input: &Input) -> Json {
    let grants = input.grants();
    let none = (BTreeSet::new(), BTreeSet::new());
    let mut rows = vec![];
    // Connected apps, and disconnected ones whose data is kept.
    let mut apps: Vec<_> =
        input.apps.iter().filter(|(id, a)| a.installed || input.containers.iter().any(|c| &c.owner_app == *id)).collect();
    apps.sort_by_key(|(_, a)| a.name.to_lowercase());
    for (id, a) in apps {
        let key = format!("app:{id}");
        let (r, w) = grants.get(key.as_str()).unwrap_or(&none);
        let mut places: Vec<Place> = input.clients.values().filter(|c| c.app.as_ref() == Some(id)).map(|c| input.place(c)).collect();
        places.sort_by_key(|p| input.rank(p));
        places.dedup();
        let summary = if a.installed { can(r, w) } else { "Not connected · its data is kept".into() };
        rows.push(json!({
            "key": key, "kind": "app", "id": id, "name": a.name, "letter": letter(&a.name), "tint": tint(id),
            "where": names(places.iter().map(|p| input.place_name(p)).collect::<Vec<_>>().iter().map(String::as_str)),
            "summary": summary, "reads": r, "writes": w, "connected": a.installed,
        }));
    }
    let mut ais: Vec<_> = input.ais().collect();
    ais.sort_by_key(|(_, c)| c.name.to_lowercase());
    for (id, c) in ais {
        let key = format!("ai:{id}");
        let (r, w) = grants.get(key.as_str()).unwrap_or(&none);
        rows.push(json!({
            "key": key, "kind": "ai", "id": id, "name": c.name, "letter": letter(&c.name),
            "where": input.place_name(&input.place(c)), "summary": can(r, w), "reads": r, "writes": w,
        }));
    }
    let mut people: Vec<_> = input.people.iter().collect();
    people.sort_by_key(|p| p.name.to_lowercase());
    for p in people {
        rows.push(json!({
            "key": format!("person:{}", p.id), "kind": "person", "id": p.id, "name": p.name, "letter": letter(&p.name),
            "where": if p.online { "Online" } else { "Offline" }, "online": p.online, "summary": shares(p),
        }));
    }
    json!(rows)
}

/// One thing drawn in the graph.
struct Spot {
    id: String,
    kind: &'static str,
    label: String,
    x: f64,
    y: f64,
    size: f64,
    extra: Json,
}

/// The Network tab's graph for a frame `width` wide (its height comes with
/// it). `open`: the groups unfolded (`apps:<device>`, `ai:<device>`,
/// `people`); more than 3 of a kind otherwise show as one bubble.
///
/// Nodes: `{id, kind, label, x, y, size, …}`, kind one of `device` (with
/// `self`, `online`, `dotted` for a device without UUOB or "Not seen yet"),
/// `app` (`letter`, `tint`), `ai`, `person` (`letter`, `online`), `bubble`
/// (`count`, `group`) and `fold` (`group`), each with `about: {kind, title,
/// detail}` for when it is tapped, and `key`, its row in [`access`]. Lines:
/// `{x1, y1, x2, y2, style}`, style `device`, `app`, `person` or `link`.
pub fn graph(input: &Input, width: f64, open: &BTreeSet<String>) -> Json {
    let w = width.clamp(300.0, 1400.0);
    let h = (w * 0.62).clamp(420.0, 560.0);
    let s = (w / 800.0).clamp(0.75, 1.0);
    let (cx, cy) = (w / 2.0, h / 2.0 - 10.0 * s);
    let rows = access(input);
    let row = |key: &str| rows.as_array().unwrap().iter().find(|r| r["key"] == key).cloned().unwrap_or_default();

    // The ring: your devices, then devices without UUOB, then "Not seen yet".
    struct Ring {
        id: String,
        label: String,
        place: Place,
        uuob: bool,
        about: Json,
        extra: Json,
    }
    let mut ring: Vec<Ring> = input
        .devices
        .iter()
        .map(|d| {
            let runs = names(
                input
                    .clients
                    .values()
                    .filter(|c| input.place(c) == Place::Device(d.id.clone()))
                    .map(|c| c.name.as_str())
                    .collect::<BTreeSet<_>>(),
            );
            let mut detail = vec![if d.online { "Online" } else { "Offline" }.to_string()];
            if d.keeper {
                detail.push("keeps a copy of everything".into());
            }
            if !runs.is_empty() {
                detail.push(format!("runs {runs}"));
            }
            Ring {
                id: format!("dev:{}", d.id),
                label: if d.is_self { "This device".into() } else { d.name.clone() },
                place: Place::Device(d.id.clone()),
                uuob: true,
                about: json!({"kind": if d.is_self { "This device" } else { "Device" }, "title": d.name, "detail": format!("{}.", detail.join(" · "))}),
                extra: json!({"self": d.is_self, "online": d.online, "keeper": d.keeper, "platform": d.platform, "key": d.id}),
            }
        })
        .collect();
    let linked: BTreeSet<String> =
        input.clients.values().filter_map(|c| match input.place(c) { Place::Linked(n) => Some(n), _ => None }).collect();
    for name in linked {
        ring.push(Ring {
            id: format!("linked:{name}"),
            label: name.clone(),
            place: Place::Linked(name.clone()),
            uuob: false,
            about: json!({"kind": "Device without UUOB", "title": name, "detail": "Its apps reach your network through any of your devices."}),
            extra: json!({"dotted": true}),
        });
    }
    if input.clients.values().any(|c| input.place(c) == Place::Unseen) {
        ring.push(Ring {
            id: "unseen".into(),
            label: "Not seen yet".into(),
            place: Place::Unseen,
            uuob: false,
            about: json!({"kind": "Not seen yet", "title": "Where these run isn’t known yet", "detail": "Each moves to its device the next time it uses UUOB."}),
            extra: json!({"dotted": true}),
        });
    }

    // How far from (x, y) in direction `a` the frame ends (nodes stay this
    // far from its sides, and leave room for their labels at the bottom).
    let edge = |x: f64, y: f64, a: f64| {
        let (lo_x, hi_x, lo_y, hi_y) = (24.0 * s, w - 24.0 * s, 0.0, h - 22.0);
        let (dx, dy) = (a.cos(), a.sin());
        let tx = if dx > 1e-9 { (hi_x - x) / dx } else if dx < -1e-9 { (lo_x - x) / dx } else { f64::MAX };
        let ty = if dy > 1e-9 { (hi_y - y) / dy } else if dy < -1e-9 { (lo_y - y) / dy } else { f64::MAX };
        tx.min(ty)
    };
    let n = ring.len().max(1);
    let radius = if n == 1 { 0.0 } else { w.min(h) * 0.2 };
    let angle = |i: usize| PI + 2.0 * PI * i as f64 / n as f64;
    let mut spots: Vec<Spot> = vec![];
    // Lines join two nodes by id: they follow the nodes when those move apart below.
    let mut lines: Vec<(String, String, &str)> = vec![];
    let centers: Vec<(f64, f64)> = (0..ring.len()).map(|i| (cx + radius * angle(i).cos(), cy + radius * angle(i).sin())).collect();
    // The nearest of your devices, for people and devices without UUOB.
    let nearest = |x: f64, y: f64| {
        ring.iter()
            .zip(&centers)
            .filter(|(r, _)| r.uuob)
            .min_by(|(_, a), (_, b)| (a.0 - x).hypot(a.1 - y).total_cmp(&(b.0 - x).hypot(b.1 - y)))
            .map(|(r, _)| r.id.clone())
    };

    // Lines between your devices: all of them while few, else each to the next.
    let mine: Vec<usize> = (0..ring.len()).filter(|&i| ring[i].uuob).collect();
    for (a, &i) in mine.iter().enumerate() {
        for &j in &mine[a + 1..] {
            if mine.len() <= 4 || j == mine[(a + 1) % mine.len()] {
                lines.push((ring[i].id.clone(), ring[j].id.clone(), "device"));
            }
        }
    }

    for (i, r) in ring.iter().enumerate() {
        let (x, y) = centers[i];
        let dotted = !r.uuob;
        if dotted {
            if let Some(to) = nearest(x, y) {
                lines.push((r.id.clone(), to, "link"));
            }
        }
        let mut extra = r.extra.clone();
        extra["about"] = r.about.clone();
        spots.push(Spot { id: r.id.clone(), kind: "device", label: r.label.clone(), x, y, size: if dotted { 50.0 } else { 68.0 } * s, extra });


        // Its apps and AIs, each kind folded into a bubble when there are more than 3.
        let here: Vec<(&String, &Conn)> = input.clients.iter().filter(|(_, c)| input.place(c) == r.place).collect();
        let mut apps: Vec<(&String, String)> =
            here.iter().filter_map(|(_, c)| c.app.as_ref().map(|a| (a, input.apps.get(a).map(|x| x.name.clone()).unwrap_or(c.name.clone())))).collect();
        apps.sort_by_key(|(id, name)| (name.to_lowercase(), (*id).clone()));
        apps.dedup_by_key(|(id, _)| (*id).clone());
        let mut ais: Vec<(&String, &Conn)> = here.iter().copied().filter(|(_, c)| c.app.is_none()).collect();
        ais.sort_by_key(|(id, c)| (c.name.to_lowercase(), (*id).clone()));
        let place = r.label.clone();
        let mut items: Vec<Spot> = vec![];
        let app_group = format!("apps:{}", r.id.trim_start_matches("dev:"));
        if apps.len() > 3 && !open.contains(&app_group) {
            items.push(bubble(&app_group, apps.len(), "apps", &format!("{} apps on {}", apps.len(), r.label), s));
        } else {
            if apps.len() > 3 {
                items.push(fold(&app_group, s));
            }
            for (id, name) in &apps {
                let key = format!("app:{id}");
                items.push(Spot {
                    id: format!("app:{}:{id}", r.id),
                    kind: "app",
                    label: name.clone(),
                    x: 0.0,
                    y: 0.0,
                    size: 42.0 * s,
                    extra: json!({"key": key, "letter": letter(name), "tint": tint(id),
                        "about": {"kind": format!("App on {place}"), "title": name, "detail": row(&key)["summary"]}}),
                });
            }
        }
        let ai_group = format!("ai:{}", r.id.trim_start_matches("dev:"));
        if ais.len() > 3 && !open.contains(&ai_group) {
            items.push(bubble(&ai_group, ais.len(), "AIs", &format!("{} AIs on {}", ais.len(), r.label), s));
        } else {
            if ais.len() > 3 {
                items.push(fold(&ai_group, s));
            }
            for (id, c) in ais {
                let key = format!("ai:{id}");
                items.push(Spot {
                    id: format!("ai:{id}"),
                    kind: "ai",
                    label: c.name.clone(),
                    x: 0.0,
                    y: 0.0,
                    size: 42.0 * s,
                    extra: json!({"key": key, "letter": letter(&c.name),
                        "about": {"kind": format!("AI on {place}"), "title": c.name, "detail": row(&key)["summary"]}}),
                });
            }
        }

        // Fanned out away from the middle (to the left when there is one
        // device), each as far out as the frame allows.
        let k = items.len();
        let dir = if n == 1 { PI } else { angle(i) };
        let dev = if dotted { 50.0 } else { 68.0 } * s;
        let item = items.iter().map(|it| it.size).fold(42.0 * s, f64::max);
        let close = dev / 2.0 + item / 2.0 + 8.0;
        let reach = ((125.0 + 14.0 * k.saturating_sub(5) as f64) * s * if dotted { 0.8 } else { 1.0 }).max(close);
        let room = |a: f64| (edge(x, y, a) - item / 2.0 - 6.0).clamp(close, reach);
        // Wide enough apart that neighbours don't touch, even when pulled in.
        let inner = (0..k).map(|j| room(dir + (j as f64 - (k as f64 - 1.0) / 2.0) * 0.55)).fold(reach, f64::min);
        let step = (2.0 * ((item + 8.0 * s) / (2.0 * inner)).min(1.0).asin()).max(if k <= 5 { 0.55 } else { 2.6 / (k - 1) as f64 });
        let at = |j: f64| dir + (j - (k as f64 - 1.0) / 2.0) * step;
        for (j, mut it) in items.into_iter().enumerate() {
            let a = at(j as f64);
            (it.x, it.y) = (x + room(a) * a.cos(), y + room(a) * a.sin());
            if it.kind != "fold" {
                lines.push((r.id.clone(), it.id.clone(), "app"));
            }
            spots.push(it);
        }
    }

    // People at the edge, in the widest gap between the devices' directions.
    if !input.people.is_empty() {
        let dirs: Vec<f64> = if n == 1 { vec![PI] } else { (0..n).map(|i| angle(i).rem_euclid(2.0 * PI)).collect() };
        let mut sorted = dirs.clone();
        sorted.sort_by(f64::total_cmp);
        let mut best = (f64::MIN, 0.0_f64);
        for (a, b) in sorted.iter().zip(sorted.iter().cycle().skip(1)) {
            let gap = (b - a).rem_euclid(2.0 * PI);
            let gap = if gap == 0.0 { 2.0 * PI } else { gap };
            let mid = (a + gap / 2.0).rem_euclid(2.0 * PI);
            // Ties go to the right-hand side.
            let rightness = mid.cos();
            if gap > best.0 + 1e-6 || ((gap - best.0).abs() <= 1e-6 && rightness > best.1.cos()) {
                best = (gap, mid);
            }
        }
        let phi = best.1;
        let (ex, ey) = (w / 2.0 - 46.0 * s, h / 2.0 - 56.0 * s);
        let rim = |a: f64| (cx + ex * a.cos(), cy + ey * a.sin());
        let mut people: Vec<&Person> = input.people.iter().collect();
        people.sort_by_key(|p| (p.name.to_lowercase(), p.id.clone()));
        let mut items = vec![];
        if people.len() > 3 && open.contains("people") {
            items.push(fold("people", s));
        }
        if people.len() > 3 && !open.contains("people") {
            items.push(bubble("people", people.len(), "people", &format!("{} people you share with", people.len()), s));
        } else {
            for p in &people {
                let key = format!("person:{}", p.id);
                items.push(Spot {
                    id: key.clone(),
                    kind: "person",
                    label: p.name.clone(),
                    x: 0.0,
                    y: 0.0,
                    size: 42.0 * s,
                    extra: json!({"key": key, "letter": letter(&p.name), "online": p.online,
                        "about": {"kind": "Person", "title": p.name, "detail": row(&key)["summary"]}}),
                });
            }
        }
        let k = items.len();
        let step = if k <= 6 { 0.42 } else { 2.4 / (k - 1) as f64 };
        for (j, mut it) in items.into_iter().enumerate() {
            (it.x, it.y) = rim(phi + (j as f64 - (k as f64 - 1.0) / 2.0) * step);
            if it.kind != "fold" {
                if let Some(to) = nearest(it.x, it.y) {
                    lines.push((to, it.id.clone(), "person"));
                }
            }
            spots.push(it);
        }
    }

    // Everything inside the frame, with room for labels underneath.
    let inside = |sp: &mut Spot| {
        let r = sp.size / 2.0 + 4.0;
        sp.x = sp.x.clamp(r + 24.0 * s, w - r - 24.0 * s);
        sp.y = sp.y.clamp(r, h - r - 22.0);
    };
    spots.iter_mut().for_each(inside);
    // A crowded network can still have nodes touching: push them apart, a
    // little at a time, leaving the devices where they are.
    for _ in 0..400 {
        let mut moved = false;
        for i in 0..spots.len() {
            for j in i + 1..spots.len() {
                let (dx, dy) = (spots[j].x - spots[i].x, spots[j].y - spots[i].y);
                let d = dx.hypot(dy);
                let need = (spots[i].size + spots[j].size) / 2.0 + 6.0;
                if d >= need {
                    continue;
                }
                let (ux, uy) = if d > 1e-6 { (dx / d, dy / d) } else { (0.0, 1.0) };
                let push = need - d + 0.5;
                let (mi, mj) = (spots[i].kind != "device", spots[j].kind != "device");
                let (pi, pj) = match (mi, mj) {
                    (true, true) => (push / 2.0, push / 2.0),
                    (true, false) => (push, 0.0),
                    (false, true) => (0.0, push),
                    (false, false) => continue,
                };
                spots[i].x -= ux * pi;
                spots[i].y -= uy * pi;
                spots[j].x += ux * pj;
                spots[j].y += uy * pj;
                inside(&mut spots[i]);
                inside(&mut spots[j]);
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    let round = |v: f64| (v * 10.0).round() / 10.0;
    let at: BTreeMap<&str, (f64, f64)> = spots.iter().map(|sp| (sp.id.as_str(), (sp.x, sp.y))).collect();
    let lines: Vec<Json> = lines
        .iter()
        .filter_map(|(a, b, style)| {
            let ((x1, y1), (x2, y2)) = (at.get(a.as_str())?, at.get(b.as_str())?);
            Some(json!({"x1": round(*x1), "y1": round(*y1), "x2": round(*x2), "y2": round(*y2), "style": style}))
        })
        .collect();
    let nodes: Vec<Json> = spots
        .into_iter()
        .map(|sp| {
            let mut j = json!({"id": sp.id, "kind": sp.kind, "label": sp.label, "x": round(sp.x), "y": round(sp.y), "size": round(sp.size)});
            if let (Json::Object(m), Json::Object(e)) = (&mut j, sp.extra) {
                m.extend(e);
            }
            j
        })
        .collect();
    json!({"width": round(w), "height": round(h), "nodes": nodes, "lines": lines})
}

fn bubble(group: &str, count: usize, what: &str, title: &str, s: f64) -> Spot {
    Spot {
        id: format!("bubble:{group}"),
        kind: "bubble",
        label: what.into(),
        x: 0.0,
        y: 0.0,
        size: 60.0 * s,
        extra: json!({"group": group, "count": count, "about": {"kind": "", "title": title, "detail": "Tap to open."}}),
    }
}

fn fold(group: &str, s: f64) -> Spot {
    Spot {
        id: format!("fold:{group}"),
        kind: "fold",
        label: String::new(),
        x: 0.0,
        y: 0.0,
        size: 30.0 * s,
        extra: json!({"group": group, "about": {"kind": "", "title": "Fold", "detail": ""}}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The mockup's network: a Mac with 4 apps and Claude, a phone with 2,
    /// a home server, 5 people, an app on a device without UUOB and one
    /// connection from before UUOB noted where.
    fn mockup() -> Input {
        let app = |id: &str, name: &str, on: &str| json!({"name": name, "token_sha256": "", "created_at": 0, "app": id, "on": on});
        serde_json::from_value(json!({
            "devices": [
                {"id": "mac", "name": "MacBook Air", "is_self": true, "online": true},
                {"id": "pixel", "name": "Pixel 8", "online": true},
                {"id": "pi", "name": "Home server", "online": true, "keeper": true},
            ],
            "clients": {
                "c1": app("calendar", "Calendar", "mac"), "c2": app("tasks", "Tasks", "mac"),
                "c3": app("photos", "Photos", "mac"), "c4": app("notes", "Notes", "mac"),
                "c5": app("calendar", "Calendar", "pixel"), "c6": app("tasks", "Tasks", "pixel"),
                "c7": {"name": "Claude", "on": "mac"},
                "c8": {"name": "Journal", "app": "journal", "endpoint": "e1", "device": "Work laptop"},
                "c9": {"name": "Tasks", "app": "tasks"},
            },
            "apps": {
                "calendar": {"name": "Calendar", "installed": true}, "tasks": {"name": "Tasks", "installed": true},
                "photos": {"name": "Photos", "installed": true}, "notes": {"name": "Notes", "installed": true},
                "journal": {"name": "Journal", "installed": true}, "old": {"name": "Old", "installed": false},
            },
            "people": [
                {"id": "p1", "name": "Anna", "online": true, "sharing": [{"name": "Shopping", "access": "write"}, {"name": "Family", "access": "read"}],
                 "receiving": [{"name": "Book club", "access": "write"}, {"name": "Trip", "access": "read", "offered": true}]},
                {"id": "p2", "name": "Ben"}, {"id": "p3", "name": "Clara"}, {"id": "p4", "name": "David"}, {"id": "p5", "name": "Eva"},
            ],
            "containers": [
                {"name": "Calendar", "access": [{"client": "app:calendar", "access": "write"}, {"client": "ai:c7", "access": "read"}]},
                {"name": "Tasks", "access": [{"client": "app:tasks", "access": "write"}, {"client": "app:calendar", "access": "read"}, {"client": "ai:c7", "access": "write"}]},
                {"name": "Old", "owner_app": "old"},
            ],
        }))
        .unwrap()
    }

    fn open(groups: &[&str]) -> BTreeSet<String> {
        groups.iter().map(|g| g.to_string()).collect()
    }

    fn nodes(g: &Json) -> &Vec<Json> {
        g["nodes"].as_array().unwrap()
    }

    fn find<'a>(g: &'a Json, id: &str) -> &'a Json {
        nodes(g).iter().find(|n| n["id"] == id).unwrap_or_else(|| panic!("no {id} in {g:#}"))
    }

    #[test]
    fn access_says_who_reads_and_writes_what() {
        let rows = access(&mockup());
        let row = |key: &str| rows.as_array().unwrap().iter().find(|r| r["key"] == key).unwrap().clone();
        let kinds: Vec<&str> = rows.as_array().unwrap().iter().map(|r| r["kind"].as_str().unwrap()).collect();
        assert_eq!(kinds, ["app", "app", "app", "app", "app", "app", "ai", "person", "person", "person", "person", "person"], "apps, AI, people");
        assert_eq!(row("app:old")["summary"], "Not connected · its data is kept");
        assert_eq!(row("app:calendar")["summary"], "Reads and writes Calendar · reads Tasks");
        assert_eq!(row("app:calendar")["where"], "This device and Pixel 8");
        assert_eq!(row("app:tasks")["where"], "This device, Pixel 8 and Not seen yet");
        assert_eq!(row("app:journal")["where"], "Work laptop");
        assert_eq!(row("app:photos")["summary"], "No access yet");
        assert_eq!(row("ai:c7")["summary"], "Reads and writes Tasks · reads Calendar");
        assert_eq!(row("person:p1")["summary"], "Can edit Shopping · can view Family · shares Book club with you · offers you Trip");
        assert_eq!(row("person:p2")["summary"], "Nothing shared yet");
    }

    #[test]
    fn groups_of_more_than_three_fold_into_bubbles() {
        let g = graph(&mockup(), 800.0, &BTreeSet::new());
        assert_eq!(find(&g, "bubble:apps:mac")["count"], 4);
        assert_eq!(find(&g, "bubble:people")["count"], 5);
        assert_eq!(find(&g, "app:dev:pixel:calendar")["about"]["kind"], "App on Pixel 8");
        assert_eq!(find(&g, "ai:c7")["about"]["detail"], "Reads and writes Tasks · reads Calendar");
        assert_eq!(find(&g, "linked:Work laptop")["dotted"], true);
        assert!(find(&g, "unseen")["dotted"] == true, "the old Tasks connection");
        assert!(nodes(&g).iter().all(|n| n["kind"] != "fold"));

        let g = graph(&mockup(), 800.0, &open(&["apps:mac", "people"]));
        assert!(nodes(&g).iter().all(|n| n["kind"] != "bubble"));
        for id in ["app:dev:mac:photos", "app:dev:mac:notes", "person:p5", "fold:apps:mac", "fold:people"] {
            find(&g, id);
        }
        assert_eq!(find(&g, "dev:mac")["about"]["detail"], "Online · runs Calendar, Claude, Notes, Photos and Tasks.");
    }

    #[test]
    fn the_same_network_draws_the_same_graph() {
        let a = graph(&mockup(), 800.0, &open(&["people"]));
        assert_eq!(a, graph(&mockup(), 800.0, &open(&["people"])));
        // And this device comes first, on the left.
        let (mac, pixel) = (find(&a, "dev:mac"), find(&a, "dev:pixel"));
        assert!(mac["x"].as_f64() < pixel["x"].as_f64());
    }

    #[test]
    fn nothing_overlaps_on_a_phone_or_a_computer() {
        for width in [358.0, 390.0, 800.0, 1200.0] {
            for groups in [&[][..], &["apps:mac"], &["people"], &["apps:mac", "people"]] {
                let g = graph(&mockup(), width, &open(groups));
                let (w, h) = (g["width"].as_f64().unwrap(), g["height"].as_f64().unwrap());
                let all = nodes(&g);
                for (i, a) in all.iter().enumerate() {
                    let (ax, ay, ar) = (a["x"].as_f64().unwrap(), a["y"].as_f64().unwrap(), a["size"].as_f64().unwrap() / 2.0);
                    assert!(ax - ar >= 0.0 && ax + ar <= w && ay - ar >= 0.0 && ay + ar <= h, "{} inside at {width}", a["id"]);
                    for b in &all[i + 1..] {
                        let (bx, by, br) = (b["x"].as_f64().unwrap(), b["y"].as_f64().unwrap(), b["size"].as_f64().unwrap() / 2.0);
                        let gap = (ax - bx).hypot(ay - by) - ar - br;
                        assert!(gap >= 4.0, "{} and {} overlap at width {width} with {groups:?} (gap {gap:.1})", a["id"], b["id"]);
                    }
                }
            }
        }
    }

    #[test]
    fn one_device_alone() {
        let input: Input = serde_json::from_value(json!({"devices": [{"id": "a", "name": "Laptop", "is_self": true, "online": true}]})).unwrap();
        let g = graph(&input, 390.0, &BTreeSet::new());
        assert_eq!(nodes(&g).len(), 1);
        assert!(g["lines"].as_array().unwrap().is_empty());
        assert_eq!(find(&g, "dev:a")["label"], "This device");
    }

    #[test]
    fn tints_match_the_mac_app() {
        // Swift: unicodeScalars.reduce(0) { ($0 &* 31 &+ value) & 0x7fffffff } % 8
        assert_eq!(tint("tasks"), 6);
        assert_eq!(tint("calendar"), 6);
        assert_eq!(tint("notes"), 1);
    }
}
