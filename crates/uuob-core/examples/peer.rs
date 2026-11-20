//! A headless device for trying sync from the command line.
//!   cargo run -p uuob-core --example peer -- <data-dir> invite        # prints a code
//!   cargo run -p uuob-core --example peer -- <data-dir> join <code>
//!   cargo run -p uuob-core --example peer -- <data-dir> run            # already linked
//! Then it adds a task and prints the task list whenever it changes.

use std::time::Duration;

use serde_json::json;
use uuob_core::*;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let node = Node::open(Config {
        data_dir: args[1].clone().into(),
        device_name: "Headless peer".into(),
        platform: "linux".into(),
        net: NetMode::Internet,
    })
    .await
    .unwrap();
    node.install_app(&AppManifest { id: "tasks".into(), name: "Tasks".into() }).unwrap();
    let mut events = node.subscribe();
    match args[2].as_str() {
        "invite" => {
            let inv = node.create_invite().await.unwrap();
            println!("CODE {}", inv.code);
            loop {
                if let Ok(Event::DeviceJoined { name, .. }) = events.recv().await {
                    println!("JOINED {name}");
                    break;
                }
            }
        }
        "join" => {
            node.join(&args[3]).await.unwrap();
            println!("JOINED");
        }
        "run" => println!("RUNNING"), // already linked earlier
        _ => panic!("invite | join <code> | run"),
    }
    tokio::time::sleep(Duration::from_secs(3)).await;
    let t = Client::App("tasks".into());
    node.put(&t, "tasks", "tasks", None, json!({"title": format!("Hello from the desktop ({})", args[2]), "done": false, "due": null, "created": 1, "notes": ""})).unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(240);
    let mut last = String::new();
    while tokio::time::Instant::now() < deadline {
        let mut titles: Vec<String> = node.list(&t, "tasks", "tasks").unwrap().iter().map(|r| r["title"].as_str().unwrap_or("").to_string()).collect();
        titles.sort();
        let now = format!("{} peers online | tasks: {:?}", node.online_peers().len(), titles);
        if now != last {
            println!("{now}");
            last = now;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    node.shutdown().await;
}
