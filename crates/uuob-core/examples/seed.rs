//! Fill a data dir with demo data (for screenshots and trying the UI).
//!   cargo run -p uuob-core --example seed -- <data-dir>

use serde_json::json;
use uuob_core::*;

fn day(offset: i64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let days = now / 86400 + offset;
    // civil-from-days (Howard Hinnant)
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

#[tokio::main]
async fn main() {
    let dir = std::env::args().nth(1).expect("data dir");
    let net = MemoryLookup::new();
    let open = |dir: std::path::PathBuf, name: &str, platform: &str| {
        Node::open(Config {
            data_dir: dir,
            device_name: name.into(),
            platform: platform.into(),
            net: NetMode::Local(net.clone()),
            keystore: None,
        })
    };
    let node = open(dir.into(), "Alex’s Laptop", "linux").await.unwrap();
    node.install_app(&AppManifest { id: "tasks".into(), name: "Tasks".into() }).unwrap();
    node.install_app(&AppManifest { id: "calendar".into(), name: "Calendar".into() }).unwrap();
    let t = Client::App("tasks".into());
    let c = Client::App("calendar".into());

    // Every list and every added calendar is a container of its own.
    let mut place = std::collections::HashMap::new();
    for (key, app, name, color, n) in [
        ("home", "tasks", "Home", "orange", 1),
        ("work", "tasks", "Work", "blue", 2),
        ("groceries", "tasks", "Groceries", "green", 3),
        ("work-cal", "calendar", "Work", "purple", 1),
        ("sport", "calendar", "Sport", "green", 2),
    ] {
        let id = node.create_container(app, name, None).unwrap();
        let (client, info) = if app == "tasks" { (&t, "list") } else { (&c, "calendar") };
        node.put(client, &id, info, Some("info"), json!({"color": color, "created": n})).unwrap();
        place.insert(key, id);
    }
    let list = |l: Option<&str>| l.map_or("tasks", |l| place[l].as_str());
    let tasks = [
        ("Call mom", Some(0), Some("17:00"), Some("home"), ""),
        ("Send invoice to Studio Brno", Some(0), None, Some("work"), "November hours + hosting"),
        ("Renew domain example.com", Some(-1), None, Some("work"), ""),
        ("Oat milk", None, None, Some("groceries"), ""),
        ("Avocados", None, None, Some("groceries"), ""),
        ("Fix 3D printer bed leveling", Some(2), None, Some("home"), ""),
        ("Review differential geometry ch. 4", Some(1), Some("09:00"), None, "Curvature of surfaces"),
        ("Book dentist", Some(5), None, Some("home"), ""),
        ("Ship ESP32 firmware update", Some(3), Some("14:00"), Some("work"), ""),
    ];
    for (i, (title, due, time, l, notes)) in tasks.iter().enumerate() {
        node.put(&t, list(*l), "tasks", None, json!({
            "title": title, "notes": notes, "done": false, "due": due.map(day), "time": time, "created": i,
        })).unwrap();
    }
    node.put(&t, list(Some("home")), "tasks", None, json!({"title": "Pay rent", "done": true, "done_at": 1, "due": day(-2), "created": 99, "notes": ""})).unwrap();

    node.put(&c, "calendar", "calendars", Some("personal"), json!({"name": "Personal", "color": "blue"})).unwrap();
    let cal = |k: &str| if k == "personal" { "calendar" } else if k == "work" { place["work-cal"].as_str() } else { place[k].as_str() };
    let events = [
        ("Standup", 0, "09:30", "09:45", "work", ""),
        ("Client call — Studio Brno", 0, "11:00", "12:00", "work", "Google Meet"),
        ("Lunch with Petr", 0, "12:30", "13:30", "personal", "Lokál"),
        ("Climbing", 1, "18:00", "20:00", "sport", "Boulder Bar"),
        ("Design review", 1, "10:00", "11:30", "work", ""),
        ("Pair programming", 1, "10:30", "12:00", "work", ""),
        ("Run", -1, "07:00", "08:00", "sport", "Stromovka"),
        ("Group theory study", 2, "16:00", "18:00", "personal", "Library"),
        ("Standup", 2, "09:30", "09:45", "work", ""),
        ("Dinner with family", 3, "19:00", "21:30", "personal", ""),
        ("Deploy window", -2, "14:00", "15:30", "work", ""),
        ("Run", 3, "07:00", "08:00", "sport", ""),
    ];
    for (title, d, s, e, k, loc) in events {
        node.put(&c, cal(k), "events", None, json!({
            "title": title, "date": day(d), "all_day": false, "start": s, "end": e, "location": loc, "notes": "",
        })).unwrap();
    }
    node.put(&c, "calendar", "events", None, json!({"title": "Prague trip", "date": day(4), "all_day": true, "location": "", "notes": "", "start": "09:00", "end": "10:00"})).unwrap();
    let (id, _) = node.create_client("Claude on my laptop").unwrap();
    node.set_grant(&format!("ai:{id}"), "tasks", Some(Access::Write)).unwrap();
    node.set_grant(&format!("ai:{id}"), "calendar", Some(Access::Read)).unwrap();

    // Someone to share with: Petra gets the groceries list, and offers a list of hers.
    let petra_dir = std::env::temp_dir().join(format!("uuob-seed-petra-{}", std::process::id()));
    let petra = open(petra_dir.clone(), "Petra’s phone", "android").await.unwrap();
    node.set_name("Alex").unwrap();
    petra.set_name("Petra").unwrap();
    petra.install_app(&AppManifest { id: "tasks".into(), name: "Tasks".into() }).unwrap();
    let trip = petra.create_container("tasks", "Trip to Vienna", None).unwrap();
    petra.put(&t, &trip, "tasks", None, json!({"title": "Book the train", "done": false, "created": 1, "notes": ""})).unwrap();
    let invite = node.create_person_invite(Some((place["groceries"].clone(), Access::Write))).await.unwrap();
    petra.meet(&invite.code).await.unwrap();
    let me = petra.people()[0].id.clone();
    petra.share(&trip, &me, Some(Access::Write)).unwrap();
    for _ in 0..100 {
        if node.people().first().is_some_and(|p| !p.receiving.is_empty()) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    // A second device that keeps everything, and a file in the Home list.
    let server_dir = std::env::temp_dir().join(format!("uuob-seed-server-{}", std::process::id()));
    let server = open(server_dir.clone(), "Home server", "linux").await.unwrap();
    let invite = node.create_invite().await.unwrap();
    server.join(&invite.code).await.unwrap();
    node.set_keeper(&server.id().to_string(), true).unwrap();
    let photo = node.put_file(&t, &place["home"], "image/jpeg", &vec![7u8; 400_000]).await.unwrap();
    for _ in 0..100 {
        if server.files(&Client::Shell, &place["home"]).is_ok_and(|fs| fs.iter().any(|f| f.id == photo.id && f.here)) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    server.shutdown().await;
    let _ = std::fs::remove_dir_all(server_dir);

    // Photos, if a folder of pictures was given (UUOB_SEED_PHOTOS): every
    // `x.jpg` with its small preview `x.thumb.jpg`, stored the way the Photos
    // app stores them (one container per month).
    if let Some(dir) = std::env::var_os("UUOB_SEED_PHOTOS") {
        node.install_app(&AppManifest { id: "photos".into(), name: "Photos".into() }).unwrap();
        let p = Client::App("photos".into());
        let mut pictures: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).collect();
        pictures.sort();
        pictures.retain(|f| f.extension().is_some_and(|e| e == "jpg") && !f.to_string_lossy().ends_with(".thumb.jpg"));
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
        for (i, picture) in pictures.iter().enumerate() {
            // Spread over the last weeks, newest first.
            let taken = now - i as i64 * 3 * 86_400_000 - i as i64 * 5_000_000;
            let month = day((taken - now) / 86_400_000)[..7].to_string();
            let container = node.create_container("photos", &month, Some(&format!("month{}00000", month.replace('-', "")))).unwrap();
            let bytes = std::fs::read(picture).unwrap();
            let file = node.put_file(&p, &container, "image/jpeg", &bytes).await.unwrap();
            let thumb = std::fs::read(picture.with_extension("thumb.jpg")).unwrap();
            let thumb = format!("data:image/jpeg;base64,{}", data_encoding::BASE64.encode(&thumb));
            node.put(&p, &container, "photos", None, json!({
                "taken": taken, "added": now, "name": picture.file_name().unwrap().to_string_lossy(),
                "type": "image/jpeg", "size": bytes.len(), "file": file.id, "w": 1600, "h": 1200, "thumb": thumb,
            })).unwrap();
        }
    }

    // A token for trying the local API by hand (e.g. the share sheet).
    let token = node.connect_app(&AppManifest { id: "tasks".into(), name: "Tasks".into() }, Some(&node.create_app_code().await.unwrap().code)).await.unwrap();
    println!("token {token}\ngroceries {}", place["groceries"]);
    petra.shutdown().await;
    node.shutdown().await;
    let _ = std::fs::remove_dir_all(petra_dir);
    println!("seeded {}", node.status().device_id);
}
