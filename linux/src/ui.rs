//! Small pieces the pages and dialogs share.

use relm4::adw::prelude::*;
use relm4::{adw, gtk, gtk::glib};
use serde_json::Value;

/// A JSON field as text ("" when missing).
pub fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or_default()
}

pub fn plural(n: u64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

pub fn size(b: u64) -> String {
    match b {
        b if b < 1024 => format!("{b} B"),
        b if b < 1 << 20 => format!("{:.1} KB", b as f64 / 1024.0),
        b if b < 1 << 30 => format!("{:.1} MB", b as f64 / 1048576.0),
        b => format!("{:.1} GB", b as f64 / 1073741824.0),
    }
}

/// A date from milliseconds since 1970, like "18 Nov 2026".
pub fn day(ms: f64) -> String {
    glib::DateTime::from_unix_local((ms / 1000.0) as i64).and_then(|d| d.format("%-d %b %Y")).map(|s| s.to_string()).unwrap_or_default()
}

/// Every app gets a tile the same way: a colour from its id and its first letter.
pub fn app_tile(app: &str, size: i32) -> gtk::Widget {
    const COLORS: [&str; 8] = ["blue", "red", "orange", "green", "purple", "pink", "teal", "indigo"];
    let hash = app.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
    tile(&initial(app), COLORS[hash as usize % COLORS.len()], size)
}

/// A person: their initial in a circle.
pub fn avatar(name: &str, size: i32) -> gtk::Widget {
    adw::Avatar::new(size, Some(name), true).upcast()
}

fn tile(text: &str, color: &str, size: i32) -> gtk::Widget {
    let label = gtk::Label::builder().label(text).css_classes(["tile", color]).width_request(size).height_request(size).build();
    label.upcast()
}

pub fn initial(name: &str) -> String {
    name.trim().chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".into())
}

/// A choice between a few values (linked toggle buttons), e.g. Can view / Can edit.
pub fn choice(options: &[(&str, &str)], selected: &str, on_select: impl Fn(String) + 'static) -> gtk::Box {
    let row = gtk::Box::builder().css_classes(["linked"]).valign(gtk::Align::Center).build();
    let on_select = std::rc::Rc::new(on_select);
    let mut first: Option<gtk::ToggleButton> = None;
    for (value, label) in options {
        let b = gtk::ToggleButton::builder().label(*label).active(*value == selected).build();
        if let Some(f) = &first {
            b.set_group(Some(f));
        } else {
            first = Some(b.clone());
        }
        let value = value.to_string();
        let on_select = on_select.clone();
        b.connect_toggled(move |b| {
            if b.is_active() {
                on_select(value.clone());
            }
        });
        row.append(&b);
    }
    row
}

/// A QR code of `text`, black on white so any camera reads it.
pub fn qr(text: &str, size: i32) -> gtk::Widget {
    let code = qrcode::QrCode::new(text.as_bytes()).ok();
    let area = gtk::DrawingArea::builder().content_width(size).content_height(size).halign(gtk::Align::Center).build();
    area.set_draw_func(move |_, cr, w, h| {
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.paint().ok();
        let Some(code) = &code else { return };
        let n = code.width() as f64;
        let quiet = 2.0;
        let cell = (w.min(h) as f64) / (n + 2.0 * quiet);
        cr.set_source_rgb(0.0, 0.0, 0.0);
        for (i, c) in code.to_colors().iter().enumerate() {
            if *c == qrcode::Color::Dark {
                let (x, y) = ((i as f64 % n) + quiet, (i as f64 / n).floor() + quiet);
                cr.rectangle(x * cell, y * cell, cell + 0.3, cell + 0.3);
            }
        }
        cr.fill().ok();
    });
    let frame = gtk::Frame::builder().child(&area).css_classes(["qr"]).halign(gtk::Align::Center).build();
    frame.upcast()
}

/// Four words or more: a code worth trying.
pub fn complete(code: &str) -> bool {
    code.split(|c: char| c.is_whitespace() || c == '-').filter(|w| !w.is_empty()).count() >= 4
}

/// UUOB's look on top of Adwaita: app tiles, the QR frame, the code's words.
pub fn style() {
    let css = gtk::CssProvider::new();
    css.load_from_string(
        ".tile { border-radius: 10px; color: white; font-weight: 700; font-size: 1.1em; }
         .tile.blue { background: #3584e4; } .tile.red { background: #e01b24; } .tile.orange { background: #ff7800; }
         .tile.green { background: #2ec27e; } .tile.purple { background: #9141ac; } .tile.pink { background: #e5739d; }
         .tile.teal { background: #2190a4; } .tile.indigo { background: #5e5ce6; }
         .qr { border-radius: 16px; padding: 12px; background: white; border: none; }
         .word { border-radius: 10px; padding: 8px 12px; font-size: 1.4em; font-weight: 700; background: alpha(currentColor, 0.08); }
         .word.number { background: alpha(@accent_bg_color, 0.2); color: @accent_color; }",
    );
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
}
