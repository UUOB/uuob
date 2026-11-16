//! Apps on a device that has no UUOB.
//!
//! Such an app reaches a UUOB on another of the user's devices through a
//! tunnel: the same end-to-end encrypted connection the devices use between
//! themselves (iroh), carrying the unchanged local API.
//!
//! - The user types a code from UUOB → Apps into the app. That is the
//!   permission: the app gets a token for its own data, and UUOB remembers
//!   the app's endpoint id next to it (`ClientInfo::endpoint`).
//! - That entry is in the network document, so *every* device of the network
//!   will serve the app; it uses whichever answers first.
//! - Nobody else gets in: a device closes connections from endpoints that
//!   are not linked apps, and a token works only from the app it was given to
//!   (`Node::authenticate_from`).
//!
//! The device's side is [`AppHandler`]: it hands each stream of a linked app
//! to the local server ([`Node::remote_requests`]). The app's side is
//! [`Tunnel`]: a port on 127.0.0.1 that forwards to a device.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use iroh::endpoint::{Connection, ConnectionError, RecvStream, SendStream};
use iroh::protocol::{AcceptError, ProtocolHandler};
use iroh::{Endpoint, EndpointId, SecretKey};
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::crypto;
use crate::error::{Error, Result};
use crate::net;
use crate::node::{APP_ALPN, AppManifest, Inner, NetMode, Node, bind_endpoint_with};
use crate::wire::{read_frame, write_frame};

/// Why a device closed an app's connection.
const UNKNOWN: u32 = 1; // it does not know this app (yet, perhaps)
const REMOVED: u32 = 2; // the user cut this app off

/// One connection from a linked app, speaking HTTP like a local one.
pub struct Request {
    /// The app's endpoint id (see [`Node::authenticate_from`]).
    pub app: String,
    pub io: tokio::io::Join<RecvStream, SendStream>,
}

impl Node {
    /// Connections of apps on other devices arrive here, to be served with
    /// the app routes of the local API. One receiver at a time; without one,
    /// such apps are turned away.
    pub fn remote_requests(&self) -> mpsc::UnboundedReceiver<Request> {
        let (tx, rx) = mpsc::unbounded_channel();
        *self.0.remote.lock().unwrap() = Some(tx);
        rx
    }

    /// Is this the endpoint of a linked app? Otherwise: was it cut off?
    fn linked(&self, endpoint: &str) -> std::result::Result<(), u32> {
        self.read_net(|d| {
            if net::clients(d).values().any(|c| c.endpoint.as_deref() == Some(endpoint)) {
                Ok(())
            } else if net::unlinked(d, endpoint) {
                Err(REMOVED)
            } else {
                Err(UNKNOWN)
            }
        })
    }
}

#[derive(Debug)]
pub(crate) struct AppHandler(pub Weak<Inner>);

impl ProtocolHandler for AppHandler {
    async fn accept(&self, conn: Connection) -> std::result::Result<(), AcceptError> {
        let Some(node) = Node::from_weak(&self.0) else { return Ok(()) };
        let app = conn.remote_id().to_string();
        if let Err(why) = node.linked(&app) {
            debug!("turning away an app that is not linked");
            conn.close(why.into(), b"not linked");
            return Ok(());
        }
        // The devices that will serve this app, so it learns of new ones.
        let devices: Vec<String> = node.devices().into_iter().map(|d| d.id).collect();
        if let Ok(mut send) = conn.open_uni().await {
            if write_frame(&mut send, &devices).await.is_ok() {
                let _ = send.finish();
            }
        }
        while let Ok((send, recv)) = conn.accept_bi().await {
            if let Err(why) = node.linked(&app) {
                conn.close(why.into(), b"not linked");
                break;
            }
            let server = node.0.remote.lock().unwrap().clone();
            let sent = server.is_some_and(|tx| tx.send(Request { app: app.clone(), io: tokio::io::join(recv, send) }).is_ok());
            if !sent {
                conn.close(UNKNOWN.into(), b"no server");
                break;
            }
        }
        Ok(())
    }
}

/// What a linked app keeps: its own identity and the devices to reach.
#[derive(Serialize, Deserialize)]
struct Link {
    key: String,
    devices: Vec<String>,
}

struct Linked {
    endpoint: Endpoint,
    devices: Vec<String>,
    conn: Option<Connection>,
    /// Devices are being asked right now.
    dialing: bool,
    /// The last time they were asked, none answered: don't keep the app waiting.
    away: bool,
    tried: Option<Instant>,
}

impl Linked {
    fn new(endpoint: Endpoint, devices: Vec<String>) -> Linked {
        Linked { endpoint, devices, conn: None, dialing: false, away: false, tried: None }
    }
}

struct TunnelInner {
    file: PathBuf,
    net: NetMode,
    port: u16,
    link: tokio::sync::Mutex<Option<Linked>>,
    /// Devices were asked and the answer is in.
    dialed: tokio::sync::Notify,
    removed: AtomicBool,
}

/// The app's side: a port on 127.0.0.1 that speaks UUOB's local API and
/// forwards it to one of the user's devices. Cheap to clone.
#[derive(Clone)]
pub struct Tunnel(Arc<TunnelInner>);

/// After no device answered: ask again this much later at the earliest.
const RETRY_AFTER: Duration = Duration::from_secs(3);
const DIAL_TIMEOUT: Duration = Duration::from_secs(8);
/// How long a request waits for a device to be found before it fails (the
/// app then works on what it last saw and queues changes).
const WAIT: Duration = Duration::from_secs(3);
/// A device that has been silent this long is gone (asleep, out of reach):
/// its connection is dropped and another device is asked.
const IDLE: Duration = Duration::from_secs(6);

impl Tunnel {
    /// Start the tunnel, linked or not (`dir`: where the app keeps its link).
    pub async fn open(dir: PathBuf, net: NetMode) -> Result<Tunnel> {
        std::fs::create_dir_all(&dir).map_err(anyhow::Error::from)?;
        let file = dir.join("uuob-link.json");
        let saved: Option<Link> = std::fs::read(&file).ok().and_then(|b| serde_json::from_slice(&b).ok());
        let link = match saved {
            Some(l) => {
                let key: [u8; 32] = hex::decode(&l.key)
                    .ok()
                    .and_then(|k| k.try_into().ok())
                    .ok_or_else(|| Error::Invalid("the saved link is damaged".into()))?;
                let endpoint = bind_endpoint_with(&net, SecretKey::from_bytes(&key), vec![], Some(IDLE)).await?;
                Some(Linked::new(endpoint, l.devices))
            }
            None => None,
        };
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.map_err(anyhow::Error::from)?;
        let port = listener.local_addr().map_err(anyhow::Error::from)?.port();
        let tunnel = Tunnel(Arc::new(TunnelInner {
            file,
            net,
            port,
            link: tokio::sync::Mutex::new(link),
            dialed: tokio::sync::Notify::new(),
            removed: AtomicBool::new(false),
        }));
        let weak = Arc::downgrade(&tunnel.0);
        tokio::spawn(async move {
            while let Ok((tcp, _)) = listener.accept().await {
                let Some(inner) = weak.upgrade() else { return };
                tokio::spawn(async move { Tunnel(inner).forward(tcp).await });
            }
        });
        Ok(tunnel)
    }

    /// The local port apps talk to.
    pub fn port(&self) -> u16 {
        self.0.port
    }

    /// Is this app linked to a UUOB on another device?
    pub async fn linked(&self) -> bool {
        self.0.link.lock().await.is_some()
    }

    /// The user cut this app off in UUOB (the link is gone; a new code links
    /// it again).
    pub fn removed(&self) -> bool {
        self.0.removed.load(Ordering::Relaxed)
    }

    /// Link with the code the user copied from UUOB → Apps on another
    /// device. Returns the app's token.
    pub async fn link(&self, code: &str, app: &AppManifest) -> Result<String> {
        let key = crypto::random32();
        let endpoint = bind_endpoint_with(&self.0.net, SecretKey::from_bytes(&key), vec![], Some(IDLE)).await?;
        let (token, devices) = match crate::pairing::link_app(&endpoint, code, app).await {
            Ok(r) => r,
            Err(e) => {
                endpoint.close().await;
                return Err(e);
            }
        };
        self.save(&Link { key: hex::encode(key), devices: devices.clone() })?;
        let old = self.0.link.lock().await.replace(Linked::new(endpoint, devices));
        if let Some(old) = old {
            old.endpoint.close().await;
        }
        self.0.removed.store(false, Ordering::Relaxed);
        info!("linked to a UUOB on another device");
        Ok(token)
    }

    fn save(&self, link: &Link) -> Result<()> {
        let bytes = serde_json::to_vec(link).map_err(anyhow::Error::from)?;
        std::fs::write(&self.0.file, bytes).map_err(anyhow::Error::from)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.0.file, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    /// One local connection: pass its bytes to a device and back. Without
    /// a device the connection is dropped at once, which the app reads as
    /// "UUOB is unreachable".
    async fn forward(&self, mut tcp: TcpStream) {
        let Some(conn) = self.connection().await else { return };
        let Ok((send, recv)) = conn.open_bi().await else { return };
        let mut far = tokio::io::join(recv, send);
        let _ = tokio::io::copy_bidirectional(&mut tcp, &mut far).await;
    }

    /// The connection to a device. If there is none, the devices are asked
    /// (in the background, one round at a time); a request waits a moment
    /// for the answer unless the last round found nobody, so that an app
    /// whose devices are all off is told so at once.
    async fn connection(&self) -> Option<Connection> {
        loop {
            let dialed = self.0.dialed.notified();
            tokio::pin!(dialed);
            dialed.as_mut().enable();
            {
                let mut link = self.0.link.lock().await;
                let l = link.as_mut()?;
                if let Some(c) = l.conn.take() {
                    match c.close_reason() {
                        None => {
                            l.conn = Some(c.clone());
                            return Some(c);
                        }
                        Some(why) if was_removed(&why) => {
                            self.unlink(&mut link).await;
                            return None;
                        }
                        Some(_) => {}
                    }
                }
                if !l.dialing && !l.tried.is_some_and(|at| at.elapsed() < RETRY_AFTER) {
                    l.dialing = true;
                    tokio::spawn(self.clone().ask_devices(l.endpoint.clone(), l.devices.clone()));
                }
                if l.away || !l.dialing {
                    return None;
                }
            }
            if tokio::time::timeout(WAIT, dialed).await.is_err() {
                return None;
            }
        }
    }

    /// Ask every device; the first that takes us serves us.
    async fn ask_devices(self, endpoint: Endpoint, devices: Vec<String>) {
        let mut tries = tokio::task::JoinSet::new();
        for id in devices.iter().filter_map(|d| d.parse::<EndpointId>().ok()) {
            let endpoint = endpoint.clone();
            tries.spawn(async move { tokio::time::timeout(DIAL_TIMEOUT, dial(endpoint, id)).await.unwrap_or(Err(false)) });
        }
        let mut found = None;
        let mut removed = false;
        while let Some(done) = tries.join_next().await {
            match done {
                Ok(Ok(taken)) => {
                    found = Some(taken);
                    break;
                }
                Ok(Err(gone)) => removed |= gone,
                Err(_) => {}
            }
        }
        drop(tries);
        let mut link = self.0.link.lock().await;
        // Linked anew meanwhile: this round was for the old link.
        if let Some(l) = link.as_mut().filter(|l| l.endpoint.id() == endpoint.id()) {
            l.dialing = false;
            l.tried = Some(Instant::now());
            l.away = found.is_none();
            match found {
                Some((conn, devices)) => {
                    if devices != l.devices && !devices.is_empty() {
                        l.devices = devices;
                        let key = hex::encode(l.endpoint.secret_key().to_bytes());
                        let _ = self.save(&Link { key, devices: l.devices.clone() });
                    }
                    l.conn = Some(conn);
                    l.tried = None;
                }
                None if removed => self.unlink(&mut link).await,
                None => {}
            }
        }
        drop(link);
        self.0.dialed.notify_waiters();
    }

    async fn unlink(&self, link: &mut Option<Linked>) {
        if let Some(l) = link.take() {
            l.endpoint.close().await;
        }
        let _ = std::fs::remove_file(&self.0.file);
        self.0.removed.store(true, Ordering::Relaxed);
        info!("this app was disconnected in UUOB");
    }
}

fn was_removed(why: &ConnectionError) -> bool {
    matches!(why, ConnectionError::ApplicationClosed(c) if c.error_code == REMOVED.into())
}

/// Connect to one device and wait until it has taken us (it then sends the
/// list of devices). `Err(true)`: it says the user cut this app off.
async fn dial(endpoint: Endpoint, device: EndpointId) -> std::result::Result<(Connection, Vec<String>), bool> {
    let conn = endpoint.connect(device, APP_ALPN).await.map_err(|_| false)?;
    let hello = async {
        let mut recv = conn.accept_uni().await?;
        read_frame::<Vec<String>>(&mut recv).await
    };
    match hello.await {
        Ok(devices) => Ok((conn, devices)),
        Err(_) => Err(conn.close_reason().is_some_and(|why| was_removed(&why))),
    }
}

/// A name for this device, e.g. "Pixel 8" or the computer's name: what a
/// new UUOB device is called at first, and what UUOB → Apps shows for an
/// app linked from here.
pub fn device_name() -> String {
    // Hostnames on Android are just "localhost".
    #[cfg(target_os = "android")]
    let name = std::process::Command::new("getprop").arg("ro.product.model").output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    #[cfg(not(target_os = "android"))]
    let name = gethostname::gethostname().to_string_lossy().trim().to_string();
    if !name.is_empty() && name != "localhost" {
        return name;
    }
    if cfg!(target_os = "android") { "Android phone" } else if cfg!(target_os = "ios") { "iPhone or iPad" } else { "Unnamed device" }.into()
}
