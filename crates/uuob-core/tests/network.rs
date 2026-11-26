//! End-to-end tests: real nodes, real QUIC connections (loopback, no relays).

use std::time::Duration;

use serde_json::json;
use tempfile::TempDir;
use uuob_core::*;

fn manifest(id: &str, name: &str) -> AppManifest {
    AppManifest { id: id.into(), name: name.into() }
}

async fn node(dir: &TempDir, name: &str, net: &MemoryLookup) -> Node {
    Node::open(Config {
        data_dir: dir.path().to_path_buf(),
        device_name: name.into(),
        platform: "linux".into(),
        net: NetMode::Local(net.clone()),
    })
    .await
    .unwrap()
}

/// Poll until `f` is true (sync is asynchronous).
async fn eventually(what: &str, mut f: impl FnMut() -> bool) {
    for _ in 0..200 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("timed out waiting for: {what}");
}

async fn pair(a: &Node, b: &Node) {
    let invite = a.create_invite().await.unwrap();
    b.join(&invite.code.to_uppercase().replace('-', " ")).await.unwrap();
    assert!(a.wait_for_peer(b.id(), Duration::from_secs(10)).await, "peers connect");
}

fn tasks() -> Client {
    Client::App("tasks".into())
}

#[tokio::test]
async fn pair_and_sync_records() {
    let net = MemoryLookup::new();
    let (da, db) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let a = node(&da, "Laptop", &net).await;
    let b = node(&db, "Phone", &net).await;

    // Data created before linking is kept and merged.
    b.install_app(&manifest("tasks", "Tasks")).unwrap();
    b.put(&tasks(), "tasks", "tasks", Some("from-b"), json!({"title": "made on phone"})).unwrap();

    a.install_app(&manifest("tasks", "Tasks")).unwrap();
    pair(&a, &b).await;
    assert_eq!(a.devices().len(), 2);

    a.put(&tasks(), "tasks", "tasks", Some("t1"), json!({"title": "Buy milk", "done": false})).unwrap();
    eventually("task reaches B", || b.get(&tasks(), "tasks", "tasks", "t1").unwrap().is_some()).await;
    eventually("B's old task reaches A", || a.get(&tasks(), "tasks", "tasks", "from-b").unwrap().is_some()).await;

    // Edit on B, see it on A.
    b.put(&tasks(), "tasks", "tasks", Some("t1"), json!({"title": "Buy milk", "done": true})).unwrap();
    eventually("edit reaches A", || {
        a.get(&tasks(), "tasks", "tasks", "t1").unwrap().unwrap()["done"] == json!(true)
    })
    .await;

    // Deletes sync too.
    a.delete(&tasks(), "tasks", "tasks", "t1").unwrap();
    eventually("delete reaches B", || b.get(&tasks(), "tasks", "tasks", "t1").unwrap().is_none()).await;
}

#[tokio::test]
async fn wrong_code_is_rejected_and_burned() {
    let net = MemoryLookup::new();
    let (da, db) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let a = node(&da, "A", &net).await;
    let b = node(&db, "B", &net).await;
    // A code nobody is showing.
    assert!(b.join("7-amber-river-falcon").await.is_err());
    // A used code cannot be used twice.
    let inv = a.create_invite().await.unwrap();
    b.join(&inv.code).await.unwrap();
    let dc = TempDir::new().unwrap();
    let c = node(&dc, "C", &net).await;
    assert!(c.join(&inv.code).await.is_err());
    assert_eq!(a.devices().len(), 2);
}

#[tokio::test]
async fn permissions_are_default_deny() {
    let net = MemoryLookup::new();
    let d = TempDir::new().unwrap();
    let n = node(&d, "A", &net).await;
    n.install_app(&manifest("tasks", "Tasks")).unwrap();
    n.install_app(&manifest("calendar", "Calendar")).unwrap();
    let cal = Client::App("calendar".into());
    n.put(&tasks(), "tasks", "tasks", Some("t1"), json!({"title": "x"})).unwrap();

    // Calendar cannot see Tasks data.
    assert!(matches!(n.list(&cal, "tasks", "tasks"), Err(Error::PermissionDenied(_))));
    // Read grant: read yes, write no.
    n.set_grant("app:calendar", "tasks", Some(Access::Read)).unwrap();
    assert_eq!(n.list(&cal, "tasks", "tasks").unwrap().len(), 1);
    assert!(n.put(&cal, "tasks", "tasks", None, json!({})).is_err());
    // Revoke works immediately.
    n.set_grant("app:calendar", "tasks", None).unwrap();
    assert!(n.list(&cal, "tasks", "tasks").is_err());

    // AI clients start with nothing and see only granted containers.
    let (id, token) = n.create_client("Claude").unwrap();
    let ai = n.authenticate(&token).unwrap();
    assert!(n.authenticate("uuob_wrong").is_none());
    assert!(n.containers(&ai).unwrap().is_empty());
    n.set_grant(&format!("ai:{id}"), "tasks", Some(Access::Write)).unwrap();
    let visible: Vec<_> = n.containers(&ai).unwrap().into_iter().map(|c| c.id).collect();
    assert_eq!(visible, vec!["tasks"]);
    n.put(&ai, "tasks", "tasks", None, json!({"title": "from AI"})).unwrap();
    assert!(n.list(&ai, "calendar", "events").is_err());
    n.delete_client(&id).unwrap();
    assert!(n.authenticate(&token).is_none());
}

#[tokio::test]
async fn uninstall_with_data_deletes_everywhere() {
    let net = MemoryLookup::new();
    let (da, db) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let a = node(&da, "A", &net).await;
    let b = node(&db, "B", &net).await;
    a.install_app(&manifest("tasks", "Tasks")).unwrap();
    pair(&a, &b).await;
    a.put(&tasks(), "tasks", "tasks", Some("t1"), json!({"title": "secret"})).unwrap();
    eventually("sync", || b.get(&Client::Shell, "tasks", "tasks", "t1").ok().flatten().is_some()).await;

    // Keep-data uninstall: app loses access, data stays.
    a.uninstall_app("tasks", false).unwrap();
    assert!(a.get(&tasks(), "tasks", "tasks", "t1").is_err());
    assert!(a.get(&Client::Shell, "tasks", "tasks", "t1").unwrap().is_some());

    // Delete data: gone on both devices.
    a.delete_container("tasks").unwrap();
    eventually("container gone on B", || b.containers(&Client::Shell).unwrap().is_empty()).await;

    // Reinstall starts empty; old data never comes back.
    b.install_app(&manifest("tasks", "Tasks")).unwrap();
    eventually("reinstall syncs", || a.apps()["tasks"].installed).await;
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(a.list(&tasks(), "tasks", "tasks").unwrap().is_empty());
    assert!(b.list(&tasks(), "tasks", "tasks").unwrap().is_empty());
}

#[tokio::test]
async fn offline_edits_merge() {
    let net = MemoryLookup::new();
    let (da, db) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let a = node(&da, "A", &net).await;
    let b = node(&db, "B", &net).await;
    a.install_app(&manifest("tasks", "Tasks")).unwrap();
    pair(&a, &b).await;
    a.put(&tasks(), "tasks", "tasks", Some("t1"), json!({"title": "Plan trip", "done": false})).unwrap();
    eventually("sync", || b.get(&tasks(), "tasks", "tasks", "t1").ok().flatten().is_some()).await;

    // B goes offline; both edit different fields of the same task.
    b.shutdown().await;
    drop(b);
    a.put(&tasks(), "tasks", "tasks", Some("t1"), json!({"title": "Plan trip", "done": true})).unwrap();
    let b = node(&db, "B", &net).await; // restart from disk
    // B edits before it reconnects... (it may already be connecting; both orders must merge)
    b.put(&tasks(), "tasks", "tasks", Some("t1"), json!({"title": "Plan trip to Prague", "done": false})).unwrap_or_default();

    eventually("converged", || {
        let x = a.get(&tasks(), "tasks", "tasks", "t1").unwrap();
        let y = b.get(&tasks(), "tasks", "tasks", "t1").unwrap();
        x.is_some() && x == y && x.as_ref().unwrap()["title"] == json!("Plan trip to Prague")
    })
    .await;
}

#[tokio::test]
async fn removed_device_is_cut_off() {
    let net = MemoryLookup::new();
    let (da, db) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let a = node(&da, "A", &net).await;
    let b = node(&db, "B", &net).await;
    a.install_app(&manifest("tasks", "Tasks")).unwrap();
    pair(&a, &b).await;
    a.remove_device(&b.id().to_string()).unwrap();
    eventually("B resets to its own network", || b.devices().len() == 1).await;
    eventually("A drops B", || a.online_peers().is_empty() && a.devices().len() == 1).await;
    a.put(&tasks(), "tasks", "tasks", Some("after"), json!({"title": "not for B"})).unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(b.get(&Client::Shell, "tasks", "tasks", "after").unwrap_or(None).is_none());
}

/// Real-world path: public relays + DNS/pkarr discovery. Needs internet.
/// Run with `cargo test -- --ignored internet`.
#[tokio::test]
#[ignore]
async fn internet_pairing_and_sync() {
    let open = |dir: &TempDir, name: &str| {
        Node::open(Config {
            data_dir: dir.path().to_path_buf(),
            device_name: name.into(),
            platform: "linux".into(),
            net: NetMode::Internet,
        })
    };
    let (da, db) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let a = open(&da, "A").await.unwrap();
    let b = open(&db, "B").await.unwrap();
    a.install_app(&manifest("tasks", "Tasks")).unwrap();
    let t0 = std::time::Instant::now();
    let inv = a.create_invite().await.unwrap();
    b.join(&inv.code).await.unwrap();
    println!("joined in {:?}", t0.elapsed());
    assert!(a.wait_for_peer(b.id(), Duration::from_secs(30)).await);
    a.put(&tasks(), "tasks", "tasks", Some("t1"), json!({"title": "over the internet"})).unwrap();
    eventually("sync", || b.get(&tasks(), "tasks", "tasks", "t1").ok().flatten().is_some()).await;
    println!("synced in {:?}", t0.elapsed());
}

#[tokio::test]
async fn dispatcher_enforces_roles() {
    let net = MemoryLookup::new();
    let d = TempDir::new().unwrap();
    let n = node(&d, "A", &net).await;
    n.install_app(&manifest("tasks", "Tasks")).unwrap();
    // Apps are installed by connecting, not through the dispatcher.
    assert!(n.call(&Client::Shell, "install_app", json!({"id": "x", "name": "X"})).await.is_err());
    // An app writes to its own container without naming it.
    let rec = n.call(&tasks(), "put", json!({"collection": "tasks", "value": {"title": "x"}})).await.unwrap();
    assert!(rec["id"].is_string());
    assert_eq!(n.call(&tasks(), "list", json!({"collection": "tasks"})).await.unwrap().as_array().unwrap().len(), 1);
    // Apps and AI can't call shell methods.
    assert!(n.call(&tasks(), "create_client", json!({"name": "x"})).await.is_err());
    assert!(n.call(&tasks(), "set_grant", json!({"client": "app:tasks", "container": "tasks", "access": "write"})).await.is_err());
    // AI must name a container.
    let t = n.call(&Client::Shell, "create_client", json!({"name": "Claude"})).await.unwrap();
    let ai = n.authenticate(t["token"].as_str().unwrap()).unwrap();
    assert!(n.call(&ai, "list", json!({"collection": "tasks"})).await.is_err());
    // An AI with no access changes nothing outside its data, not even settings.
    assert!(n.call(&ai, "set_settings", json!({"hour12": true})).await.is_err());
    assert!(n.call(&ai, "settings", json!({})).await.is_err());
    assert!(n.call(&ai, "keep", json!({"container": "tasks", "keep": "none"})).await.is_err());
    assert!(n.call(&tasks(), "settings", json!({})).await.is_ok(), "apps still may");
}

#[tokio::test]
async fn rename_survives_restart() {
    let net = MemoryLookup::new();
    let d = TempDir::new().unwrap();
    let n = node(&d, "hostname", &net).await;
    n.set_device_name("Work laptop").unwrap();
    n.shutdown().await;
    drop(n);
    let n = node(&d, "hostname", &net).await;
    assert_eq!(n.status().device_name, "Work laptop");
    assert_eq!(n.devices()[0].name, "Work laptop");
}

// ---------------------------------------------------------------------------
// Sharing with other people
// ---------------------------------------------------------------------------

/// A one-device network belonging to `owner`, with Tasks installed.
async fn person(dir: &TempDir, owner: &str, net: &MemoryLookup) -> Node {
    let n = node(dir, &format!("{owner}'s phone"), net).await;
    n.set_name(owner).unwrap();
    n.install_app(&manifest("tasks", "Tasks")).unwrap();
    n
}

/// Link two people. Returns (b's id as a sees it, a's id as b sees it).
async fn meet(a: &Node, b: &Node) -> (String, String) {
    let invite = a.create_person_invite(None).await.unwrap();
    b.meet(&invite.code).await.unwrap();
    assert_eq!((a.people().len(), b.people().len()), (1, 1));
    assert!(a.wait_for_peer(b.id(), Duration::from_secs(10)).await, "people connect");
    (a.people()[0].id.clone(), b.people()[0].id.clone())
}

fn has(n: &Node, container: &str, id: &str) -> bool {
    n.get(&tasks(), container, "tasks", id).ok().flatten().is_some()
}

#[tokio::test]
async fn people_share_one_list_and_nothing_else() {
    let net = MemoryLookup::new();
    let dirs: Vec<TempDir> = (0..4).map(|_| TempDir::new().unwrap()).collect();
    let a1 = person(&dirs[0], "Alice", &net).await;
    let a2 = node(&dirs[1], "Alice's laptop", &net).await;
    let b1 = person(&dirs[2], "Bob", &net).await;
    let b2 = node(&dirs[3], "Bob's laptop", &net).await;
    pair(&a1, &a2).await;
    pair(&b1, &b2).await;

    a1.put(&tasks(), "tasks", "tasks", Some("private"), json!({"title": "Alice only"})).unwrap();
    let list = a1.create_container("tasks", "Groceries", None).unwrap();
    a1.put(&tasks(), &list, "tasks", Some("milk"), json!({"title": "Milk"})).unwrap();

    // Moving old data over: the same key gives the same container, on any device.
    let moved = a1.create_container("tasks", "Old list", Some("abcdefghij234567")).unwrap();
    assert_eq!(a1.create_container("tasks", "Old list", Some("abcdefghij234567")).unwrap(), moved);
    assert!(a1.create_container("tasks", "x", Some("short")).is_err());

    // Linking shares nothing.
    let (bob, alice) = meet(&a1, &b1).await;
    assert_eq!(a1.people()[0].name, "Bob");
    assert_eq!(b1.people()[0].name, "Alice");
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(b1.people()[0].receiving.is_empty());

    // The app's first container can't be shared; a list can.
    assert!(a1.share("tasks", &bob, Some(Access::Write)).is_err());
    a1.share(&list, &bob, Some(Access::Write)).unwrap();
    eventually("Bob is offered the list", || b1.people()[0].receiving.iter().any(|s| s.offered && s.name == "Groceries")).await;
    // Until Bob accepts, nothing is stored on his side.
    assert!(!has(&b1, &list, "milk"));
    assert!(b1.containers(&tasks()).unwrap().iter().all(|c| c.id != list));

    b1.accept_share(&list).unwrap();
    eventually("the list reaches both of Bob's devices", || has(&b1, &list, "milk") && has(&b2, &list, "milk")).await;
    let seen = b1.containers(&tasks()).unwrap().into_iter().find(|c| c.id == list).unwrap();
    assert_eq!(seen.from.as_ref().map(|p| (p.id.as_str(), p.name.as_str())), Some((alice.as_str(), "Alice")));
    assert!(seen.can_write);
    let shared: Vec<_> = a1.containers(&tasks()).unwrap().into_iter().find(|c| c.id == list).unwrap().shared_with;
    assert_eq!(shared.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), vec!["Bob"]);

    // Edits flow both ways, to all four devices.
    b2.put(&tasks(), &list, "tasks", Some("eggs"), json!({"title": "Eggs"})).unwrap();
    eventually("Bob's edit reaches Alice's devices", || has(&a1, &list, "eggs") && has(&a2, &list, "eggs")).await;
    a2.put(&tasks(), &list, "tasks", Some("tea"), json!({"title": "Tea"})).unwrap();
    eventually("Alice's edit reaches Bob's devices", || has(&b1, &list, "tea") && has(&b2, &list, "tea")).await;

    // Alice's private data never left her devices.
    assert!(b1.get(&Client::Shell, "tasks", "tasks", "private").unwrap().is_none());
    assert!(b2.get(&Client::Shell, "tasks", "tasks", "private").unwrap().is_none());
    // Bob can't pass it on.
    assert!(b1.share(&list, &alice, Some(Access::Read)).is_err());
}

#[tokio::test]
async fn view_only_is_enforced_by_the_owner_and_sharing_can_stop() {
    let net = MemoryLookup::new();
    let (da, db) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let a = person(&da, "Alice", &net).await;
    let b = person(&db, "Bob", &net).await;
    let (bob, _) = meet(&a, &b).await;
    let list = a.create_container("tasks", "Trip", None).unwrap();
    a.put(&tasks(), &list, "tasks", Some("t1"), json!({"title": "Book train"})).unwrap();
    a.share(&list, &bob, Some(Access::Write)).unwrap();
    eventually("offered", || b.people()[0].receiving.iter().any(|s| s.offered)).await;
    b.accept_share(&list).unwrap();
    eventually("synced", || has(&b, &list, "t1")).await;

    // Alice goes offline and Bob keeps editing. Before they reconnect, Alice
    // makes the list view-only: her devices must not take that edit.
    a.shutdown().await;
    drop(a);
    b.put(&tasks(), &list, "tasks", Some("late"), json!({"title": "Too late"})).unwrap();
    let a = node(&da, "Alice's phone", &net).await;
    a.share(&list, &bob, Some(Access::Read)).unwrap();
    eventually("Bob learns it is view-only", || {
        b.containers(&tasks()).unwrap().iter().any(|c| c.id == list && !c.can_write)
    })
    .await;
    assert!(matches!(b.put(&tasks(), &list, "tasks", None, json!({"title": "no"})), Err(Error::PermissionDenied(_))));
    // He still receives Alice's changes…
    a.put(&tasks(), &list, "tasks", Some("t2"), json!({"title": "Pack"})).unwrap();
    eventually("Alice's edit reaches Bob", || has(&b, &list, "t2")).await;
    // …but his never reached her.
    assert!(!has(&a, &list, "late"));

    // Stop sharing: Bob's copy goes away.
    a.share(&list, &bob, None).unwrap();
    eventually("Bob's copy is removed", || b.containers(&Client::Shell).unwrap().iter().all(|c| c.id != list)).await;
    assert!(b.people()[0].receiving.is_empty());
    assert!(a.people()[0].sharing.is_empty());
    a.put(&tasks(), &list, "tasks", Some("t3"), json!({"title": "Not for Bob"})).unwrap();
    tokio::time::sleep(Duration::from_millis(700)).await;
    assert!(b.get(&Client::Shell, &list, "tasks", "t3").unwrap_or(None).is_none());

    // Sharing it again is a new offer.
    a.share(&list, &bob, Some(Access::Read)).unwrap();
    eventually("offered again", || b.people()[0].receiving.iter().any(|s| s.offered)).await;
    // Declining tells Alice, who then no longer shares it.
    b.delete_container(&list).unwrap();
    eventually("Alice sees Bob declined", || a.people()[0].sharing.is_empty()).await;
}

#[tokio::test]
async fn a_persons_devices_come_and_go() {
    let net = MemoryLookup::new();
    let (da, db1, db2) = (TempDir::new().unwrap(), TempDir::new().unwrap(), TempDir::new().unwrap());
    let a = person(&da, "Alice", &net).await;
    let b1 = person(&db1, "Bob", &net).await;
    let (bob, _) = meet(&a, &b1).await;
    let list = a.create_container("tasks", "Home", None).unwrap();
    a.share(&list, &bob, Some(Access::Write)).unwrap();
    eventually("offered", || b1.people()[0].receiving.iter().any(|s| s.offered)).await;
    b1.accept_share(&list).unwrap();

    // Bob adds a laptop later. Alice's phone learns about it and talks to it directly.
    let b2 = node(&db2, "Bob's laptop", &net).await;
    pair(&b1, &b2).await;
    assert!(a.wait_for_peer(b2.id(), Duration::from_secs(10)).await, "Alice connects to Bob's new device");
    b1.shutdown().await;
    b2.put(&tasks(), &list, "tasks", Some("from-laptop"), json!({"title": "x"})).unwrap();
    eventually("the laptop's edit reaches Alice without Bob's phone", || has(&a, &list, "from-laptop")).await;

    // Bob removes the laptop: Alice's phone stops talking to it.
    drop(b1);
    let b1 = node(&db1, "Bob's phone", &net).await;
    b1.remove_device(&b2.id().to_string()).unwrap();
    let laptop = b2.id().to_string();
    eventually("Alice drops Bob's removed device", || !a.online_peers().contains(&laptop)).await;
    a.put(&tasks(), &list, "tasks", Some("after"), json!({"title": "y"})).unwrap();
    eventually("Bob's phone still gets updates", || has(&b1, &list, "after")).await;
    assert!(b2.get(&Client::Shell, &list, "tasks", "after").unwrap_or(None).is_none());
}

#[tokio::test]
async fn device_codes_and_person_codes_do_not_mix() {
    let net = MemoryLookup::new();
    let (da, db, dc) = (TempDir::new().unwrap(), TempDir::new().unwrap(), TempDir::new().unwrap());
    let a = person(&da, "Alice", &net).await;
    let b = person(&db, "Bob", &net).await;
    let c = node(&dc, "Spare", &net).await;

    // A person can't use a device code (that would make them a full member)…
    let device = a.create_invite().await.unwrap();
    assert!(matches!(b.meet(&device.code).await, Err(Error::Pairing(_))));
    assert!(a.people().is_empty() && a.devices().len() == 1);
    // …and the code is still good for a device.
    c.join(&device.code).await.unwrap();
    assert_eq!(a.devices().len(), 2);

    // A device can't join with a person code.
    let d = TempDir::new().unwrap();
    let stray = node(&d, "Stray", &net).await;
    let invite = a.create_person_invite(None).await.unwrap();
    assert!(matches!(stray.join(&invite.code).await, Err(Error::Pairing(_))));
    assert_eq!(a.devices().len(), 2);
    b.meet(&invite.code).await.unwrap();
    assert_eq!(a.people()[0].name, "Bob");

    // You need a name before linking with people.
    assert!(stray.create_person_invite(None).await.is_err());
}

#[tokio::test]
async fn sharing_with_someone_new_and_unlinking() {
    let net = MemoryLookup::new();
    let (da, db) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let a = person(&da, "Alice", &net).await;
    let b = person(&db, "Bob", &net).await;
    let list = a.create_container("tasks", "Work", None).unwrap();
    a.put(&tasks(), &list, "tasks", Some("t1"), json!({"title": "Report"})).unwrap();

    // One code both links Bob and offers him the list.
    let invite = a.create_person_invite(Some((list.clone(), Access::Read))).await.unwrap();
    b.meet(&invite.code).await.unwrap();
    eventually("offered", || b.people().first().is_some_and(|p| p.receiving.iter().any(|s| s.offered))).await;
    b.accept_share(&list).unwrap();
    eventually("synced", || has(&b, &list, "t1")).await;

    // Another app needs a grant, and one grant covers all of Tasks' lists.
    b.install_app(&manifest("calendar", "Calendar")).unwrap();
    let cal = Client::App("calendar".into());
    assert!(b.list(&cal, &list, "tasks").is_err());
    b.set_grant("app:calendar", "tasks", Some(Access::Write)).unwrap();
    assert_eq!(b.list(&cal, &list, "tasks").unwrap().len(), 1);
    // …but nobody on Bob's side can write to a view-only list.
    assert!(b.put(&cal, &list, "tasks", None, json!({})).is_err());
    assert!(b.put(&Client::Shell, &list, "tasks", None, json!({})).is_err());

    // Alice unlinks Bob: everything between them ends, on both sides.
    let bob = a.people()[0].id.clone();
    a.remove_person(&bob).unwrap();
    eventually("Bob loses the list and the link", || {
        b.people().is_empty() && b.containers(&Client::Shell).unwrap().iter().all(|c| c.id != list)
    })
    .await;
    assert!(a.people().is_empty());
    eventually("they disconnect", || a.online_peers().is_empty() && b.online_peers().is_empty()).await;
}

// ---------------------------------------------------------------------------
// Storage
// ---------------------------------------------------------------------------

#[tokio::test]
async fn many_containers_stay_out_of_memory() {
    const LIMIT: usize = 32;
    let net = MemoryLookup::new();
    let (da, db) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let a = node(&da, "Laptop", &net).await;
    a.install_app(&manifest("tasks", "Tasks")).unwrap();
    let mut lists = vec![];
    for i in 0..300 {
        let id = a.create_container("tasks", &format!("List {i}"), None).unwrap();
        a.put(&tasks(), &id, "tasks", Some("t"), json!({"title": format!("task {i}")})).unwrap();
        lists.push(id);
    }
    assert!(a.docs_in_memory() <= LIMIT, "only recently used documents stay loaded");
    a.shutdown().await;

    // Listing containers (with their sizes and counts) loads none of them.
    let a = node(&da, "Laptop", &net).await;
    let listed = a.containers(&tasks()).unwrap();
    assert_eq!(listed.iter().filter(|c| c.records == 1 && c.size_bytes > 0).count(), 300);
    assert!(a.docs_in_memory() <= 2, "listing loaded {} documents", a.docs_in_memory());

    // A second device gets everything, and neither holds it all in memory.
    let b = node(&db, "Phone", &net).await;
    pair(&a, &b).await;
    eventually("all lists reach B", || {
        b.containers(&Client::Shell).unwrap().iter().filter(|c| c.records == 1).count() == 300
    })
    .await;
    assert!(a.docs_in_memory() <= LIMIT && b.docs_in_memory() <= LIMIT);
    assert_eq!(b.get(&tasks(), &lists[299], "tasks", "t").unwrap().unwrap()["title"], json!("task 299"));

    // After a restart the devices compare fingerprints and load nothing,
    // because nothing differs.
    a.shutdown().await;
    b.shutdown().await;
    let a = node(&da, "Laptop", &net).await;
    let b = node(&db, "Phone", &net).await;
    assert!(a.wait_for_peer(b.id(), Duration::from_secs(30)).await, "they reconnect");
    tokio::time::sleep(Duration::from_secs(1)).await;
    assert!(a.docs_in_memory() <= 2 && b.docs_in_memory() <= 2, "identical documents were loaded");

    // Changes made after that still travel, in both directions.
    a.put(&tasks(), &lists[7], "tasks", Some("new"), json!({"title": "from A"})).unwrap();
    b.put(&tasks(), &lists[8], "tasks", Some("new"), json!({"title": "from B"})).unwrap();
    eventually("the changes cross", || has(&b, &lists[7], "new") && has(&a, &lists[8], "new")).await;
    assert!(a.docs_in_memory() <= LIMIT && b.docs_in_memory() <= LIMIT);
}

#[tokio::test]
async fn offline_changes_are_found_when_devices_reconnect() {
    let net = MemoryLookup::new();
    let (da, db) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let a = node(&da, "Laptop", &net).await;
    let b = node(&db, "Phone", &net).await;
    a.install_app(&manifest("tasks", "Tasks")).unwrap();
    pair(&a, &b).await;
    a.put(&tasks(), "tasks", "tasks", Some("t1"), json!({"title": "one"})).unwrap();
    eventually("task reaches B", || has(&b, "tasks", "t1")).await;

    // B changes something while A is off; A changes something while B is off.
    a.shutdown().await;
    b.put(&tasks(), "tasks", "tasks", Some("t2"), json!({"title": "made on B alone"})).unwrap();
    b.shutdown().await;
    let a = node(&da, "Laptop", &net).await;
    a.put(&tasks(), "tasks", "tasks", Some("t3"), json!({"title": "made on A alone"})).unwrap();
    let b = node(&db, "Phone", &net).await;
    eventually("both changes cross", || has(&a, "tasks", "t2") && has(&b, "tasks", "t3")).await;
}

/// `tests/fixtures/v0.1.3` is a data folder written by the 0.1.3 release
/// (commit 9306d29): the device "Old laptop" of Alex, Tasks with a list
/// "Home" (40 tasks, shared with Sam) and a deleted list, Calendar with a
/// calendar "Work", an AI client, and a view-only list "Trip" from Sam.
#[tokio::test]
async fn a_database_from_0_1_3_opens_and_syncs() {
    let net = MemoryLookup::new();
    let (da, db) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let old = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v0.1.3");
    std::fs::create_dir(da.path().join("keys")).unwrap();
    for f in ["uuob.db", "keys/master.key", "keys/device.key"] {
        std::fs::copy(old.join(f), da.path().join(f)).unwrap();
    }

    let a = node(&da, "ignored: the stored name wins", &net).await;
    assert_eq!(a.status().device_name, "Old laptop");
    assert_eq!(a.status().network.name.as_deref(), Some("Alex"));
    let all = a.containers(&Client::Shell).unwrap();
    let named = |n: &str| all.iter().find(|c| c.name == n).unwrap_or_else(|| panic!("no container {n}"));
    let (home, trip) = (named("Home").id.clone(), named("Trip").id.clone());
    assert_eq!(named("Home").records, 41, "40 tasks and the list's info");
    assert_eq!(named("Home").shared_with.len(), 1);
    assert_eq!(named("Work").records, 1);
    assert!(named("Trip").from.is_some() && !named("Trip").can_write);
    assert!(all.iter().all(|c| c.name != "Gone"));
    assert_eq!(a.people().len(), 1);
    assert_eq!(a.get(&tasks(), &home, "tasks", "h7").unwrap().unwrap()["title"], json!("Home task 7"));
    let calendar = Client::App("calendar".into());
    assert_eq!(a.get(&calendar, "calendar", "events", "e1").unwrap().unwrap()["title"], json!("Dentist"));

    // Changes made after the upgrade are kept next to the old data.
    a.put(&tasks(), &home, "tasks", Some("new"), json!({"title": "added after the upgrade"})).unwrap();
    a.shutdown().await;
    let a = node(&da, "Old laptop", &net).await;
    assert!(has(&a, &home, "new") && has(&a, &home, "h39") && has(&a, "tasks", "t1"));

    // A new device joins and receives everything.
    let b = node(&db, "Phone", &net).await;
    pair(&a, &b).await;
    eventually("old and new data reach the new device", || {
        has(&b, &home, "h39") && has(&b, &home, "new") && has(&b, &trip, "x1") && has(&b, "tasks", "t1")
    })
    .await;
}

#[tokio::test]
async fn a_large_document_is_saved_in_pieces_and_survives_a_restart() {
    let net = MemoryLookup::new();
    let da = TempDir::new().unwrap();
    let a = node(&da, "Laptop", &net).await;
    a.install_app(&manifest("tasks", "Tasks")).unwrap();
    // About 1.5 MB of changes: enough to be folded into a full save several times.
    for i in 0..500 {
        let notes = format!("{i} ").repeat(600);
        a.put(&tasks(), "tasks", "tasks", Some(&format!("t{i}")), json!({"title": format!("task {i}"), "notes": notes})).unwrap();
    }
    a.delete(&tasks(), "tasks", "tasks", "t0").unwrap();
    a.shutdown().await;

    let a = node(&da, "Laptop", &net).await;
    let all = a.list(&tasks(), "tasks", "tasks").unwrap();
    assert_eq!(all.len(), 499);
    assert_eq!(a.get(&tasks(), "tasks", "tasks", "t499").unwrap().unwrap()["notes"], json!("499 ".repeat(600)));
    let size = a.containers(&tasks()).unwrap()[0].size_bytes;
    assert!(size > 100_000 && size < 3_000_000, "stored size {size}");
}

// ---------------------------------------------------------------------------
// Files
// ---------------------------------------------------------------------------

/// Bytes that do not repeat, so a mixed-up piece would be noticed.
fn bytes(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i * 31 + i / 251) as u8).collect()
}

fn stored(dir: &TempDir, id: &str) -> std::path::PathBuf {
    dir.path().join("files").join(id)
}

#[tokio::test]
async fn files_are_stored_encrypted_and_read_in_ranges() {
    const PIECE: usize = 64 * 1024;
    let net = MemoryLookup::new();
    let da = TempDir::new().unwrap();
    let a = node(&da, "Laptop", &net).await;
    a.install_app(&manifest("tasks", "Tasks")).unwrap();
    a.install_app(&manifest("calendar", "Calendar")).unwrap();

    for size in [0, 5, PIECE - 1, PIECE, PIECE + 1, 3 * PIECE, 300_000] {
        let data = bytes(size);
        let f = a.put_file(&tasks(), "tasks", "image/jpeg", &data).await.unwrap();
        assert_eq!((f.size, f.content_type.as_str(), f.here), (size as u64, "image/jpeg", true));
        assert_eq!(a.get_file(&tasks(), "tasks", &f.id).await.unwrap(), data, "size {size}");
        for (start, end) in [(0, 1), (size / 2, size), (PIECE.min(size), size), (size.saturating_sub(3), size + 10)] {
            let (_, mut r) = a.read_file(&tasks(), "tasks", &f.id, start as u64, Some(end as u64)).await.unwrap();
            let mut got = vec![];
            while let Some(b) = r.next().await.unwrap() {
                got.extend(b);
            }
            assert_eq!(got, data[start.min(size)..end.min(size)], "size {size}, range {start}..{end}");
        }
    }
    assert_eq!(a.files(&tasks(), "tasks").unwrap().len(), 7);

    // On disk there is only ciphertext, under the hash of what is stored.
    let secret = b"a line nobody else should read".repeat(100);
    let f = a.put_file(&tasks(), "tasks", "text/plain", &secret).await.unwrap();
    let raw = std::fs::read(stored(&da, &f.id)).unwrap();
    assert!(!raw.windows(12).any(|w| w == b"nobody else "));
    assert!(raw.len() > secret.len());

    // Listed with the container, counted apart from its records.
    let c = a.containers(&tasks()).unwrap().into_iter().find(|c| c.id == "tasks").unwrap();
    assert_eq!((c.files, c.records), (8, 0));
    assert!(c.file_bytes > 300_000 + secret.len() as u64);

    // Permissions are the container's: another app gets nothing.
    let calendar = Client::App("calendar".into());
    assert!(a.get_file(&calendar, "tasks", &f.id).await.is_err());
    assert!(a.put_file(&calendar, "tasks", "text/plain", b"x").await.is_err());
    assert!(a.files(&calendar, "tasks").is_err());
    // Records cannot be written where files are listed.
    assert!(a.put(&tasks(), "tasks", "~file", Some("x"), json!({"key": "k"})).is_err());

    // Deleting removes the entry and the bytes; they also survive a restart until then.
    a.shutdown().await;
    let a = node(&da, "Laptop", &net).await;
    assert_eq!(a.get_file(&tasks(), "tasks", &f.id).await.unwrap(), secret);
    assert!(a.delete_file(&tasks(), "tasks", &f.id).unwrap());
    assert!(!a.delete_file(&tasks(), "tasks", &f.id).unwrap());
    assert!(!stored(&da, &f.id).exists());
    assert!(a.get_file(&tasks(), "tasks", &f.id).await.is_err());
    assert_eq!(a.files(&tasks(), "tasks").unwrap().len(), 7);
}

#[tokio::test]
async fn files_travel_between_devices_and_bad_copies_are_replaced() {
    let net = MemoryLookup::new();
    let (da, db) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let a = node(&da, "Laptop", &net).await;
    a.install_app(&manifest("tasks", "Tasks")).unwrap();
    let photo = bytes(1_000_000);
    let f = a.put_file(&tasks(), "tasks", "image/jpeg", &photo).await.unwrap();
    let small = a.put_file(&tasks(), "tasks", "text/plain", b"hello").await.unwrap();

    // B already has the first part of the big file from an attempt that was cut off.
    let b = node(&db, "Phone", &net).await;
    std::fs::create_dir_all(db.path().join("files")).unwrap();
    let on_a = std::fs::read(stored(&da, &f.id)).unwrap();
    std::fs::write(stored(&db, &format!("{}.part", f.id)), &on_a[..400_000]).unwrap();
    // ...and a part of the small one that is rubbish.
    std::fs::write(stored(&db, &format!("{}.part", small.id)), b"xx").unwrap();

    pair(&a, &b).await;
    let here = |n: &Node, id: &str| n.files(&tasks(), "tasks").is_ok_and(|fs| fs.iter().any(|f| f.id == id && f.here));
    eventually("both files reach B", || here(&b, &f.id) && here(&b, &small.id)).await;
    assert_eq!(b.get_file(&tasks(), "tasks", &f.id).await.unwrap(), photo);
    assert_eq!(b.get_file(&tasks(), "tasks", &small.id).await.unwrap(), b"hello");
    assert_eq!(std::fs::read(stored(&db, &f.id)).unwrap(), on_a, "the same bytes are stored on both devices");

    // B's copy gets damaged: reading notices, and a good copy is fetched again.
    let mut raw = on_a.clone();
    raw[500_000] ^= 1;
    std::fs::write(stored(&db, &f.id), &raw).unwrap();
    assert!(b.get_file(&tasks(), "tasks", &f.id).await.is_err());
    eventually("B has a good copy again", || here(&b, &f.id)).await;
    assert_eq!(b.get_file(&tasks(), "tasks", &f.id).await.unwrap(), photo);

    // A file added on B travels the other way; deleting on A removes B's bytes.
    let back = b.put_file(&tasks(), "tasks", "video/mp4", &bytes(200_000)).await.unwrap();
    eventually("B's file reaches A", || here(&a, &back.id)).await;
    a.delete_file(&tasks(), "tasks", &f.id).unwrap();
    eventually("the file is gone from B's disk", || !stored(&db, &f.id).exists()).await;

    // Deleting the app's data removes its files everywhere.
    a.uninstall_app("tasks", true).unwrap();
    eventually("all file bytes are gone on both", || {
        [&da, &db].iter().all(|d| std::fs::read_dir(d.path().join("files")).unwrap().count() == 0)
    })
    .await;
}

#[tokio::test]
async fn people_get_the_files_of_what_is_shared_and_no_others() {
    let net = MemoryLookup::new();
    let (da, db) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let a = person(&da, "Alice", &net).await;
    let b = person(&db, "Bob", &net).await;
    let (bob, _alice) = meet(&a, &b).await;

    let list = a.create_container("tasks", "Trip", None).unwrap();
    let shared = a.put_file(&tasks(), &list, "image/jpeg", &bytes(150_000)).await.unwrap();
    let private = a.put_file(&tasks(), "tasks", "image/jpeg", &bytes(90_000)).await.unwrap();
    a.share(&list, &bob, Some(Access::Read)).unwrap();
    eventually("Bob is offered the list", || b.people().first().is_some_and(|p| !p.receiving.is_empty())).await;
    b.accept_share(&list).unwrap();
    let here = |n: &Node, id: &str| n.files(&tasks(), &list).is_ok_and(|fs| fs.iter().any(|f| f.id == id && f.here));
    eventually("the shared file reaches Bob", || here(&b, &shared.id)).await;
    assert_eq!(b.get_file(&tasks(), &list, &shared.id).await.unwrap(), bytes(150_000));

    // Bob's device asks Alice's for a file of her private container: refused.
    assert!(!b.fetch_for_test("tasks~0", &private.id, private.size).await);
    assert!(!stored(&db, &private.id).exists());
    // The same request for the shared list's file is served.
    assert!(b.fetch_for_test(&format!("{list}~0"), &shared.id, shared.size).await);

    // View only: Bob cannot add or remove files.
    assert!(b.put_file(&tasks(), &list, "image/jpeg", b"x").await.is_err());
    assert!(b.delete_file(&tasks(), &list, &shared.id).is_err());

    // With edit access his file reaches Alice.
    a.share(&list, &bob, Some(Access::Write)).unwrap();
    eventually("Bob may edit", || b.containers(&tasks()).unwrap().iter().any(|c| c.id == list && c.can_write)).await;
    let from_bob = b.put_file(&tasks(), &list, "image/png", &bytes(70_000)).await.unwrap();
    eventually("Bob's file reaches Alice", || here(&a, &from_bob.id)).await;
    assert_eq!(a.get_file(&tasks(), &list, &from_bob.id).await.unwrap(), bytes(70_000));

    // Sharing stops: Bob's copy of the list goes, and its files with it.
    a.share(&list, &bob, None).unwrap();
    eventually("Bob's files are gone", || !stored(&db, &shared.id).exists() && !stored(&db, &from_bob.id).exists()).await;
    assert!(!b.fetch_for_test(&format!("{list}~0"), &shared.id, shared.size).await);
}

// ---------------------------------------------------------------------------
// What each device keeps
// ---------------------------------------------------------------------------

fn summary(n: &Node, container: &str) -> ContainerSummary {
    n.containers(&Client::Shell).unwrap().into_iter().find(|c| c.id == container).unwrap()
}

fn file_here(n: &Node, container: &str, id: &str) -> bool {
    n.files(&tasks(), container).is_ok_and(|fs| fs.iter().any(|f| f.id == id && f.here))
}

async fn settle() {
    tokio::time::sleep(Duration::from_millis(600)).await;
}

#[tokio::test]
async fn a_device_lets_go_only_once_a_storage_device_has_it() {
    let net = MemoryLookup::new();
    let (dp, dm, dk) = (TempDir::new().unwrap(), TempDir::new().unwrap(), TempDir::new().unwrap());
    let p = node(&dp, "Phone", &net).await;
    p.install_app(&manifest("tasks", "Tasks")).unwrap();
    let album = p.create_container("tasks", "Album", None).unwrap();
    p.put(&tasks(), &album, "tasks", Some("r1"), json!({"title": "one"})).unwrap();
    let photo = bytes(200_000);
    let f = p.put_file(&tasks(), &album, "image/jpeg", &photo).await.unwrap();

    // Asked to keep nothing, with no storage device: the phone keeps everything.
    p.set_keep(&tasks(), &album, Keep::None).unwrap();
    settle().await;
    assert!(summary(&p, &album).here && stored(&dp, &f.id).exists());
    assert_eq!(summary(&p, &album).keep, Keep::None);

    // Another device that is not a storage device does not count.
    let m = node(&dm, "Mac", &net).await;
    pair(&p, &m).await;
    eventually("the Mac has the album", || has(&m, &album, "r1") && file_here(&m, &album, &f.id)).await;
    // (Reconnect, so that the Mac reports the album exactly as the phone has it.)
    m.shutdown().await;
    let m = node(&dm, "Mac", &net).await;
    assert!(p.wait_for_peer(m.id(), Duration::from_secs(30)).await);
    settle().await;
    assert!(summary(&p, &album).here && summary(&p, &album).copies == 0);

    // A storage device joins. Once it has everything, the phone lets go.
    let k = node(&dk, "Pi", &net).await;
    pair(&p, &k).await;
    p.set_keeper(&k.id().to_string(), true).unwrap();
    eventually("the phone lets go of the album", || !summary(&p, &album).here && !stored(&dp, &f.id).exists()).await;
    assert!(has(&k, &album, "r1") && file_here(&k, &album, &f.id));
    assert!(k.devices().iter().any(|d| d.is_self && d.keeper));
    assert!(matches!(p.get(&tasks(), &album, "tasks", "r1"), Err(Error::Unavailable(_))));
    assert!(matches!(p.put(&tasks(), &album, "tasks", None, json!({})), Err(Error::Unavailable(_))));
    assert!(matches!(p.files(&tasks(), &album), Err(Error::Unavailable(_))));
    // Its other data is untouched, and the album is still listed.
    p.put(&tasks(), "tasks", "tasks", Some("t"), json!({"title": "still works"})).unwrap();
    assert_eq!(summary(&p, &album).name, "Album");

    // Changes made elsewhere do not bring it back.
    m.put(&tasks(), &album, "tasks", Some("r2"), json!({"title": "two"})).unwrap();
    eventually("the storage device gets the change", || has(&k, &album, "r2")).await;
    settle().await;
    assert!(!summary(&p, &album).here);

    // The phone asks for the list again: records return, files come when opened.
    p.set_keep(&tasks(), &album, Keep::List).unwrap();
    eventually("the records return", || has(&p, &album, "r1") && has(&p, &album, "r2")).await;
    settle().await;
    assert!(!file_here(&p, &album, &f.id) && !stored(&dp, &f.id).exists());
    assert_eq!(p.get_file(&tasks(), &album, &f.id).await.unwrap(), photo);
    assert!(file_here(&p, &album, &f.id));

    // With every other device off, a file that is not here cannot be opened;
    // one that is here still can.
    let f2 = k.put_file(&tasks(), &album, "image/jpeg", &bytes(50_000)).await.unwrap();
    eventually("the phone knows about the new file", || p.files(&tasks(), &album).is_ok_and(|fs| fs.len() == 2)).await;
    k.shutdown().await;
    m.shutdown().await;
    eventually("the phone is alone", || p.online_peers().is_empty()).await;
    assert!(matches!(p.get_file(&tasks(), &album, &f2.id).await, Err(Error::Unavailable(_))));
    assert_eq!(p.get_file(&tasks(), &album, &f.id).await.unwrap(), photo);
}

#[tokio::test]
async fn copies_says_how_many_storage_devices_are_needed() {
    let net = MemoryLookup::new();
    let dirs: Vec<TempDir> = (0..3).map(|_| TempDir::new().unwrap()).collect();
    let p = node(&dirs[0], "Phone", &net).await;
    p.install_app(&manifest("tasks", "Tasks")).unwrap();
    let album = p.create_container("tasks", "Album", None).unwrap();
    p.put(&tasks(), &album, "tasks", Some("r1"), json!({"title": "one"})).unwrap();
    p.call(&Client::Shell, "set_copies", json!({"copies": 2})).await.unwrap();
    assert!(p.call(&Client::Shell, "set_copies", json!({"copies": 0})).await.is_err());
    assert!(p.call(&tasks(), "set_copies", json!({"copies": 1})).await.is_err(), "only the user sets this");
    p.call(&tasks(), "keep", json!({"container": album, "keep": "none"})).await.unwrap();

    let k1 = node(&dirs[1], "Pi", &net).await;
    pair(&p, &k1).await;
    p.call(&Client::Shell, "set_keeper", json!({"device": k1.id().to_string(), "on": true})).await.unwrap();
    eventually("one storage device confirms", || summary(&p, &album).copies == 1).await;
    settle().await;
    assert!(summary(&p, &album).here, "one copy is not enough when two are asked for");

    let k2 = node(&dirs[2], "Mac", &net).await;
    pair(&p, &k2).await;
    p.set_keeper(&k2.id().to_string(), true).unwrap();
    eventually("with two storage devices the phone lets go", || !summary(&p, &album).here).await;
    assert!(has(&k1, &album, "r1") && has(&k2, &album, "r1"));

    // A storage device keeps everything, whatever an app asks of it.
    k1.set_keep(&tasks(), &album, Keep::None).unwrap();
    settle().await;
    assert!(summary(&k1, &album).here && summary(&k1, &album).keep == Keep::All);
}

#[tokio::test]
async fn a_list_only_device_caches_files_up_to_a_limit() {
    let net = MemoryLookup::new();
    let (dp, dk) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let p = node(&dp, "Phone", &net).await;
    p.install_app(&manifest("tasks", "Tasks")).unwrap();
    p.set_cache_limit(&p.id().to_string(), 250_000).unwrap();
    let album = p.create_container("tasks", "Album", None).unwrap();
    p.set_keep(&tasks(), &album, Keep::List).unwrap();
    let mut files = vec![];
    for i in 0..3u8 {
        let data: Vec<u8> = bytes(100_000).into_iter().map(|b| b ^ i).collect();
        files.push((p.put_file(&tasks(), &album, "image/jpeg", &data).await.unwrap(), data));
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let on_phone = |i: usize| stored(&dp, &files[i].0.id).exists();

    // Over the limit, but nothing else has the files: all of them stay.
    settle().await;
    assert!(on_phone(0) && on_phone(1) && on_phone(2));

    // Once a storage device has them, the least recently used one goes.
    let k = node(&dk, "Pi", &net).await;
    pair(&p, &k).await;
    p.set_keeper(&k.id().to_string(), true).unwrap();
    eventually("the oldest file leaves the phone", || !on_phone(0)).await;
    settle().await;
    assert!(on_phone(1) && on_phone(2));
    assert!((0..3).all(|i| file_here(&k, &album, &files[i].0.id)));
    assert_eq!(summary(&p, &album).files_here, 2);

    // Opening it brings it back, and the one not opened for longest goes instead.
    assert_eq!(p.get_file(&tasks(), &album, &files[0].0.id).await.unwrap(), files[0].1);
    eventually("the cache is trimmed again", || on_phone(0) && !on_phone(1) && on_phone(2)).await;
}

#[tokio::test]
async fn a_big_file_that_is_elsewhere_is_read_without_fetching_it_whole() {
    let net = MemoryLookup::new();
    let (dp, dk) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let p = node(&dp, "Phone", &net).await;
    p.install_app(&manifest("tasks", "Tasks")).unwrap();
    p.set_cache_limit(&p.id().to_string(), 0).unwrap();
    p.set_stream_threshold(100_000);
    let album = p.create_container("tasks", "Album", None).unwrap();
    p.set_keep(&tasks(), &album, Keep::List).unwrap();
    let video = bytes(700_000);
    let big = p.put_file(&tasks(), &album, "video/mp4", &video).await.unwrap();
    let small = p.put_file(&tasks(), &album, "image/jpeg", &bytes(50_000)).await.unwrap();

    let k = node(&dk, "Pi", &net).await;
    pair(&p, &k).await;
    p.set_keeper(&k.id().to_string(), true).unwrap();
    eventually("the phone keeps no files", || !stored(&dp, &big.id).exists() && !stored(&dp, &small.id).exists()).await;

    // A part from the middle, across piece borders, then the whole file.
    let (_, mut r) = p.read_file(&tasks(), &album, &big.id, 130_000, Some(400_001)).await.unwrap();
    let mut part = vec![];
    while let Some(b) = r.next().await.unwrap() {
        part.extend(b);
    }
    assert_eq!(part, video[130_000..400_001]);
    assert_eq!(p.get_file(&tasks(), &album, &big.id).await.unwrap(), video);
    assert!(!stored(&dp, &big.id).exists(), "the big file was streamed, not stored");
    // A small one does not fit this phone's cache (0 bytes) either: read the same way,
    // any number of times.
    for _ in 0..5 {
        assert_eq!(p.get_file(&tasks(), &album, &small.id).await.unwrap(), bytes(50_000));
    }
    assert!(!stored(&dp, &small.id).exists());

    // With the storage device off, neither can be read.
    k.shutdown().await;
    eventually("the phone is alone", || p.online_peers().is_empty()).await;
    assert!(matches!(p.read_file(&tasks(), &album, &big.id, 0, None).await, Err(Error::Unavailable(_))));
}

// ---- Apps on a device without UUOB (remote.rs) ----

/// Stands in for the local server: sends back whatever a linked app sends,
/// after the app's endpoint id and the device's name.
fn echo(n: &Node, name: &'static str) {
    let mut requests = n.remote_requests();
    tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        while let Some(mut r) = requests.recv().await {
            tokio::spawn(async move {
                let mut buf = [0u8; 64];
                let n = r.io.read(&mut buf).await.unwrap_or(0);
                let _ = r.io.write_all(format!("{name} {} ", r.app).as_bytes()).await;
                let _ = r.io.write_all(&buf[..n]).await;
                let _ = r.io.shutdown().await;
            });
        }
    });
}

/// What comes back through the tunnel for "ping" (`None`: nobody answered).
async fn ping(t: &Tunnel) -> Option<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut tcp = tokio::net::TcpStream::connect(("127.0.0.1", t.port())).await.ok()?;
    tcp.write_all(b"ping").await.ok()?;
    let mut out = String::new();
    tcp.read_to_string(&mut out).await.ok()?;
    (!out.is_empty()).then_some(out)
}

async fn tunnel(dir: &TempDir, net: &MemoryLookup) -> Tunnel {
    Tunnel::open(dir.path().join("app"), NetMode::Local(net.clone())).await.unwrap()
}

#[tokio::test]
async fn an_app_on_another_device_links_with_a_code() {
    let net = MemoryLookup::new();
    let (da, dt) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let a = node(&da, "Laptop", &net).await;
    echo(&a, "laptop");

    // Not linked: nothing answers.
    let t = tunnel(&dt, &net).await;
    assert!(!t.linked().await && ping(&t).await.is_none());

    let code = a.create_app_code().await.unwrap().code;
    let token = t.link(&code, &manifest("tasks", "Tasks")).await.unwrap();
    assert!(t.linked().await);
    assert!(a.apps()["tasks"].installed, "the app is registered like one on the device");

    // Its requests arrive, marked with its endpoint id; the token is the
    // app's and works only from there.
    let reply = ping(&t).await.expect("the device answers");
    let endpoint = reply.split(' ').nth(1).unwrap().to_string();
    assert_eq!(reply, format!("laptop {endpoint} ping"));
    assert_eq!(a.authenticate_from(&token, Some(&endpoint)), Some(tasks()));
    assert_eq!(a.authenticate_from(&token, Some("someone-else")), None);

    // The code was used up.
    let dt2 = TempDir::new().unwrap();
    let t2 = tunnel(&dt2, &net).await;
    assert!(matches!(t2.link(&code, &manifest("tasks", "Tasks")).await, Err(Error::Pairing(_))));

    // The link survives a restart of the app.
    drop(t);
    let t = tunnel(&dt, &net).await;
    assert!(t.linked().await);
    assert_eq!(ping(&t).await.unwrap(), reply);
}

#[tokio::test]
async fn any_device_of_the_network_serves_a_linked_app() {
    let net = MemoryLookup::new();
    let (da, db, dc, dt) = (TempDir::new().unwrap(), TempDir::new().unwrap(), TempDir::new().unwrap(), TempDir::new().unwrap());
    let a = node(&da, "Laptop", &net).await;
    let b = node(&db, "Server", &net).await;
    pair(&a, &b).await;
    echo(&a, "laptop");
    echo(&b, "server");

    let t = tunnel(&dt, &net).await;
    t.link(&a.create_app_code().await.unwrap().code, &manifest("tasks", "Tasks")).await.unwrap();
    assert!(ping(&t).await.is_some());

    // The laptop goes away: the server, which only heard of the app through
    // the network document, takes over.
    eventually("server knows the app", || b.apps().contains_key("tasks")).await;
    a.shutdown().await;
    let mut reply = None;
    for _ in 0..40 {
        reply = ping(&t).await;
        if reply.as_deref().is_some_and(|r| r.starts_with("server ")) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert!(reply.unwrap().starts_with("server "));

    // A device added later is learned from the device that serves the app.
    let c = node(&dc, "Phone", &net).await;
    pair(&b, &c).await;
    echo(&c, "phone");
    eventually("phone knows the app", || c.apps().contains_key("tasks")).await;
    drop(t);
    let t = tunnel(&dt, &net).await; // reconnects, and hears of the phone
    assert!(ping(&t).await.unwrap().starts_with("server "));
    b.shutdown().await;
    let mut reply = None;
    for _ in 0..40 {
        reply = ping(&t).await;
        if reply.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert!(reply.unwrap().starts_with("phone "));
}

#[tokio::test]
async fn strangers_and_removed_apps_are_turned_away() {
    let net = MemoryLookup::new();
    let (da, dt, ds) = (TempDir::new().unwrap(), TempDir::new().unwrap(), TempDir::new().unwrap());
    let a = node(&da, "Laptop", &net).await;
    echo(&a, "laptop");
    let t = tunnel(&dt, &net).await;
    t.link(&a.create_app_code().await.unwrap().code, &manifest("tasks", "Tasks")).await.unwrap();
    assert!(ping(&t).await.is_some());

    // Someone who knows the device's id but was never linked gets nothing,
    // and is not told that it was "removed" either.
    std::fs::create_dir_all(ds.path().join("app")).unwrap();
    let stranger = json!({ "key": "11".repeat(32), "devices": [a.id().to_string()] });
    std::fs::write(ds.path().join("app/uuob-link.json"), stranger.to_string()).unwrap();
    let s = tunnel(&ds, &net).await;
    assert!(ping(&s).await.is_none());
    assert!(s.linked().await && !s.removed());

    // The user disconnects the app: it is cut off and told so.
    a.uninstall_app("tasks", false).unwrap();
    let mut cut = false;
    for _ in 0..40 {
        if ping(&t).await.is_none() && t.removed() {
            cut = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert!(cut, "the removed app is cut off");
    assert!(!t.linked().await);
    assert!(!dt.path().join("app/uuob-link.json").exists());

    // A new code links it again.
    t.link(&a.create_app_code().await.unwrap().code, &manifest("tasks", "Tasks")).await.unwrap();
    assert!(ping(&t).await.is_some() && !t.removed());
}

#[tokio::test]
async fn one_copy_of_an_app_is_cut_off_without_the_others() {
    let net = MemoryLookup::new();
    let (da, db, d1, d2) = (TempDir::new().unwrap(), TempDir::new().unwrap(), TempDir::new().unwrap(), TempDir::new().unwrap());
    let a = node(&da, "Laptop", &net).await;
    let b = node(&db, "Server", &net).await;
    pair(&a, &b).await;
    echo(&a, "laptop");
    echo(&b, "server");
    a.install_app(&manifest("calendar", "Calendar")).unwrap();

    // Tasks on a tablet and on a phone, neither with UUOB.
    let (tablet, phone) = (tunnel(&d1, &net).await, tunnel(&d2, &net).await);
    let lost = tablet.link(&a.create_app_code().await.unwrap().code, &manifest("tasks", "Tasks")).await.unwrap();
    let (lost_id, lost_copy) = a.clients().into_iter().next().unwrap();
    let kept = phone.link(&a.create_app_code().await.unwrap().code, &manifest("tasks", "Tasks")).await.unwrap();
    let (_, kept_copy) = a.clients().into_iter().find(|(id, _)| *id != lost_id).unwrap();
    a.set_grant("app:tasks", "calendar", Some(Access::Read)).unwrap();
    // UUOB shows each copy with the name of its device.
    assert_eq!(lost_copy.device, Some(device_name()));
    let (lost_at, kept_at) = (lost_copy.endpoint.unwrap(), kept_copy.endpoint.unwrap());
    assert!(ping(&tablet).await.is_some() && ping(&phone).await.is_some());
    eventually("server knows both copies", || b.clients().len() == 2).await;

    // The tablet is lost. The user removes that copy on the laptop; the
    // server, which only hears of it, turns the tablet away too.
    a.delete_client(&lost_id).unwrap();
    eventually("server hears of it", || b.clients().len() == 1).await;
    a.shutdown().await;
    let mut cut = false;
    for _ in 0..40 {
        if ping(&tablet).await.is_none() && tablet.removed() {
            cut = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert!(cut, "the removed copy is cut off");
    // Its token is worth nothing any more, from the tablet or anywhere else.
    assert_eq!(b.authenticate_from(&lost, Some(&lost_at)), None);
    assert_eq!(b.authenticate_from(&lost, Some(&kept_at)), None);
    assert_eq!(b.authenticate(&lost), None);

    // The phone's copy goes on, and Tasks keeps what it was allowed.
    let mut reply = None;
    for _ in 0..40 {
        reply = ping(&phone).await;
        if reply.is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert!(reply.unwrap().starts_with("server "));
    assert_eq!(b.authenticate_from(&kept, Some(&kept_at)), Some(tasks()));
    assert_eq!(b.authenticate_from(&kept, Some(&lost_at)), None);
    assert!(b.apps()["tasks"].installed);
    assert_eq!(b.grants()["app:tasks"]["calendar"], Access::Read);

    // Without its last copy the app is disconnected; its data stays.
    let (kept_id, _) = b.clients().into_iter().next().unwrap();
    b.delete_client(&kept_id).unwrap();
    assert!(!b.apps()["tasks"].installed && b.clients().is_empty());
    assert!(!b.grants().contains_key("app:tasks"));
    assert!(b.containers(&Client::Shell).unwrap().iter().any(|c| c.id == "tasks"));
}

#[tokio::test]
async fn app_codes_are_only_for_apps() {
    let net = MemoryLookup::new();
    let (da, db, dt) = (TempDir::new().unwrap(), TempDir::new().unwrap(), TempDir::new().unwrap());
    let a = node(&da, "Laptop", &net).await;
    let b = node(&db, "Spare", &net).await;
    let t = tunnel(&dt, &net).await;

    // A device cannot join with an app code, and the code stays good.
    let code = a.create_app_code().await.unwrap().code;
    assert!(matches!(b.join(&code).await, Err(Error::Pairing(_))));
    assert_eq!(a.devices().len(), 1);
    // An app cannot link with a device code.
    let device = a.create_invite().await.unwrap().code;
    assert!(matches!(t.link(&device, &manifest("tasks", "Tasks")).await, Err(Error::Pairing(_))));
    assert!(a.apps().is_empty());

    // An app on the same device hands the same kind of code to UUOB directly.
    assert!(a.connect_app(&manifest("tasks", "Tasks"), Some("12-not-the-code")).await.is_err());
    assert!(a.connect_app(&manifest("tasks", "Tasks"), Some(&device)).await.is_err(), "a device code is not an app code");
    let token = a.connect_app(&manifest("tasks", "Tasks"), Some(&code.to_uppercase().replace('-', " "))).await.unwrap();
    assert_eq!(a.authenticate(&token), Some(tasks()));
    assert!(a.connect_app(&manifest("tasks", "Tasks"), Some(&code)).await.is_err(), "used up");

    // A right code clears the typos before it, so typos spread over time cancel nothing.
    let a = &a;
    let wrong = |n: usize| async move {
        for _ in 0..n {
            assert!(a.connect_app(&manifest("x", "X"), Some("12-not-the-code")).await.is_err());
        }
    };
    let code = a.create_app_code().await.unwrap().code;
    wrong(3).await;
    a.connect_app(&manifest("notes", "Notes"), Some(&code)).await.unwrap();
    let code = a.create_app_code().await.unwrap().code;
    wrong(4).await;
    a.connect_app(&manifest("photos", "Photos"), Some(&code)).await.unwrap();
    // Guessing: 5 wrong codes in a row cancel every open app code, even the right one.
    let code = a.create_app_code().await.unwrap().code;
    wrong(5).await;
    assert!(a.connect_app(&manifest("music", "Music"), Some(&code)).await.is_err(), "cancelled after 5 wrong codes");
}

#[tokio::test]
async fn a_question_is_answered_on_any_device() {
    let net = MemoryLookup::new();
    let (da, db) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let a = node(&da, "Laptop", &net).await;
    let b = node(&db, "Server", &net).await;
    a.install_app(&manifest("tasks", "Tasks")).unwrap();
    a.install_app(&manifest("calendar", "Calendar")).unwrap();
    pair(&a, &b).await;
    eventually("server knows the apps", || b.apps().len() == 2).await;
    let calendar = Client::App("calendar".into());

    // Calendar asks on the server (which has no screen); the laptop shows
    // the question and the user answers there.
    let asking = tokio::spawn({
        let (b, calendar) = (b.clone(), calendar.clone());
        async move { b.request_access(&calendar, "tasks", Access::Read).await.unwrap() }
    });
    eventually("the laptop shows the question", || a.asks().len() == 1).await;
    let ask = a.asks().remove(0);
    assert!(!ask.here && b.asks()[0].here);
    assert!(matches!(&ask.ask, Ask::Access { app, container, .. } if app == "calendar" && container == "tasks"));
    a.answer(&ask.id, true).unwrap();
    assert!(asking.await.unwrap());
    eventually("the question is gone everywhere", || a.asks().is_empty() && b.asks().is_empty()).await;
    eventually("the grant reached the laptop", || a.list(&calendar, "tasks", "tasks").is_ok()).await;

    // "No" travels the same way, and a question nobody waits for any more
    // (the app that asked went away) disappears.
    let asking = tokio::spawn({
        let (b, calendar) = (b.clone(), calendar.clone());
        async move { b.request_access(&calendar, "tasks", Access::Write).await.unwrap() }
    });
    eventually("asked again", || a.asks().len() == 1).await;
    a.answer(&a.asks()[0].id, false).unwrap();
    assert!(!asking.await.unwrap());
    let asking = tokio::spawn({
        let (b, calendar) = (b.clone(), calendar.clone());
        async move { b.request_access(&calendar, "tasks", Access::Write).await.unwrap() }
    });
    eventually("asked a third time", || a.asks().len() == 1).await;
    asking.abort();
    eventually("withdrawn", || a.asks().is_empty()).await;
}

#[tokio::test]
async fn settings_apply_to_every_app_and_device() {
    let net = MemoryLookup::new();
    let (da, db) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let a = node(&da, "Laptop", &net).await;
    let b = node(&db, "Phone", &net).await;
    a.install_app(&manifest("tasks", "Tasks")).unwrap();
    b.install_app(&manifest("calendar", "Calendar")).unwrap();
    pair(&a, &b).await;
    let cal = Client::App("calendar".into());

    // 24-hour time and Monday first, until someone chooses otherwise.
    assert_eq!(a.call(&tasks(), "settings", json!({})).await.unwrap(), json!({"hour12": false, "sunday_first": false}));
    // Tasks on the laptop switches to AM/PM: Calendar on the phone sees it.
    a.call(&tasks(), "set_settings", json!({"hour12": true})).await.unwrap();
    eventually("the phone gets the time format", || {
        at_once(b.call(&cal, "settings", json!({})))["hour12"] == json!(true)
    })
    .await;
    // Two devices changing different choices at once keep both.
    b.call(&cal, "set_settings", json!({"sunday_first": true})).await.unwrap();
    eventually("both choices on both devices", || {
        let want = json!({"hour12": true, "sunday_first": true});
        at_once(a.call(&tasks(), "settings", json!({}))) == want && at_once(b.call(&cal, "settings", json!({}))) == want
    })
    .await;
    // Only known choices, with the right type.
    assert!(a.call(&tasks(), "set_settings", json!({"theme": "pink"})).await.is_err());
    assert!(a.call(&tasks(), "set_settings", json!({"hour12": "yes"})).await.is_err());
}

#[tokio::test]
async fn apps_upcoming_reaches_the_other_devices() {
    let net = MemoryLookup::new();
    let (da, db) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let a = node(&da, "Laptop", &net).await;
    let b = node(&db, "Phone", &net).await;
    a.install_app(&manifest("tasks", "Tasks")).unwrap();
    pair(&a, &b).await;
    let app = tasks();
    let put = |container: &str, id: &str, value: serde_json::Value| {
        a.call(&app, "put", json!({"container": container, "collection": "upcoming", "id": id, "value": value}))
    };

    // Tasks on the laptop keeps what is coming up in its own container.
    put("tasks", "later", json!({"date": "2030-01-02", "title": "Call the bank"})).await.unwrap();
    put("tasks", "soon", json!({"date": "2030-01-01", "time": "18:30", "tz": "Europe/Prague", "title": "Water the plants", "body": "Home", "alert": 0, "new": 1}))
        .await
        .unwrap();
    // Not an item: no date.
    put("tasks", "broken", json!({"time": "09:00", "title": "?"})).await.unwrap();
    // A list's container can be shared with other people: its "upcoming" is just records.
    let list = a.create_container("tasks", "Groceries", None).unwrap();
    put(&list, "x", json!({"date": "2030-01-01", "time": "08:00", "title": "Not for the phone"})).await.unwrap();

    // The phone has them, soonest first, with the app they came from.
    eventually("the phone has what is coming up", || b.upcoming().len() == 2).await;
    let items = b.upcoming();
    assert_eq!(
        items[0],
        json!({"id": "soon", "date": "2030-01-01", "time": "18:30", "tz": "Europe/Prague", "title": "Water the plants", "body": "Home", "alert": 0, "new": 1, "app": "tasks", "app_name": "Tasks"})
    );
    assert_eq!(items[1]["id"], "later");
    assert_eq!(b.call(&Client::Shell, "upcoming", json!({})).await.unwrap(), json!(items));
    // Apps can't read it (only the device itself acts on it).
    assert!(b.call(&tasks(), "upcoming", json!({})).await.is_err());

    // A removed app's items go.
    a.uninstall_app("tasks", false).unwrap();
    eventually("the items are gone", || b.upcoming().is_empty()).await;
}

/// `call` is async but answers settings right away; `eventually` takes a plain closure.
fn at_once<F: std::future::Future<Output = Result<serde_json::Value>>>(f: F) -> serde_json::Value {
    let mut f = std::pin::pin!(f);
    let waker = std::task::Waker::noop();
    match f.as_mut().poll(&mut std::task::Context::from_waker(waker)) {
        std::task::Poll::Ready(r) => r.unwrap(),
        std::task::Poll::Pending => panic!("settings should answer at once"),
    }
}

/// A change that could not be stored (the disk is full, say) does not stay
/// in memory as if it were: the app sees the old record, and trying again
/// stores it once the disk works again.
#[tokio::test]
async fn a_change_that_could_not_be_stored_is_not_kept() {
    let net = MemoryLookup::new();
    let d = TempDir::new().unwrap();
    let n = node(&d, "A", &net).await;
    n.install_app(&manifest("tasks", "Tasks")).unwrap();
    n.put(&tasks(), "tasks", "tasks", Some("t1"), json!({"title": "Milk"})).unwrap();

    // Break the store from outside, the way a full disk would.
    let db = rusqlite::Connection::open(d.path().join("uuob.db")).unwrap();
    db.execute_batch("ALTER TABLE changes RENAME TO changes_away;").unwrap();
    assert!(n.put(&tasks(), "tasks", "tasks", Some("t1"), json!({"title": "Oat milk"})).is_err());

    db.execute_batch("ALTER TABLE changes_away RENAME TO changes;").unwrap();
    assert_eq!(n.get(&tasks(), "tasks", "tasks", "t1").unwrap().unwrap()["title"], "Milk", "not kept in memory");
    n.put(&tasks(), "tasks", "tasks", Some("t1"), json!({"title": "Oat milk"})).unwrap();
    n.shutdown().await;
    drop(n);
    let n = node(&d, "A", &net).await;
    assert_eq!(n.get(&tasks(), "tasks", "tasks", "t1").unwrap().unwrap()["title"], "Oat milk", "stored for good");
}
