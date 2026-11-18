//! Computers: apps' alerts (an event in ten minutes, a task that is due),
//! shown by UUOB as notifications at their time, from what is coming up in
//! the user's apps (see `Node::upcoming`). Android rings them in `Alerts.kt`.

use jiff::{Timestamp, ToSpan, Zoned, civil, tz::TimeZone};
use serde_json::Value as Json;
use std::time::Duration;

/// An alert found more than this late (the computer was asleep or off) is skipped.
const STALE: i64 = 60 * 60;

/// Rings each alert at its time, for as long as UUOB runs.
pub fn ring() {
    #[cfg(target_os = "macos")]
    let _ = notify_rust::set_application("app.uuob"); // UUOB's name and icon on its notifications
    crate::runtime().spawn(async move {
        let mut rx = crate::upcoming_now().subscribe();
        // What was due before UUOB started is not rung any more.
        let since = Timestamp::now();
        // What rang (app/id at a moment), for as long as it could ring again.
        let mut rung = std::collections::HashSet::<(String, Timestamp)>::new();
        loop {
            let now = Timestamp::now();
            let (items, hour12) = {
                let v: Json = serde_json::from_str(&rx.borrow_and_update()).unwrap_or_default();
                (v["items"].as_array().cloned().unwrap_or_default(), v["hour12"].as_bool().unwrap_or(false))
            };
            let mut next = None::<Timestamp>;
            rung.retain(|(_, at)| (now - *at).get_seconds() < STALE);
            for item in &items {
                let Some(at) = rings_at(item) else { continue };
                // Also one that arrives a little late from another device.
                let key = format!("{}/{}", item["app"].as_str().unwrap_or_default(), item["id"].as_str().unwrap_or_default());
                if at > since && at <= now && (now - at).get_seconds() < STALE && rung.insert((key, at)) {
                    show(item, hour12);
                }
                if at > now {
                    next = Some(next.map_or(at, |n| n.min(at)));
                }
            }
            // Short naps: a computer's sleep pauses timers, the clock goes on.
            let wait = next.map_or(30.0, |n| (n - now).total(jiff::Unit::Second).unwrap_or(30.0).clamp(0.05, 30.0));
            tokio::select! {
                _ = rx.changed() => {}
                _ = tokio::time::sleep(Duration::from_secs_f64(wait)) => {}
            }
        }
    });
}

/// When `item` starts: `date` `time` on the wall clock of `tz`, or of this computer.
fn start(item: &Json) -> Option<Zoned> {
    let date: civil::Date = item["date"].as_str()?.parse().ok()?;
    let time: civil::Time = item["time"].as_str()?.parse().ok()?;
    let tz = item["tz"].as_str().and_then(|t| TimeZone::get(t).ok()).unwrap_or_else(TimeZone::system);
    date.to_datetime(time).to_zoned(tz).ok()
}

/// When `item` ends (`end_date` `end`; past midnight when it ends "before" it starts).
fn end(item: &Json, start: &Zoned) -> Option<Zoned> {
    let time: civil::Time = item["end"].as_str()?.parse().ok()?;
    let date = match item["end_date"].as_str() {
        Some(d) => d.parse().ok()?,
        None if time <= start.time() => start.date().checked_add(1.day()).ok()?,
        None => start.date(),
    };
    date.to_datetime(time).to_zoned(start.time_zone().clone()).ok()
}

/// When to ring for `item`: `alert` minutes before it starts, if it asks for that.
fn rings_at(item: &Json) -> Option<Timestamp> {
    let before = item["alert"].as_i64()?;
    start(item)?.timestamp().checked_sub((before * 60).seconds()).ok()
}

/// "9:30–10:00 · Room 2": when (on this computer's clock) and the app's second line.
fn text(item: &Json, hour12: bool) -> String {
    let here = TimeZone::system();
    let clock = |z: Zoned| {
        let z = z.with_time_zone(here.clone());
        let (h, m) = (z.hour(), z.minute());
        if hour12 { format!("{}:{m:02} {}", (h + 11) % 12 + 1, if h < 12 { "AM" } else { "PM" }) } else { format!("{h}:{m:02}") }
    };
    let when = start(item).map(|s| match end(item, &s) {
        Some(e) => format!("{}–{}", clock(s), clock(e)),
        None => clock(s),
    });
    [when, item["body"].as_str().filter(|b| !b.is_empty()).map(String::from)].into_iter().flatten().collect::<Vec<_>>().join(" · ")
}

fn show(item: &Json, hour12: bool) {
    let title = item["title"].as_str().unwrap_or_default().to_string();
    let app = item["app"].as_str().unwrap_or_default().to_string();
    let mut n = notify_rust::Notification::new();
    n.summary(&title).subtitle(item["app_name"].as_str().unwrap_or_default()).body(&text(item, hour12)).appname("UUOB");
    // Waits for a click on its own thread; a click opens the app the alert came from.
    std::thread::spawn(move || {
        let Ok(shown) = n.show() else { return };
        #[cfg(target_os = "macos")]
        shown.wait_for_action(|action| {
            if action == "default" {
                // UUOB's apps are app.uuob.<id>; not on this Mac, nothing opens.
                let _ = std::process::Command::new("open").args(["-b", &format!("app.uuob.{app}")]).status();
            }
        });
        #[cfg(not(target_os = "macos"))]
        let _ = (shown, app);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn when_alerts_ring_and_what_they_say() {
        let item = json!({"date": "2030-01-03", "time": "09:30", "end": "10:00", "tz": "Europe/Prague", "title": "Call", "body": "Room 2", "alert": 10});
        assert_eq!(rings_at(&item), Some("2030-01-03T08:20:00Z".parse().unwrap()));
        // Over midnight; no alert asked for.
        let late = json!({"date": "2030-01-03", "time": "22:00", "end": "01:00", "tz": "UTC", "title": "Party"});
        assert_eq!(end(&late, &start(&late).unwrap()).unwrap().to_string(), "2030-01-04T01:00:00+00:00[UTC]");
        assert_eq!(rings_at(&late), None);
        // An item without a time (a task due that day) has nothing to ring.
        assert_eq!(rings_at(&json!({"date": "2030-01-03", "title": "Bank", "alert": 0})), None);
        // The text is on this computer's clock.
        if TimeZone::system() == TimeZone::UTC {
            assert_eq!(text(&item, false), "8:30–9:00 · Room 2");
            assert_eq!(text(&item, true), "8:30 AM–9:00 AM · Room 2");
            assert_eq!(text(&json!({"date": "2030-01-03", "time": "13:05", "tz": "UTC"}), true), "1:05 PM");
        }
    }
}
