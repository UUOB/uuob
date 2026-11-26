use serde_json::{Value, json};
use uuob_core::*;

async fn rpc(port: u16, token: &str, method: &str, params: Value) -> (u16, Value) {
    let body = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).to_string();
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let req = format!(
        "POST /mcp HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut out = String::new();
    stream.read_to_string(&mut out).await.unwrap();
    let status: u16 = out[9..12].parse().unwrap();
    let body = out.split("\r\n\r\n").nth(1).unwrap_or("");
    (status, serde_json::from_str(body).unwrap_or(Value::Null))
}

fn text(v: &Value) -> &str {
    v["result"]["content"][0]["text"].as_str().unwrap()
}

#[tokio::test]
async fn ai_sees_only_what_it_was_granted() {
    let dir = tempfile::TempDir::new().unwrap();
    let node = Node::open(Config {
        data_dir: dir.path().into(),
        device_name: "A".into(),
        platform: "linux".into(),
        net: NetMode::Local(MemoryLookup::new()),
    })
    .await
    .unwrap();
    node.install_app(&AppManifest { id: "tasks".into(), name: "Tasks".into() }).unwrap();
    node.install_app(&AppManifest { id: "calendar".into(), name: "Calendar".into() }).unwrap();
    node.put(&Client::App("tasks".into()), "tasks", "tasks", Some("t1"), json!({"title": "Buy milk"})).unwrap();
    let (cid, token) = node.create_client("Claude").unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = uuob_api::router(node.clone());
    tokio::spawn(async move { axum::serve(listener, app).await });

    assert_eq!(rpc(port, "bad", "initialize", json!({})).await.0, 401);
    let (_, init) = rpc(port, &token, "initialize", json!({})).await;
    assert_eq!(init["result"]["serverInfo"]["name"], "uuob");
    let (_, tl) = rpc(port, &token, "tools/list", json!({})).await;
    assert_eq!(tl["result"]["tools"].as_array().unwrap().len(), 5);

    // No grants yet: nothing visible.
    let (_, r) = rpc(port, &token, "tools/call", json!({"name": "list_containers", "arguments": {}})).await;
    assert_eq!(text(&r).trim(), "[]");
    let (_, r) = rpc(port, &token, "tools/call", json!({"name": "list_records", "arguments": {"container": "tasks", "collection": "tasks"}})).await;
    assert_eq!(r["result"]["isError"], true);

    // Read grant on tasks: can list, cannot write, calendar invisible.
    node.set_grant(&format!("ai:{cid}"), "tasks", Some(Access::Read)).unwrap();
    let (_, r) = rpc(port, &token, "tools/call", json!({"name": "list_records", "arguments": {"container": "tasks", "collection": "tasks"}})).await;
    assert!(text(&r).contains("Buy milk"));
    let (_, r) = rpc(port, &token, "tools/call", json!({"name": "put_record", "arguments": {"container": "tasks", "collection": "tasks", "value": {"title": "x"}}})).await;
    assert_eq!(r["result"]["isError"], true);
    let (_, r) = rpc(port, &token, "tools/call", json!({"name": "list_containers", "arguments": {}})).await;
    assert!(!text(&r).contains("calendar"));

    // Write grant: AI creates a task that the app sees.
    node.set_grant(&format!("ai:{cid}"), "tasks", Some(Access::Write)).unwrap();
    let (_, r) = rpc(port, &token, "tools/call", json!({"name": "put_record", "arguments": {"container": "tasks", "collection": "tasks", "value": {"title": "From Claude"}}})).await;
    assert_eq!(r["result"]["isError"], false);
    let all = node.list(&Client::App("tasks".into()), "tasks", "tasks").unwrap();
    assert!(all.iter().any(|t| t["title"] == "From Claude"));
}

async fn http(port: u16, method: &str, path: &str, token: &str, body: Option<Value>) -> (u16, Value) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let body = body.map(|b| b.to_string()).unwrap_or_default();
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut out = String::new();
    stream.read_to_string(&mut out).await.unwrap();
    let status: u16 = out[9..12].parse().unwrap();
    let body = out.split("\r\n\r\n").nth(1).unwrap_or("");
    (status, serde_json::from_str(body).unwrap_or(Value::Null))
}

#[tokio::test]
async fn apps_connect_by_allow_or_code() {
    let dir = tempfile::TempDir::new().unwrap();
    let node = Node::open(Config {
        data_dir: dir.path().into(),
        device_name: "Laptop".into(),
        platform: "linux".into(),
        net: NetMode::Local(MemoryLookup::new()),
    })
    .await
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = uuob_api::router(node.clone());
    tokio::spawn(async move { axum::serve(listener, app).await });

    let (s, hello) = http(port, "GET", "/api", "", None).await;
    assert_eq!((s, hello["device"].as_str()), (200, Some("Laptop")));

    // Connect by asking: the user says no, then yes.
    let n = node.clone();
    let deny = tokio::spawn(async move {
        loop {
            if let Some(a) = n.asks().first() {
                n.answer(&a.id, false).unwrap();
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    });
    let (s, _) = http(port, "POST", "/api/connect", "", Some(json!({"app": "tasks", "name": "Tasks"}))).await;
    assert_eq!(s, 403);
    deny.await.unwrap();
    assert!(node.asks().is_empty());

    let n = node.clone();
    tokio::spawn(async move {
        loop {
            if let Some(a) = n.asks().first() {
                assert!(matches!(&a.ask, Ask::Connect { app, .. } if app == "tasks"));
                n.answer(&a.id, true).unwrap();
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    });
    let (s, r) = http(port, "POST", "/api/connect", "", Some(json!({"app": "tasks", "name": "Tasks"}))).await;
    assert_eq!(s, 200);
    let tasks = r["token"].as_str().unwrap().to_string();

    // The app uses its own container.
    let (s, _) = http(port, "POST", "/api/call", &tasks, Some(json!({"method": "put", "params": {"collection": "tasks", "id": "t1", "value": {"title": "Milk"}}}))).await;
    assert_eq!(s, 200);
    let (_, r) = http(port, "POST", "/api/call", &tasks, Some(json!({"method": "list", "params": {"collection": "tasks"}}))).await;
    assert_eq!(r["result"][0]["title"], "Milk");
    // Shell-only methods are refused.
    let (s, _) = http(port, "POST", "/api/call", &tasks, Some(json!({"method": "devices"}))).await;
    assert_eq!(s, 403);

    // Asking again under an id that is connected is refused without a
    // question: the question shows only the name the asker gives, so anything
    // could claim to be "Notes" and take Tasks' data. A code still works.
    let (s, r) = http(port, "POST", "/api/connect", "", Some(json!({"app": "tasks", "name": "Notes"}))).await;
    assert_eq!(s, 403, "{r}");
    assert!(node.asks().is_empty());
    assert_eq!(node.apps()["tasks"].name, "Tasks", "not renamed");
    let code = node.create_app_code().await.unwrap().code;
    let (s, _) = http(port, "POST", "/api/connect", "", Some(json!({"app": "tasks", "name": "Tasks", "code": code}))).await;
    assert_eq!(s, 200);

    // Connect with a code: wrong code refused, right code works once.
    let (s, _) = http(port, "POST", "/api/connect", "", Some(json!({"app": "calendar", "name": "Calendar", "code": "000000x"}))).await;
    assert_eq!(s, 403);
    let code = node.create_app_code().await.unwrap().code;
    let (s, r) = http(port, "POST", "/api/connect", "", Some(json!({"app": "calendar", "name": "Calendar", "code": code}))).await;
    assert_eq!(s, 200);
    let cal = r["token"].as_str().unwrap().to_string();
    let (s, _) = http(port, "POST", "/api/connect", "", Some(json!({"app": "x", "name": "X", "code": code}))).await;
    assert_eq!(s, 403);

    // Calendar can't read Tasks until it asks and the user allows.
    let (s, _) = http(port, "POST", "/api/call", &cal, Some(json!({"method": "list", "params": {"container": "tasks", "collection": "tasks"}}))).await;
    assert_eq!(s, 403);
    let n = node.clone();
    tokio::spawn(async move {
        loop {
            if let Some(a) = n.asks().first() {
                assert!(matches!(&a.ask, Ask::Access { container, .. } if container == "tasks"));
                n.answer(&a.id, true).unwrap();
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    });
    let (_, r) = http(port, "POST", "/api/call", &cal, Some(json!({"method": "request_access", "params": {"container": "tasks"}}))).await;
    assert_eq!(r["result"], true);
    let (s, _) = http(port, "POST", "/api/call", &cal, Some(json!({"method": "list", "params": {"container": "tasks", "collection": "tasks"}}))).await;
    assert_eq!(s, 200);
    let (s, _) = http(port, "POST", "/api/call", &cal, Some(json!({"method": "put", "params": {"container": "tasks", "collection": "tasks", "value": {}}}))).await;
    assert_eq!(s, 403);

    // Disconnecting the app (keeping its data) invalidates its token.
    node.uninstall_app("tasks", false).unwrap();
    let (s, _) = http(port, "POST", "/api/call", &tasks, Some(json!({"method": "list", "params": {"collection": "tasks"}}))).await;
    assert_eq!(s, 401);
    assert!(node.containers(&Client::Shell).unwrap().iter().any(|c| c.id == "tasks"));

    // Requests for other host names are refused (DNS rebinding).
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut st = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    st.write_all(b"GET /api HTTP/1.1\r\nHost: evil.example\r\nConnection: close\r\n\r\n").await.unwrap();
    let mut out = String::new();
    st.read_to_string(&mut out).await.unwrap();
    assert!(out.starts_with("HTTP/1.1 403"));
}

#[tokio::test]
async fn apps_keep_lists_in_their_own_containers() {
    let dir = tempfile::TempDir::new().unwrap();
    let node = Node::open(Config {
        data_dir: dir.path().into(),
        device_name: "Laptop".into(),
        platform: "linux".into(),
        net: NetMode::Local(MemoryLookup::new()),
    })
    .await
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = uuob_api::router(node.clone());
    tokio::spawn(async move { axum::serve(listener, app).await });

    let mut tokens = vec![];
    for (id, name) in [("tasks", "Tasks"), ("calendar", "Calendar")] {
        let code = node.create_app_code().await.unwrap().code;
        let (_, r) = http(port, "POST", "/api/connect", "", Some(json!({"app": id, "name": name, "code": code}))).await;
        tokens.push(r["token"].as_str().unwrap().to_string());
    }
    let (tasks, cal) = (&tokens[0], &tokens[1]);
    let call = |token: &str, method: &str, params: Value| {
        let token = token.to_string();
        let body = json!({"method": method, "params": params});
        async move { http(port, "POST", "/api/call", &token, Some(body)).await }
    };

    // Tasks makes a list and keeps tasks in it.
    let (s, r) = call(tasks, "create_container", json!({"name": "Groceries"})).await;
    assert_eq!(s, 200);
    let list = r["result"].as_str().unwrap().to_string();
    assert!(list.starts_with("tasks."));
    let (s, _) = call(tasks, "put", json!({"container": list, "collection": "tasks", "id": "t1", "value": {"title": "Milk"}})).await;
    assert_eq!(s, 200);
    let (_, r) = call(tasks, "containers", json!({})).await;
    let mine = r["result"].as_array().unwrap().iter().find(|c| c["id"] == list.as_str()).unwrap().clone();
    assert_eq!((mine["name"].as_str(), mine["can_write"].as_bool(), mine["from"].is_null()), (Some("Groceries"), Some(true), true));

    // Another app can't see, rename, share or delete it.
    for (method, params) in [
        ("list", json!({"container": list, "collection": "tasks"})),
        ("rename_container", json!({"id": list, "name": "Mine now"})),
        ("share", json!({"container": list})),
        ("delete_container", json!({"id": list})),
    ] {
        assert_eq!(call(cal, method, params).await.0, 403, "{method}");
    }
    let (_, r) = call(cal, "containers", json!({})).await;
    assert!(r["result"].as_array().unwrap().iter().all(|c| c["owner_app"] == "calendar"));

    // Share opens UUOB's sheet and waits for the user to finish there.
    let n = node.clone();
    let expect = list.clone();
    tokio::spawn(async move {
        loop {
            if let Some(a) = n.asks().first() {
                assert!(matches!(&a.ask, Ask::Share { container, container_name, .. } if *container == expect && container_name == "Groceries"));
                n.answer(&a.id, true).unwrap();
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    });
    let (_, r) = call(tasks, "share", json!({"container": list})).await;
    assert_eq!(r["result"], true);
    // The app's first container is private: it can't be shared or deleted by the app.
    assert_eq!(call(tasks, "share", json!({"container": "tasks"})).await.0, 400);
    assert_eq!(call(tasks, "delete_container", json!({"id": "tasks"})).await.0, 403);
    // People and sharing itself are for the UUOB app only.
    assert_eq!(call(tasks, "people", json!({})).await.0, 403);
    assert_eq!(call(tasks, "share_with", json!({"container": list, "person": "x", "access": "write"})).await.0, 403);

    // Rename and delete its own list.
    assert_eq!(call(tasks, "rename_container", json!({"id": list, "name": "Food"})).await.0, 200);
    assert_eq!(node.containers(&Client::Shell).unwrap().iter().find(|c| c.id == list).unwrap().name, "Food");
    assert_eq!(call(tasks, "delete_container", json!({"id": list})).await.0, 200);
    assert_ne!(call(tasks, "list", json!({"container": list, "collection": "tasks"})).await.0, 200);
}

/// A raw HTTP request. Returns the status, the response head (lower case)
/// and the body bytes.
async fn raw(port: u16, head: String, body: &[u8]) -> (u16, String, Vec<u8>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.write_all(body).await.unwrap();
    let mut out = Vec::new();
    stream.read_to_end(&mut out).await.unwrap();
    let split = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&out[..split]).to_lowercase();
    (head[9..12].parse().unwrap(), head, out[split + 4..].to_vec())
}

#[tokio::test]
async fn apps_store_and_read_files() {
    let dir = tempfile::TempDir::new().unwrap();
    let node = Node::open(Config {
        data_dir: dir.path().into(),
        device_name: "Laptop".into(),
        platform: "linux".into(),
        net: NetMode::Local(MemoryLookup::new()),
    })
    .await
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = uuob_api::router(node.clone());
    tokio::spawn(async move { axum::serve(listener, app).await });
    let mut tokens = vec![];
    for (id, name) in [("photos", "Photos"), ("tasks", "Tasks")] {
        let code = node.create_app_code().await.unwrap().code;
        let (_, r) = http(port, "POST", "/api/connect", "", Some(json!({"app": id, "name": name, "code": code}))).await;
        tokens.push(r["token"].as_str().unwrap().to_string());
    }
    let (photos, tasks) = (&tokens[0], &tokens[1]);
    let get = |path: String, token: &str, range: &str| {
        let auth = if token.is_empty() { String::new() } else { format!("Authorization: Bearer {token}\r\n") };
        let range = if range.is_empty() { String::new() } else { format!("Range: {range}\r\n") };
        async move {
            raw(port, format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n{auth}{range}Connection: close\r\n\r\n"), b"").await
        }
    };

    // Upload: the body is the file. Larger than any JSON body the API takes.
    let data: Vec<u8> = (0..3_000_000usize).map(|i| (i * 7 + i / 255) as u8).collect();
    let post = format!(
        "POST /api/files HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer {photos}\r\nContent-Type: video/mp4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        data.len()
    );
    let (s, _, body) = raw(port, post.clone(), &data).await;
    assert_eq!(s, 200);
    let info: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!((info["size"].as_u64(), info["type"].as_str()), (Some(3_000_000), Some("video/mp4")));
    let id = info["id"].as_str().unwrap();
    let anonymous = post.replace(&format!("Authorization: Bearer {photos}\r\n"), "").replace("3000000", "5");
    assert_eq!(raw(port, anonymous, b"hello").await.0, 401);

    // The whole file.
    let (s, head, body) = get(format!("/api/files/{id}"), photos, "").await;
    assert_eq!(s, 200);
    assert!(body == data, "the bytes come back unchanged");
    for expected in ["content-type: video/mp4", "accept-ranges: bytes", "content-length: 3000000", "nosniff"] {
        assert!(head.contains(expected), "{expected} in {head}");
    }

    // Parts of it, with the key in the address the way <video> asks.
    let keyed = format!("/api/files/{id}?key={photos}");
    let (s, head, body) = get(keyed.clone(), "", "bytes=70000-70009").await;
    assert_eq!((s, &body[..]), (206, &data[70000..70010]));
    assert!(head.contains("content-range: bytes 70000-70009/3000000"), "{head}");
    let (s, _, body) = get(keyed.clone(), "", "bytes=2999990-").await;
    assert_eq!((s, &body[..]), (206, &data[2_999_990..]));
    let (s, _, body) = get(keyed.clone(), "", "bytes=-5").await;
    assert_eq!((s, &body[..]), (206, &data[2_999_995..]));
    assert_eq!(get(keyed.clone(), "", "bytes=3000000-").await.0, 416);

    // Nobody else gets it.
    assert_eq!(get(format!("/api/files/{id}"), "", "").await.0, 401);
    assert_eq!(get(format!("/api/files/{id}?container=photos"), tasks, "").await.0, 403);
    assert_eq!(get(format!("/api/files/{}", "0".repeat(64)), photos, "").await.0, 404);

    // Listing and deleting go through the ordinary calls.
    let call = |method: &str, params: Value| {
        let body = json!({"method": method.to_string(), "params": params});
        let token = photos.clone();
        async move { http(port, "POST", "/api/call", &token, Some(body)).await.1 }
    };
    let list = call("files", json!({})).await;
    assert_eq!((list["result"][0]["id"].as_str(), list["result"][0]["here"].as_bool()), (Some(id), Some(true)));
    assert_eq!(call("delete_file", json!({"id": id})).await["result"], json!(true));
    assert_eq!(get(format!("/api/files/{id}"), photos, "").await.0, 404);
    assert_eq!(call("files", json!({})).await["result"], json!([]));
}

#[tokio::test]
async fn the_owner_key_acts_as_the_user() {
    let dir = tempfile::TempDir::new().unwrap();
    let open = || {
        Node::open(Config {
            data_dir: dir.path().into(),
            device_name: "Server".into(),
            platform: "linux".into(),
            net: NetMode::Local(MemoryLookup::new()),
        })
    };
    let node = open().await.unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = uuob_api::router(node.clone());
    tokio::spawn(async move { axum::serve(listener, app).await });

    // The key is in the data folder, readable by its owner only.
    let path = dir.path().join("owner.key");
    let key = std::fs::read_to_string(&path).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    }

    // With it, the local API does what the UUOB window can do.
    let (s, r) = http(port, "POST", "/api/call", &key, Some(json!({"method": "status"}))).await;
    assert_eq!((s, r["result"]["device_name"].as_str()), (200, Some("Server")));
    let (s, _) = http(port, "POST", "/api/call", &key, Some(json!({"method": "set_copies", "params": {"copies": 2}}))).await;
    assert_eq!(s, 200);
    assert_eq!(node.status().network.copies, Some(2));
    // Without it, or with a made-up one, nothing.
    for wrong in ["", "uuob_owner_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"] {
        let (s, _) = http(port, "POST", "/api/call", wrong, Some(json!({"method": "status"}))).await;
        assert_eq!(s, 401);
    }

    // Before sending the key, the `uuob` command checks the server can prove it holds it.
    let (_, hello) = http(port, "GET", "/api?prove=n1", "", None).await;
    let expected = uuob_core::sha256_hex(format!("{}:n1", uuob_core::sha256_hex(key.trim().as_bytes())).as_bytes());
    assert_eq!(hello["owner_proof"].as_str(), Some(expected.as_str()));

    // The key stays the same across restarts.
    node.shutdown().await;
    let node = open().await.unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), key);
    assert_eq!(node.authenticate(&key), Some(Client::Shell));
}

/// Read from `stream` until `what` shows up.
async fn read_until(stream: &mut tokio::net::TcpStream, seen: &mut String, what: &str) {
    use tokio::io::AsyncReadExt;
    let mut buf = [0u8; 4096];
    while !seen.contains(what) {
        let n = tokio::time::timeout(std::time::Duration::from_secs(10), stream.read(&mut buf)).await.expect(what).unwrap();
        assert!(n > 0, "the stream ended before: {what}");
        seen.push_str(&String::from_utf8_lossy(&buf[..n]));
    }
}

#[tokio::test]
async fn an_app_on_another_device_gets_the_app_routes_and_nothing_else() {
    let net = MemoryLookup::new();
    let dir = tempfile::TempDir::new().unwrap();
    let node = Node::open(Config {
        data_dir: dir.path().into(),
        device_name: "Server".into(),
        platform: "linux".into(),
        net: NetMode::Local(net.clone()),
    })
    .await
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let here = listener.local_addr().unwrap().port();
    let app = uuob_api::router(node.clone());
    tokio::spawn(async move { axum::serve(listener, app).await });
    tokio::spawn(uuob_api::serve_remote(node.clone()));

    // Photos runs on a device without UUOB and links with a code.
    let phone = tempfile::TempDir::new().unwrap();
    let tunnel = Tunnel::open(phone.path().into(), NetMode::Local(net.clone())).await.unwrap();
    let code = node.create_app_code().await.unwrap().code;
    let photos = tunnel.link(&code, &AppManifest { id: "photos".into(), name: "Photos".into() }).await.unwrap();
    let far = tunnel.port();
    let call = |port: u16, token: &str, method: &str, params: Value| {
        let token = token.to_string();
        let body = json!({"method": method, "params": params});
        async move { http(port, "POST", "/api/call", &token, Some(body)).await }
    };

    let (s, hello) = http(far, "GET", "/api", "", None).await;
    assert_eq!((s, hello["device"].as_str(), hello["remote"].as_bool()), (200, Some("Server"), Some(true)));
    assert_eq!(http(here, "GET", "/api", "", None).await.1["remote"], false);

    // Records, as on the device.
    let (s, _) = call(far, &photos, "put", json!({"collection": "photos", "id": "p1", "value": {"name": "beach.jpg"}})).await;
    assert_eq!(s, 200);
    let (_, r) = call(far, &photos, "list", json!({"collection": "photos"})).await;
    assert_eq!(r["result"][0]["name"], "beach.jpg");

    // Live changes.
    use tokio::io::AsyncWriteExt;
    let mut events = tokio::net::TcpStream::connect(("127.0.0.1", far)).await.unwrap();
    events.write_all(format!("GET /api/events HTTP/1.1\r\nHost: 127.0.0.1:{far}\r\nAuthorization: Bearer {photos}\r\n\r\n").as_bytes()).await.unwrap();
    let mut seen = String::new();
    read_until(&mut events, &mut seen, r#"{"container":"*"}"#).await;
    node.put(&Client::App("photos".into()), "photos", "photos", Some("p2"), json!({"name": "made on the server"})).unwrap();
    read_until(&mut events, &mut seen, r#"{"container":"photos"}"#).await;

    // Files, in ranges, with the token in the address (as <img> and <video> send it).
    let data: Vec<u8> = (0..1_500_000usize).map(|i| (i * 7 + i / 255) as u8).collect();
    let post = format!(
        "POST /api/files HTTP/1.1\r\nHost: 127.0.0.1:{far}\r\nAuthorization: Bearer {photos}\r\nContent-Type: video/mp4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        data.len()
    );
    let (s, _, body) = raw(far, post, &data).await;
    assert_eq!(s, 200);
    let id = serde_json::from_slice::<Value>(&body).unwrap()["id"].as_str().unwrap().to_string();
    let get = format!("GET /api/files/{id}?key={photos} HTTP/1.1\r\nHost: 127.0.0.1:{far}\r\nRange: bytes=1000000-1000999\r\nConnection: close\r\n\r\n");
    let (s, head, body) = raw(far, get, b"").await;
    assert_eq!(s, 206);
    assert!(head.contains("content-range: bytes 1000000-1000999/1500000"));
    assert_eq!(body, data[1_000_000..1_001_000]);

    // What the device keeps is not for an app elsewhere to decide.
    let (s, _) = call(far, &photos, "keep", json!({"container": "photos", "keep": "none"})).await;
    assert_eq!(s, 200);
    let kept = node.containers(&Client::Shell).unwrap().into_iter().find(|c| c.id == "photos").unwrap();
    assert_eq!(serde_json::to_value(kept.keep).unwrap(), "all");

    // Through the tunnel only the linked app's own token counts. The owner
    // key (which is the user, on the device) and other apps' tokens do not.
    let owner = std::fs::read_to_string(dir.path().join("owner.key")).unwrap();
    assert_eq!(call(here, &owner, "devices", json!({})).await.0, 200);
    assert_eq!(call(far, &owner, "devices", json!({})).await.0, 401);
    let code = node.create_app_code().await.unwrap().code;
    let (_, r) = http(here, "POST", "/api/connect", "", Some(json!({"app": "tasks", "name": "Tasks", "code": code}))).await;
    let tasks = r["token"].as_str().unwrap().to_string();
    assert_eq!(call(here, &tasks, "list", json!({"collection": "tasks"})).await.0, 200);
    assert_eq!(call(far, &tasks, "list", json!({"collection": "tasks"})).await.0, 401);
    // The app stays an app: no methods of the UUOB window.
    assert_eq!(call(far, &photos, "devices", json!({})).await.0, 403);
    // No connecting without a code, and no AI access, from outside.
    assert_eq!(http(far, "POST", "/api/connect", "", Some(json!({"app": "x", "name": "X"}))).await.0, 404);
    assert_eq!(http(far, "POST", "/mcp", &photos, Some(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize"}))).await.0, 404);

    // Disconnected in UUOB: its open stream of changes ends and it is turned away.
    node.uninstall_app("photos", false).unwrap();
    let mut gone = false;
    for _ in 0..40 {
        if tokio::net::TcpStream::connect(("127.0.0.1", far)).await.is_ok() && tunnel.removed() {
            gone = true;
            break;
        }
        let _ = raw_try(far).await;
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    assert!(gone, "the app learns it was disconnected");
}

/// One request that may fail (the tunnel drops it when no device takes the app).
async fn raw_try(port: u16) -> Option<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.ok()?;
    stream.write_all(format!("GET /api HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n").as_bytes()).await.ok()?;
    let mut out = Vec::new();
    stream.read_to_end(&mut out).await.ok()?;
    Some(())
}

/// An app on this device says which Android app it is: one package per app,
/// bound to the connection, gone when the app is disconnected.
#[tokio::test]
async fn apps_bind_their_android_package() {
    let dir = tempfile::TempDir::new().unwrap();
    let node = Node::open(Config {
        data_dir: dir.path().into(),
        device_name: "Phone".into(),
        platform: "android".into(),
        net: NetMode::Local(MemoryLookup::new()),
    })
    .await
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(axum::serve(listener, uuob_api::router(node.clone())).into_future());
    let connect = |app: &'static str| {
        let node = node.clone();
        async move {
            let code = node.create_app_code().await.unwrap().code;
            let (_, r) = http(port, "POST", "/api/connect", "", Some(json!({"app": app, "name": app, "code": code}))).await;
            r["token"].as_str().unwrap().to_string()
        }
    };
    let bind = |token: String, package: &'static str| async move {
        http(port, "POST", "/api/call", &token, Some(json!({"method": "android", "params": {"package": package}}))).await.0
    };

    let tasks = connect("tasks").await;
    assert_eq!(bind(tasks.clone(), "org.example.tasks").await, 200);
    assert_eq!(bind(tasks.clone(), "org.example.tasks").await, 200, "saying it again is fine");
    let first = node.android_packages()["tasks"]["org.example.tasks"].clone();
    // Another app can't take that package, and a package must be a package.
    let calendar = connect("calendar").await;
    assert_eq!(bind(calendar.clone(), "org.example.tasks").await, 403);
    assert_eq!(bind(calendar, "not a package").await, 400);
    // Only apps: an AI client is refused.
    let (_, ai) = node.create_client("Claude").unwrap();
    assert_eq!(bind(ai, "org.example.ai").await, 403);

    // Disconnected: the binding no longer counts. Approved again: a new connection.
    node.uninstall_app("tasks", false).unwrap();
    assert!(node.android_packages().get("tasks").is_none());
    let again = connect("tasks").await;
    assert_eq!(bind(again, "org.example.tasks").await, 200);
    assert_ne!(node.android_packages()["tasks"]["org.example.tasks"], first);
}

/// The server on UUOB's port can show an app (through Android, see
/// `ProveReceiver`) that it is this UUOB, and only while it holds the port.
#[tokio::test]
async fn the_server_proves_it_is_this_uuob() {
    let dir = tempfile::TempDir::new().unwrap();
    let node = Node::open(Config {
        data_dir: dir.path().into(),
        device_name: "Phone".into(),
        platform: "android".into(),
        net: NetMode::Local(MemoryLookup::new()),
    })
    .await
    .unwrap();
    assert_eq!(uuob_api::proof("n1"), None, "not listening yet");
    let port = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap().local_addr().unwrap().port();
    tokio::spawn(uuob_api::serve(node, ([127, 0, 0, 1], port).into()));
    for _ in 0..100 {
        if uuob_api::proof("n1").is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let (s, hello) = http(port, "GET", "/api?prove=n1", "", None).await;
    assert_eq!(s, 200);
    assert_eq!(hello["proof"].as_str(), uuob_api::proof("n1").as_deref());
    assert!(hello["proof"].is_string());
    assert_ne!(uuob_api::proof("n1"), uuob_api::proof("n2"));
    let (_, plain) = http(port, "GET", "/api", "", None).await;
    assert!(plain.get("proof").is_none());
}
