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

/// A choice between a few values (linked toggle buttons), e.g. Can view / Can edit.
pub fn choice(options: &[(&str, &str)], selected: &str, on_select: impl Fn(String) + 'static) -> gtk::Box {
    let row = gtk::Box::builder().css_classes(["linked", "segmented"]).valign(gtk::Align::Center).build();
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

/// UUOB's look, the one its screens had as a web page (ui/tokens.css), on top
/// of Adwaita: its colours light and dark, pill buttons, grouped rows with a
/// hairline edge, the green switch, the segmented control, the rail.
pub fn style() {
    let css = gtk::CssProvider::new();
    let manager = relm4::adw::StyleManager::default();
    let load = {
        let css = css.clone();
        move |dark: bool| css.load_from_string(&stylesheet(dark))
    };
    load(manager.is_dark());
    manager.connect_dark_notify(move |m| load(m.is_dark()));
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }
}

/// The tokens, as GTK CSS.
fn stylesheet(dark: bool) -> String {
    // bg, elevated, surface, surface-2, text, line, line-strong, accent, danger, ok, rail
    let (bg, elevated, surface, surface2, text, accent, danger, ok, rail, low) = if dark {
        ("#0b0b0d", "#161618", "#1c1c1f", "#26262a", "#f5f5f7", "#0a84ff", "#ff453a", "#30d158", "#141416", "#161618")
    } else {
        ("#f5f5f7", "#ffffff", "#ffffff", "#f0f0f3", "#1d1d1f", "#007aff", "#ff3b30", "#34c759", "#ececf0", "#fafafb")
    };
    let ink = if dark { "255, 255, 255" } else { "0, 0, 0" };
    let text_rgb = if dark { "245, 245, 247" } else { "29, 29, 31" };
    format!(
        r#"
:root {{ --window-bg-color: {bg}; --window-fg-color: {text}; --view-bg-color: {bg}; --card-bg-color: {surface};
  --headerbar-bg-color: {bg}; --dialog-bg-color: {elevated}; --popover-bg-color: {elevated};
  --accent-bg-color: {accent}; --accent-color: {accent}; --destructive-bg-color: {danger}; --success-color: {ok}; }}
@define-color window_bg_color {bg}; @define-color window_fg_color {text}; @define-color view_bg_color {bg};
@define-color card_bg_color {surface}; @define-color headerbar_bg_color {bg}; @define-color dialog_bg_color {elevated};
@define-color accent_bg_color {accent}; @define-color accent_color {accent}; @define-color popover_bg_color {elevated};

window, .background, preferencespage, preferencespage > scrolledwindow, viewstack {{ background: {bg}; color: {text}; }}
window {{ font-family: "Inter", "SF Pro Text", system-ui, sans-serif; font-size: 15px; letter-spacing: -0.01em; }}
headerbar {{ background: {bg}; box-shadow: none; border: none; }}

/* type */
.large-title {{ font-size: 34px; font-weight: 700; letter-spacing: -0.025em; }}
.lead {{ font-size: 17px; color: rgba({text_rgb}, 0.62); }}
.sheet-title {{ font-size: 22px; font-weight: 600; letter-spacing: -0.02em; }}
preferencesgroup .heading, .section-label {{ font-size: 13px; font-weight: 600; color: rgba({text_rgb}, 0.38); letter-spacing: 0.04em; }}
preferencesgroup .description {{ font-size: 13px; color: rgba({text_rgb}, 0.62); }}

/* groups of rows: a hairline edge, 18 point corners, a line between rows */
list.boxed-list, .boxed-list {{ background: {surface}; border-radius: 18px; border: 1px solid rgba({ink}, 0.08); box-shadow: none; }}
.boxed-list > row {{ min-height: 56px; padding: 4px 6px; border-bottom: 1px solid rgba({ink}, 0.08); background: transparent; }}
.boxed-list > row:last-child {{ border-bottom: none; }}
.boxed-list > row:hover {{ background: rgba({ink}, 0.045); }}
row .title {{ font-size: 16px; font-weight: 600; }}
row .subtitle {{ font-size: 13px; color: rgba({text_rgb}, 0.62); }}
.tag {{ font-size: 12px; font-weight: 600; color: {accent}; background: alpha({accent}, 0.16); padding: 2px 8px; border-radius: 999px; }}

/* buttons: pills */
button {{ border-radius: 999px; min-height: 40px; padding: 0 18px; font-weight: 600; background: {surface2}; color: {text}; box-shadow: none; border: none; }}
button:hover {{ background: mix({surface2}, {text}, 0.08); }}
button:active {{ background: mix({surface2}, {text}, 0.14); }}
button.suggested-action {{ background: {accent}; color: white; }}
button.suggested-action:hover {{ background: mix({accent}, white, 0.12); }}
button.destructive-action {{ background: alpha({danger}, 0.14); color: {danger}; }}
button.flat {{ background: transparent; color: {accent}; }}
button.flat:hover {{ background: alpha({accent}, 0.12); }}
button.flat.image-button, button.image-button.flat {{ min-width: 36px; min-height: 36px; padding: 0; color: rgba({text_rgb}, 0.62); }}
button.flat.image-button:hover {{ background: rgba({ink}, 0.06); color: {text}; }}
button.pill {{ min-height: 50px; padding: 0 26px; font-size: 17px; }}
headerbar button {{ background: transparent; min-height: 32px; min-width: 32px; padding: 0 8px; color: rgba({text_rgb}, 0.62); }}
headerbar button:hover {{ background: rgba({ink}, 0.06); color: {text}; }}
headerbar button.suggested-action {{ background: {accent}; color: white; padding: 0 14px; }}
headerbar windowcontrols button, headerbar windowcontrols button.image-button {{ min-height: 24px; min-width: 24px; padding: 0; margin: 0 4px; border-radius: 999px; background: rgba({ink}, 0.08); }}

/* the switch: 51 by 31, green when on */
switch {{ min-width: 51px; min-height: 31px; border-radius: 999px; padding: 2px; background: rgba({ink}, 0.14); border: none; }}
switch:checked {{ background: {ok}; }}
switch slider {{ min-width: 27px; min-height: 27px; border-radius: 999px; background: white; box-shadow: 0 2px 5px rgba(0,0,0,0.3); margin: 0; }}

/* the segmented control: a grey track, the chosen one raised */
box.linked.segmented {{ background: {surface2}; border-radius: 10px; padding: 3px; }}
box.linked.segmented > button {{ background: transparent; border-radius: 8px; min-height: 0; padding: 5px 14px; font-size: 14px; font-weight: 500; color: rgba({text_rgb}, 0.62); }}
box.linked.segmented > button:checked {{ background: {elevated}; color: {text}; box-shadow: 0 1px 3px rgba(0,0,0,0.25); }}

/* text fields */
entry {{ min-height: 46px; border-radius: 12px; background: {surface2}; border: 1px solid rgba({ink}, 0.08); padding: 0 14px; box-shadow: none; }}
entry:focus-within {{ border-color: {accent}; box-shadow: 0 0 0 4px alpha({accent}, 0.16); background: {surface}; outline: none; }}

/* sheets and toasts */
dialog sheet, dialog.alert sheet, .dialog-sheet {{ background: {elevated}; border-radius: 26px; border: 1px solid rgba({ink}, 0.08); }}
dialog.alert .message-area {{ padding: 28px 28px 8px; }}
dialog.alert .heading, dialog .title-2 {{ font-size: 22px; font-weight: 600; }}
dialog.alert .body {{ font-size: 17px; color: rgba({text_rgb}, 0.62); }}
dialog.alert .response-area {{ padding: 14px 28px 28px; border-spacing: 10px; }}
dialog.alert .response-area > button {{ min-height: 50px; border-radius: 999px; border: none; font-size: 17px; }}
dialog.alert .response-area > button.destructive {{ background: alpha({danger}, 0.14); color: {danger}; }}
dialog.alert .response-area > button.suggested {{ background: {accent}; color: white; }}
toast {{ border-radius: 999px; background: {elevated}; color: {text}; border: 1px solid rgba({ink}, 0.14); }}
banner > revealer > widget {{ background: alpha({danger}, 0.14); color: {danger}; }}

/* the rail (the bottom bar on a phone) */
.rail {{ background: {rail}; padding: 16px 4px 14px; border-right: 1px solid rgba({ink}, 0.08); }}
.rail.bottom {{ padding: 4px; border-right: none; border-top: 1px solid rgba({ink}, 0.08); }}
.rail button.nav {{ background: transparent; border-radius: 14px; min-height: 0; padding: 6px 0; color: rgba({text_rgb}, 0.62); }}
.rail button.nav:hover {{ color: {text}; }}
.rail button.nav .navtile {{ min-width: 40px; min-height: 40px; border-radius: 12px; }}
.rail button.nav:hover .navtile {{ background: rgba({ink}, 0.06); }}
.rail button.nav.on {{ color: {text}; }}
.rail button.nav.on .navtile {{ background: rgba({ink}, 0.1); }}
.rail .navlabel {{ font-size: 11px; font-weight: 600; letter-spacing: 0; }}
.rail .navdot {{ min-width: 8px; min-height: 8px; border-radius: 999px; border: 2px solid {elevated}; margin: 6px; }}
.rail .navdot.ok {{ background: {ok}; }} .rail .navdot.off {{ background: rgba({text_rgb}, 0.38); }} .rail .navdot.warn {{ background: #ff9f0a; }}
.rail .lock {{ color: rgba({text_rgb}, 0.38); }}

/* app tiles, people, the graph, codes */
.tile {{ border-radius: 10px; color: white; font-weight: 700; font-size: 1.1em; padding: 0; min-height: 0; }}
.tile.blue {{ background: #0a84ff; }} .tile.red {{ background: #ff453a; }} .tile.orange {{ background: #ff9f0a; }}
.tile.green {{ background: #30d158; }} .tile.purple {{ background: #bf5af2; }} .tile.pink {{ background: #ff375f; }}
.tile.teal {{ background: #40c8e0; }} .tile.indigo {{ background: #5e5ce6; }}
.face {{ border-radius: 999px; color: white; font-weight: 600; background-image: linear-gradient(140deg, #30c48d, #2a9df4); }}
.face.mine {{ background-image: none; background: {accent}; }}
.devtile {{ border-radius: 12px; background: {surface2}; color: rgba({text_rgb}, 0.62); }}
.qr {{ border-radius: 16px; padding: 12px; background: white; border: none; }}
.word {{ border-radius: 10px; padding: 8px 12px; font-size: 1.4em; font-weight: 700; background: {surface2}; }}
.word.number {{ background: alpha({accent}, 0.16); color: {accent}; }}
.graph {{ border-radius: 20px; border: 1px solid rgba({ink}, 0.08); background: {low}; }}
.about {{ background: {surface}; border-radius: 18px; border: 1px solid rgba({ink}, 0.08); padding: 16px 18px; }}
.about .kind {{ font-size: 12px; font-weight: 700; color: {accent}; letter-spacing: 0.04em; }}
.about .what {{ font-size: 20px; font-weight: 600; }}
.node {{ padding: 0; min-width: 0; min-height: 0; }}
.node.device {{ border-radius: 30%; background: {surface2}; color: {text}; }}
.node.app, .node.ai {{ border-radius: 28%; }}
.node.person, .node.bubble, .node.fold {{ border-radius: 999px; }}
.node.person {{ background-image: linear-gradient(140deg, #30c48d, #2a9df4); color: white; }}
.node.bubble {{ background: {surface}; border: 1.5px dashed rgba({ink}, 0.35); font-weight: 700; color: {text}; }}
.node.fold {{ background: {surface2}; color: rgba({text_rgb}, 0.62); }}
.node.dotted {{ border: 1.5px dashed rgba({ink}, 0.35); background: none; }}
.node.picked {{ outline: 3px solid {accent}; outline-offset: 2px; }}
"#
    )
}

/// UUOB's line icons (24 point grid, 1.75 stroke), as ui/Icon.svelte drew them.
pub fn icon(name: &str, size: i32) -> gtk::Widget {
    let path = ICONS.iter().find(|(n, _)| *n == name).map(|(_, p)| *p).unwrap_or("");
    let area = gtk::DrawingArea::builder().content_width(size).content_height(size).valign(gtk::Align::Center).halign(gtk::Align::Center).build();
    let path = gtk::gsk::Path::parse(path).ok();
    area.set_draw_func(move |area, cr, w, h| {
        let Some(path) = &path else { return };
        let fg = area.color();
        cr.scale(w as f64 / 24.0, h as f64 / 24.0);
        path.to_cairo(cr);
        cr.set_source_rgba(fg.red() as f64, fg.green() as f64, fg.blue() as f64, fg.alpha() as f64);
        cr.set_line_width(1.75);
        cr.set_line_cap(gtk::cairo::LineCap::Round);
        cr.set_line_join(gtk::cairo::LineJoin::Round);
        cr.stroke().ok();
    });
    area.upcast()
}

const ICONS: &[(&str, &str)] = &[
    ("plus", "M12 5v14M5 12h14"),
    ("trash", "M4 7h16M10 11v6M14 11v6M6 7l1 12a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-12M9 7V4h6v3"),
    ("link", "M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1"),
    ("close", "M6 6l12 12M18 6L6 18"),
    ("copy", "M9 9h10v10a1 1 0 0 1-1 1H10a1 1 0 0 1-1-1zM5 15V5a1 1 0 0 1 1-1h9"),
    ("lock", "M6 11h12v9H6zM8 11V8a4 4 0 0 1 8 0v3"),
    ("chevron", "M9 6l6 6-6 6"),
    ("back", "M15 6l-6 6 6 6"),
    ("edit", "M4 20h4L19 9l-4-4L4 16zM13 7l4 4"),
    ("laptop", "M4 6a1 1 0 0 1 1-1h14a1 1 0 0 1 1 1v9H4zM2 18h20"),
    ("phone", "M8 3h8a1 1 0 0 1 1 1v16a1 1 0 0 1-1 1H8a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zM11 18h2"),
    ("server", "M5 4h14a1 1 0 0 1 1 1v5H4V5a1 1 0 0 1 1-1zM4 14h16v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1zM8 7h.01M8 17h.01"),
    ("question", "M9.1 9a3 3 0 0 1 5.8 1c0 2-3 2.5-3 4.5M12 18h.01"),
    ("settings", "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6zM19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"),
    ("people", "M9 11a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7zM2.5 20a6.5 6.5 0 0 1 13 0M16 4.2a3.5 3.5 0 0 1 0 6.6M18 14.3a6.5 6.5 0 0 1 3.5 5.7"),
    ("ai", "M12 3l1.8 4.7L18.5 9.5l-4.7 1.8L12 16l-1.8-4.7L5.5 9.5l4.7-1.8zM19 15l.8 2.2 2.2.8-2.2.8L19 21l-.8-2.2L16 18l2.2-.8z"),
    ("network", "M14.5 5a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0zM7.5 18a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0zM21.5 18a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0zM10.8 7.2L6.2 15.8M13.2 7.2l4.6 8.6M7.5 18h9"),
    ("shield", "M12 3l7 3v5c0 4.5-3 8.3-7 10-4-1.7-7-5.5-7-10V6zM9 12l2 2 4-4"),
];
