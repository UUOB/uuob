//! Small pieces the pages and dialogs share.

use relm4::adw::prelude::*;
use relm4::{gtk, gtk::glib};
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

/// Every app gets a tile the same way: a colour from its id (the same on every
/// platform, see `tint` in crates/uuob-app/src/screens.rs) and its first letter.
pub fn app_tile(app: &str, size: i32) -> gtk::Widget {
    const COLORS: [&str; 8] = ["blue", "red", "orange", "green", "purple", "pink", "teal", "indigo"];
    let hash = app.bytes().fold(0x811c_9dc5u32, |h, b| (h ^ b as u32).wrapping_mul(0x0100_0193));
    tile(&initial(app), COLORS[hash as usize % COLORS.len()], size)
}

/// A person: their initial on UUOB's green to blue.
pub fn avatar(name: &str, size: i32) -> gtk::Widget {
    let label = gtk::Label::builder().label(initial(name)).css_classes(["face"]).width_request(size).height_request(size).valign(gtk::Align::Center).halign(gtk::Align::Center).build();
    label.upcast()
}

fn tile(text: &str, color: &str, size: i32) -> gtk::Widget {
    let label = gtk::Label::builder().label(text).css_classes(["tile", color]).width_request(size).height_request(size).valign(gtk::Align::Center).halign(gtk::Align::Center).build();
    label.upcast()
}

pub fn initial(name: &str) -> String {
    name.trim().chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".into())
}

/// A choice between a few values (the kit's segmented control), e.g. Can view / Can edit.
pub fn choice(options: &[(&str, &str)], selected: &str, on_select: impl Fn(String) + 'static) -> gtk::Box {
    let labels: Vec<&str> = options.iter().map(|(_, l)| *l).collect();
    let values: Vec<String> = options.iter().map(|(v, _)| v.to_string()).collect();
    let at = values.iter().position(|v| v == selected).unwrap_or(0);
    uuob_kit_gtk::segmented(&labels, at, move |i| on_select(values[i].clone()))
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

/// The look every UUOB app shares (the kit), plus what only UUOB draws: its
/// pages of grouped rows, the graph, app tiles, faces and codes.
pub fn style() {
    uuob_kit_gtk::style();
    let css = gtk::CssProvider::new();
    css.load_from_string(CSS);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
}

/// In the kit's colours (light and dark follow the theme).
const CSS: &str = r#"
window, .background, preferencespage, preferencespage > scrolledwindow, viewstack { background: @window_bg_color; color: @window_fg_color; }
headerbar { background: @window_bg_color; box-shadow: none; border: none; }
.lead { font-size: 17px; color: @uuob_text2; }
preferencesgroup .heading, .section-label { font-size: 12px; font-weight: 600; color: @uuob_text3; letter-spacing: 0.5px; }
preferencesgroup .description { font-size: 13px; color: @uuob_text2; }

/* groups of rows: a hairline edge, 18 point corners, a line between rows */
list.boxed-list, .boxed-list { background: @card_bg_color; border-radius: 18px; border: 1px solid @uuob_line; box-shadow: none; }
.boxed-list > row { min-height: 56px; padding: 4px 6px; border-bottom: 1px solid @uuob_line; background: transparent; }
.boxed-list > row:last-child { border-bottom: none; }
.boxed-list > row:hover { background: alpha(@window_fg_color, 0.045); }
row .title { font-size: 16px; font-weight: 600; }
row .subtitle { font-size: 13px; color: @uuob_text2; }
.tag { font-size: 12px; font-weight: 600; color: @accent_color; background: alpha(@accent_color, 0.16); padding: 2px 8px; border-radius: 999px; }

/* UUOB's own buttons on its pages: pills (the kit's have their own classes) */
preferencespage button:not(.icon-btn):not(.btn):not(.plus-btn):not(.node):not(.toggle) { border-radius: 999px; min-height: 40px; padding: 0 18px; font-weight: 600; background: @uuob_surface2; color: @window_fg_color; box-shadow: none; border: none; }
preferencespage button.suggested-action { background: @accent_bg_color; color: white; }
preferencespage button.destructive-action { background: alpha(@destructive_color, 0.14); color: @destructive_color; }
preferencespage button.flat:not(.node) { background: transparent; color: @accent_color; }
preferencespage button.flat.image-button { min-width: 36px; min-height: 36px; padding: 0; color: @uuob_text2; }
preferencespage button.pill { min-height: 50px; padding: 0 26px; font-size: 17px; }
headerbar button { background: transparent; min-height: 32px; min-width: 32px; padding: 0 8px; color: @uuob_text2; box-shadow: none; }
headerbar button.suggested-action { background: @accent_bg_color; color: white; padding: 0 14px; border-radius: 999px; }

/* the switch: 51 by 31, green when on */
switch { min-width: 51px; min-height: 31px; border-radius: 999px; padding: 2px; background: alpha(@window_fg_color, 0.14); border: none; }
switch:checked { background: @success_color; }
switch slider { min-width: 27px; min-height: 27px; border-radius: 999px; background: white; box-shadow: 0 2px 5px rgba(0,0,0,0.3); margin: 0; }
banner > revealer > widget { background: alpha(@destructive_color, 0.14); color: @destructive_color; }

/* a segmented control across the page: segments share the width, with little padding so it fits a narrow window */
.segmented.stretch > button { padding-left: 4px; padding-right: 4px; }

/* the sidebar's dots and lock */
.navdot { min-width: 8px; min-height: 8px; border-radius: 999px; }
.navdot.ok { background: @success_color; } .navdot.off { background: @uuob_text3; } .navdot.warn { background: #ff9f0a; }
.rail-btn .navdot { border: 2px solid @uuob_rail; margin: 4px; }
.lock { color: @uuob_text3; font-size: 12px; }

/* app tiles, people, the graph, codes */
.tile { border-radius: 10px; color: white; font-weight: 700; font-size: 1.1em; padding: 0; min-height: 0; }
.tile.blue { background: #0a84ff; } .tile.red { background: #ff453a; } .tile.orange { background: #ff9f0a; }
.tile.green { background: #30d158; } .tile.purple { background: #bf5af2; } .tile.pink { background: #ff375f; }
.tile.teal { background: #40c8e0; } .tile.indigo { background: #5e5ce6; }
.face { border-radius: 999px; color: white; font-weight: 600; background-image: linear-gradient(140deg, #30c48d, #2a9df4); }
.face.mine { background-image: none; background: @accent_color; }
.devtile { border-radius: 12px; background: @uuob_surface2; color: @uuob_text2; }
.qr { border-radius: 16px; padding: 12px; background: white; border: none; }
.word { border-radius: 10px; padding: 8px 12px; font-size: 1.4em; font-weight: 700; background: @uuob_surface2; }
.word.number { background: alpha(@accent_color, 0.16); color: @accent_color; }
.graph { border-radius: 20px; border: 1px solid @uuob_line; background: @view_bg_color; }
.about { background: @card_bg_color; border-radius: 18px; border: 1px solid @uuob_line; padding: 16px 18px; }
.about .kind { font-size: 12px; font-weight: 700; color: @accent_color; letter-spacing: 0.04em; }
.about .what { font-size: 20px; font-weight: 600; }
button.node { padding: 0; min-width: 0; min-height: 0; box-shadow: none; }
.node.device { border-radius: 30%; background: @uuob_surface2; color: @window_fg_color; }
.node.app, .node.ai { border-radius: 28%; }
.node.person, .node.bubble, .node.fold { border-radius: 999px; }
.node.person { background-image: linear-gradient(140deg, #30c48d, #2a9df4); color: white; }
.node.bubble { background: @card_bg_color; border: 1.5px dashed alpha(@window_fg_color, 0.35); font-weight: 700; color: @window_fg_color; }
.node.fold { background: @uuob_surface2; color: @uuob_text2; }
.node.dotted { border: 1.5px dashed alpha(@window_fg_color, 0.35); background: none; }
.node.picked { outline: 3px solid @accent_color; outline-offset: 2px; }
"#;

/// A line icon (24 point grid, 1.75 stroke), the kit's.
pub fn icon(name: &str, size: i32) -> gtk::Widget {
    uuob_kit_gtk::icon(name, size, 1.75, "currentColor").upcast()
}
