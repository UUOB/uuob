//! `uuob`: UUOB without a window, and a way to control the UUOB running on
//! this computer from the terminal.
//!
//! `uuob run` is a whole UUOB device (the node and its local server), for a
//! server or a Raspberry Pi. Every other command talks to the UUOB that is
//! running here (started by `uuob run` or by the desktop app) through its
//! local API, as the user: it reads the owner key from UUOB's data folder.
//! All commands end up in the same one entry point as the app's own screens
//! (`Node::call`), so nothing is implemented twice.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuob_core::{Config, NetMode, Node};

const HELP: &str = "\
uuob: your own backend, from the terminal

Start UUOB on this computer (no window):
  uuob run [--data <folder>] [--name <device name>]
  uuob autostart on|off       start it by itself when this computer turns on,
                              or only when you run `uuob run`

Control the UUOB running on this computer:
  uuob status                 this device and its network
  uuob devices                the devices of your network
  uuob invite                 show a code to add another device
  uuob join <code>            join the network of the device that showed the code
  uuob name <name>            rename this device
  uuob remove <device>        remove a device from the network
  uuob leave                  leave the network (this device keeps its data)
  uuob keep on|off [device]   make this (or another) device a storage device:
                              it keeps everything, so others can keep less
  uuob copies <n>             storage devices that must have something before
                              another device frees its own copy
  uuob cache <MB> [device]    how much space a device that is not a storage
                              device may use for files it has opened
  uuob people                 the people you are linked with
  uuob meet <code>            link with the person who showed the code
  uuob apps                   connected apps and how much they store
  uuob app-code               a code to connect an app
  uuob asks                   questions waiting for your answer
  uuob allow <id> | deny <id> answer one
  uuob call <method> [json]   anything else the UUOB window can do

Where UUOB keeps its data: --data, else $UUOB_DATA_DIR, else ~/.local/share/uuob.
The local port is $UUOB_PORT (7745 if unset).
";

#[tokio::main]
async fn main() {
    // `uuob status | head -1`: a reader that stops listening is not a failure.
    let report = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic| {
        if panic.to_string().contains("Broken pipe") {
            std::process::exit(0);
        }
        report(panic)
    }));
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = command(&args).await {
        eprintln!("uuob: {e:#}");
        std::process::exit(1);
    }
}

fn port() -> u16 {
    std::env::var("UUOB_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or_else(uuob_api::port)
}

fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

/// Where `uuob run` keeps its data.
fn default_data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("UUOB_DATA_DIR") {
        return dir.into();
    }
    std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home().join(".local/share")).join("uuob")
}

/// The owner key of the UUOB running here: from `uuob run`'s folder, or
/// from the desktop app's.
fn owner_key() -> Result<String> {
    let app = if cfg!(target_os = "macos") { "Library/Application Support/app.uuob" } else { ".local/share/app.uuob" };
    [default_data_dir(), home().join(app)]
        .iter()
        .find_map(|dir| std::fs::read_to_string(dir.join("owner.key")).ok())
        .map(|key| key.trim().to_string())
        .ok_or_else(|| anyhow!("no UUOB data found on this computer. Start one with `uuob run`."))
}

/// Call a method of the running UUOB, as the user.
async fn call(method: &str, params: Value) -> Result<Value> {
    let key = owner_key()?;
    let port = port();
    let body = json!({ "method": method, "params": params }).to_string();
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .map_err(|_| anyhow!("UUOB is not running on this computer. Start it with `uuob run`."))?;
    let request = format!(
        "POST /api/call HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer {key}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).await?;
    let mut response = String::new();
    stream.read_to_string(&mut response).await?;
    let json = response.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or("");
    let reply: Value = serde_json::from_str(json).context("UUOB sent an answer this command does not understand")?;
    if let Some(message) = reply["error"]["message"].as_str() {
        bail!("{message}");
    }
    Ok(reply["result"].clone())
}

async fn command(args: &[String]) -> Result<()> {
    let rest = args.get(1..).unwrap_or(&[]);
    let text = || rest.join(" ");
    let first = || rest.first().map(String::as_str).ok_or_else(|| anyhow!("something is missing; see `uuob help`"));
    match args.first().map(String::as_str) {
        None | Some("help" | "-h" | "--help") => print!("{HELP}"),
        Some("run") => run(rest).await?,
        Some("status") => status().await?,
        Some("autostart") => autostart(rest.first().map(String::as_str)).await?,
        Some("devices") => devices().await?,
        Some("invite") => {
            let invite = call("create_invite", json!({})).await?;
            println!("{}", invite["code"].as_str().unwrap_or_default());
            println!("On the other device: UUOB → Join a network (or `uuob join <code>`). The code works once, for a few minutes.");
        }
        Some("join") => {
            call("join", json!({ "code": text() })).await?;
            println!("Joined. Data will sync in a moment.");
            status().await?;
        }
        Some("name") => {
            call("set_device_name", json!({ "name": text() })).await?;
            status().await?;
        }
        Some("remove") => {
            let device = device(Some(&text())).await?;
            call("remove_device", json!({ "id": device["id"] })).await?;
            println!("Removed {}.", device["name"].as_str().unwrap_or_default());
        }
        Some("leave") => {
            call("leave_network", json!({})).await?;
            println!("This device now has a network of its own.");
        }
        Some("keep") => {
            let on = match first()? {
                "on" => true,
                "off" => false,
                _ => bail!("say `uuob keep on` or `uuob keep off`"),
            };
            let name = rest.get(1..).filter(|r| !r.is_empty()).map(|r| r.join(" "));
            let device = device(name.as_deref()).await?;
            call("set_keeper", json!({ "device": device["id"], "on": on })).await?;
            devices().await?;
        }
        Some("cache") => {
            let megabytes: u64 = first()?.parse().map_err(|_| anyhow!("give the size in megabytes"))?;
            let name = rest.get(1..).filter(|r| !r.is_empty()).map(|r| r.join(" "));
            let device = device(name.as_deref()).await?;
            call("set_cache", json!({ "device": device["id"], "bytes": megabytes << 20 })).await?;
            println!("{} caches up to {megabytes} MB of opened files.", device["name"].as_str().unwrap_or_default());
        }
        Some("copies") => {
            let copies: u8 = first()?.parse().map_err(|_| anyhow!("copies must be a number"))?;
            call("set_copies", json!({ "copies": copies })).await?;
            status().await?;
        }
        Some("people") => {
            let people = call("people", json!({})).await?;
            for p in people.as_array().into_iter().flatten() {
                let count = |k: &str| p[k].as_array().map_or(0, Vec::len);
                println!(
                    "{}  {}  you share {}, they share {}",
                    p["name"].as_str().unwrap_or("?"),
                    if p["online"] == true { "online" } else { "offline" },
                    count("sharing"),
                    count("receiving")
                );
            }
            if people.as_array().is_none_or(Vec::is_empty) {
                println!("You are not linked with anyone.");
            }
        }
        Some("meet") => {
            call("meet", json!({ "code": text() })).await?;
            println!("Linked. Nothing is shared until one of you shares something.");
        }
        Some("apps") => apps().await?,
        Some("app-code") => {
            let code = call("create_app_code", json!({})).await?;
            println!("{}", code["code"].as_str().unwrap_or_default());
            println!("Type it into the app within 10 minutes.");
        }
        Some("asks") => {
            let asks = call("asks", json!({})).await?;
            for a in asks.as_array().into_iter().flatten() {
                let what = match a["kind"].as_str() {
                    Some("connect") => format!("{} wants to keep its data in your network", a["name"].as_str().unwrap_or("An app")),
                    Some("access") => format!(
                        "{} wants to {} {}",
                        a["app_name"].as_str().unwrap_or("An app"),
                        if a["access"] == "write" { "read and change" } else { "read" },
                        a["container_name"].as_str().unwrap_or("another app's data")
                    ),
                    _ => a.to_string(),
                };
                let id = a["id"].as_str().unwrap_or_default();
                println!("{id}  {what}   (uuob allow {id} | uuob deny {id})");
            }
            if asks.as_array().is_none_or(Vec::is_empty) {
                println!("Nothing is waiting for you.");
            }
        }
        Some(answer @ ("allow" | "deny")) => {
            call("answer", json!({ "id": first()?, "allow": answer == "allow" })).await?;
        }
        Some("call") => {
            let params = match rest.get(1) {
                Some(json) => serde_json::from_str(json).context("the parameters must be JSON")?,
                None => json!({}),
            };
            println!("{}", serde_json::to_string_pretty(&call(first()?, params).await?)?);
        }
        Some(other) => bail!("unknown command `{other}`; see `uuob help`"),
    }
    Ok(())
}

/// A device by name or by the start of its id; this device if none is named.
async fn device(name: Option<&str>) -> Result<Value> {
    let devices = call("devices", json!({})).await?;
    let all = devices.as_array().cloned().unwrap_or_default();
    let found = all.into_iter().find(|d| match name {
        None => d["is_self"] == true,
        Some(n) => d["name"].as_str().is_some_and(|x| x.eq_ignore_ascii_case(n)) || d["id"].as_str().is_some_and(|id| n.len() >= 6 && id.starts_with(n)),
    });
    found.ok_or_else(|| anyhow!("no device called {}; see `uuob devices`", name.unwrap_or("that")))
}

async fn status() -> Result<()> {
    let s = call("status", json!({})).await?;
    let me = device(None).await?;
    println!("Device   {} ({})", s["device_name"].as_str().unwrap_or("?"), s["platform"].as_str().unwrap_or("?"));
    println!("Network  {} of {} devices online", s["devices_online"], s["devices_total"]);
    println!(
        "Storage  {}; copies needed: {}",
        if me["keeper"] == true {
            "this device keeps everything".to_string()
        } else {
            format!("this device keeps what its apps ask for (cache: {} MB)", me["cache_bytes"].as_u64().unwrap_or(0) >> 20)
        },
        s["network"]["copies"].as_u64().unwrap_or(1)
    );
    if has_systemd() {
        println!(
            "Startup  {}",
            if autostarts() {
                "starts by itself when this computer turns on"
            } else {
                "runs only when started (`uuob autostart on` changes that)"
            }
        );
    }
    Ok(())
}

// ---- Starting when the computer turns on (Linux with systemd) -------------

const SERVICE: &str = "/etc/systemd/system/uuob.service";

fn has_systemd() -> bool {
    std::path::Path::new("/run/systemd/system").exists()
}

fn output(program: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(program).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn autostarts() -> bool {
    output("systemctl", &["is-enabled", "uuob"]).is_some()
}

/// Run a command that changes the system: as is when we are root, with `sudo` otherwise.
fn as_root(args: &[&str]) -> Result<()> {
    let root = output("id", &["-u"]).as_deref() == Some("0");
    let (program, args) = if root { (args[0], &args[1..]) } else { ("sudo", args) };
    let done = std::process::Command::new(program).args(args).status().with_context(|| format!("could not run {program}"))?;
    if !done.success() {
        bail!("`{}` did not succeed", args.join(" "));
    }
    Ok(())
}

/// Have systemd start `uuob run` when this computer turns on, or stop doing
/// that. The service is the same UUOB the other commands talk to: the same
/// user, data folder and port as the person who turned it on.
async fn autostart(want: Option<&str>) -> Result<()> {
    if !has_systemd() {
        bail!("this works on Linux with systemd. On a Mac, use the switch in the UUOB app; elsewhere start `uuob run` yourself");
    }
    match want {
        None => println!("{}", if autostarts() { "on" } else { "off" }),
        Some("on") => {
            if std::env::var_os("SUDO_USER").is_some() {
                bail!("run `uuob autostart on` as yourself, without sudo: it asks for your password when it needs it");
            }
            let user = output("id", &["-un"]).ok_or_else(|| anyhow!("could not tell who you are"))?;
            let program = std::env::current_exe().context("could not find the uuob program")?;
            let mut service = format!(
                "[Unit]\nDescription=UUOB\nAfter=network-online.target\nWants=network-online.target\n\n[Service]\nUser={user}\nExecStart={} run\nRestart=on-failure\n",
                program.display()
            );
            // A data folder or port chosen for this shell is the service's too.
            for name in ["UUOB_DATA_DIR", "UUOB_PORT"] {
                if let Ok(value) = std::env::var(name) {
                    // Quoted, so a folder with a space in its name stays whole.
                    service += &format!("Environment=\"{name}={}\"\n", value.replace('\\', "\\\\").replace('"', "\\\""));
                }
            }
            service += "\n[Install]\nWantedBy=multi-user.target\n";
            let draft = std::env::temp_dir().join(format!("uuob.service.{}", std::process::id()));
            std::fs::write(&draft, service)?;
            let installed = as_root(&["install", "-m", "644", &draft.to_string_lossy(), SERVICE]);
            let _ = std::fs::remove_file(&draft);
            installed?;
            as_root(&["systemctl", "daemon-reload"])?;
            as_root(&["systemctl", "enable", "--quiet", "uuob"])?;
            let service_runs = output("systemctl", &["is-active", "uuob"]).is_some();
            if !service_runs && call("status", json!({})).await.is_ok() {
                // Started by hand: a second one could not use the same port.
                println!("UUOB starts by itself from the next time this computer turns on. The one running now stays as it is.");
                return Ok(());
            }
            as_root(&["systemctl", "restart", "uuob"])?;
            for _ in 0..60 {
                if call("status", json!({})).await.is_ok() {
                    println!("UUOB is running and starts by itself when this computer turns on.");
                    return Ok(());
                }
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
            bail!("UUOB was set to start by itself but did not come up; see `journalctl -u uuob`");
        }
        Some("off") => {
            if autostarts() {
                as_root(&["systemctl", "disable", "--quiet", "uuob"])?;
            }
            println!("UUOB no longer starts by itself.");
            if output("systemctl", &["is-active", "uuob"]).is_some() {
                println!("It keeps running until this computer restarts; to stop it now: sudo systemctl stop uuob");
            }
            println!("Start it when you need it with `uuob run`.");
        }
        _ => bail!("say `uuob autostart on` or `uuob autostart off`"),
    }
    Ok(())
}

async fn devices() -> Result<()> {
    for d in call("devices", json!({})).await?.as_array().into_iter().flatten() {
        println!(
            "{} {:<24} {:<8} {:<8} {:<15} {}",
            if d["is_self"] == true { "*" } else { " " },
            d["name"].as_str().unwrap_or("?"),
            if d["online"] == true { "online" } else { "offline" },
            d["platform"].as_str().unwrap_or("?"),
            if d["keeper"] == true { "storage device" } else { "" },
            &d["id"].as_str().unwrap_or("")[..10.min(d["id"].as_str().map_or(0, str::len))],
        );
    }
    Ok(())
}

async fn apps() -> Result<()> {
    let apps = call("apps", json!({})).await?;
    let containers = call("containers", json!({})).await?;
    for (id, app) in apps.as_object().into_iter().flatten() {
        let mine: Vec<&Value> = containers.as_array().into_iter().flatten().filter(|c| c["owner_app"] == id.as_str()).collect();
        let sum = |k: &str| mine.iter().map(|c| c[k].as_u64().unwrap_or(0)).sum::<u64>();
        println!(
            "{:<12} {} items, {} files ({} here), {:.1} MB{}",
            app["name"].as_str().unwrap_or(id),
            sum("records"),
            sum("files"),
            sum("files_here"),
            (sum("size_bytes") + sum("file_bytes")) as f64 / 1_048_576.0,
            if app["installed"] == true { "" } else { "  (not connected)" },
        );
    }
    Ok(())
}

/// A whole UUOB device, without a window.
async fn run(args: &[String]) -> Result<()> {
    let option = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let data_dir = option("--data").map(PathBuf::from).unwrap_or_else(default_data_dir);
    let device_name = option("--name")
        .or_else(|| std::fs::read_to_string("/etc/hostname").ok().map(|h| h.trim().to_string()).filter(|h| !h.is_empty()))
        .unwrap_or_else(|| "UUOB server".into());
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn,uuob_core=info,uuob_api=info".into()))
        .init();

    let node = Node::open(Config {
        data_dir: data_dir.clone(),
        device_name,
        platform: std::env::consts::OS.into(),
        net: NetMode::Internet,
    })
    .await?;
    let status = node.status();
    println!("UUOB is running: {} ({} of {} devices online).", status.device_name, status.devices_online, status.devices_total);
    println!("Data: {}. Link it with `uuob join <code>` or `uuob invite`; see `uuob help`.", data_dir.display());

    let server = uuob_api::serve(node.clone(), ([127, 0, 0, 1], port()).into());
    tokio::select! {
        r = server => r.with_context(|| format!("the local port {} is in use (is UUOB already running?)", port()))?,
        _ = stop() => {}
    }
    node.shutdown().await;
    Ok(())
}

/// Ctrl-C, or the stop signal a service manager or Docker sends.
async fn stop() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = signal(SignalKind::terminate()).expect("signal handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
}
