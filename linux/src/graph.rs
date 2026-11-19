//! The Network page's graph. Where everything goes is worked out by UUOB
//! (`graph`, crates/uuob-app/src/screens.rs), the same on every platform;
//! this draws it: lines behind (cairo), a button per device, app, AI and
//! person on top. A bubble opens its group, the × folds it again; anything
//! else is picked, and the page shows what it is underneath.

use relm4::adw::prelude::*;
use relm4::{ComponentSender, gtk};
use serde_json::Value;

use crate::ui::*;
use crate::{App, Msg};

const COLORS: [&str; 8] = ["blue", "red", "orange", "green", "purple", "pink", "teal", "indigo"];

pub fn graph(g: &Value, picked: Option<&str>, sender: &ComponentSender<App>) -> gtk::Widget {
    let f = |v: &Value| v.as_f64().unwrap_or(0.0);
    let (w, h) = (f(&g["width"]), f(&g["height"]));
    let area = gtk::DrawingArea::builder().content_width(w as i32).content_height(h as i32).build();
    let lines = g["lines"].as_array().cloned().unwrap_or_default();
    area.set_draw_func(move |area, cr, _, _| {
        let fg = area.color();
        for l in &lines {
            let style = s(l, "style");
            let alpha = if style == "device" { 0.45 } else { 0.25 };
            cr.set_source_rgba(fg.red() as f64, fg.green() as f64, fg.blue() as f64, alpha);
            cr.set_line_width(if style == "device" { 2.5 } else { 1.5 });
            cr.set_dash(if matches!(style, "person" | "link") { &[5.0, 5.0] } else { &[] }, 0.0);
            cr.move_to(f(&l["x1"]), f(&l["y1"]));
            cr.line_to(f(&l["x2"]), f(&l["y2"]));
            cr.stroke().ok();
        }
    });

    let fixed = gtk::Fixed::new();
    for n in g["nodes"].as_array().into_iter().flatten() {
        let (x, y, size) = (f(&n["x"]), f(&n["y"]), f(&n["size"]));
        let kind = s(n, "kind");
        let b = gtk::Button::builder().width_request(size as i32).height_request(size as i32).css_classes(["node", kind]).build();
        let what = match kind {
            "bubble" => format!("Open {} {}", n["count"], s(n, "label")),
            "fold" => "Fold".into(),
            _ => s(n, "label").into(),
        };
        b.set_tooltip_text(Some(&what));
        match kind {
            "app" => {
                b.add_css_class("tile");
                b.add_css_class(COLORS[n["tint"].as_u64().unwrap_or(0) as usize % 8]);
                b.set_label(s(n, "letter"));
            }
            "person" => {
                b.add_css_class("tile");
                b.add_css_class("teal");
                b.set_label(s(n, "letter"));
            }
            "ai" => {
                b.add_css_class("tile");
                b.add_css_class("purple");
                b.set_icon_name("starred-symbolic");
            }
            "bubble" => b.set_label(&n["count"].to_string()),
            "fold" => b.set_icon_name("window-close-symbolic"),
            _ => b.set_icon_name(match () {
                _ if s(n, "id") == "unseen" => "dialog-question-symbolic",
                _ if n["keeper"].as_bool() == Some(true) => "network-server-symbolic",
                _ if matches!(s(n, "platform"), "android" | "ios") => "phone-symbolic",
                _ => "computer-symbolic",
            }),
        }
        if n["dotted"].as_bool() == Some(true) {
            b.add_css_class("dotted");
        }
        if picked == Some(s(n, "id")) && !matches!(kind, "bubble" | "fold") {
            b.add_css_class("picked");
        }
        let toggle = matches!(kind, "bubble" | "fold");
        let id = if toggle { s(n, "group") } else { s(n, "id") }.to_string();
        let sender = sender.clone();
        b.connect_clicked(move |_| sender.input(if toggle { Msg::Toggle(id.clone()) } else { Msg::Pick(id.clone()) }));
        fixed.put(&b, x - size / 2.0, y - size / 2.0);
        if kind != "fold" {
            let label = gtk::Label::builder()
                .label(s(n, "label"))
                .width_request(100)
                .max_width_chars(12)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .justify(gtk::Justification::Center)
                .css_classes(if n["self"].as_bool() == Some(true) { vec!["caption-heading"] } else { vec!["caption", "dim-label"] })
                .build();
            fixed.put(&label, x - 50.0, y + size / 2.0 + 4.0);
        }
    }
    let overlay = gtk::Overlay::builder().child(&area).halign(gtk::Align::Center).build();
    overlay.add_overlay(&fixed);
    let frame = gtk::Frame::builder().child(&overlay).css_classes(["graph"]).build();
    frame.upcast()
}
