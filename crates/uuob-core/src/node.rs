//! The UUOB node: one per device. It owns local storage, enforces
//! permissions, and keeps every container in sync with the other devices of
//! the network.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use automerge::{AutoCommit, ROOT, ReadDoc};
use iroh::address_lookup::MemoryLookup;
use iroh::endpoint::{Connection, presets};
use iroh::protocol::Router;
use iroh::{Endpoint, EndpointAddr, EndpointId, RelayMode, SecretKey};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value as Json};
use tokio::sync::{Notify, broadcast, watch};
use tracing::{info, warn};

use crate::crypto::{self, FileKeyStore, KeyStore};
use crate::doc;
use crate::error::{Error, Result};
use crate::net::{self, Access, AppInfo, ClientInfo, ContainerInfo, Inbox, Keep, Member, NET_DOC, NetworkInfo};
use crate::store::{Store, now_ms};

pub const SYNC_ALPN: &[u8] = b"uuob/sync/2";
pub const PAIR_ALPN: &[u8] = b"uuob/pair/1";
pub const APP_ALPN: &[u8] = b"uuob/app/1";

/// How the node reaches other devices.
#[derive(Clone, Debug)]
pub enum NetMode {
    /// Normal operation: public relays (ciphertext only) and DNS/pkarr discovery.
    Internet,
    /// No relays and no public discovery; endpoints find each other through
    /// the shared in-memory lookup. Used by tests and offline demos.
    Local(MemoryLookup),
}

pub struct Config {
    pub data_dir: PathBuf,
    pub device_name: String,
    pub platform: String,
    pub net: NetMode,
    /// Where the master key lives. Defaults to `<data_dir>/keys`.
    pub keystore: Option<Arc<dyn KeyStore>>,
}

/// Who is making a request. Permissions are enforced against this.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Client {
    /// The UUOB shell itself, acting for the user. Full access.
    Shell,
    /// An installed app. Owns its containers (the first one has the app's id).
    App(String),
    /// An AI (MCP) or API client created on the AI access screen.
    Ai(String),
}

impl Client {
    pub fn key(&self) -> String {
        match self {
            Client::Shell => "shell".into(),
            Client::App(a) => format!("app:{a}"),
            Client::Ai(a) => format!("ai:{a}"),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    ContainerChanged { container: String },
    NetworkChanged,
    PeerConnected { id: String },
    PeerDisconnected { id: String },
    DeviceJoined { id: String, name: String },
    RemovedFromNetwork,
    /// The list of questions waiting for the user changed (see [`Node::asks`]).
    AsksChanged,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AppManifest {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub added_at: i64,
    pub is_self: bool,
    pub online: bool,
    /// A storage device: it keeps everything.
    pub keeper: bool,
    /// How many bytes of files it caches for containers it keeps only the list of.
    pub cache_bytes: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Status {
    pub device_id: String,
    pub device_name: String,
    pub platform: String,
    pub network: NetworkInfo,
    pub devices_total: usize,
    pub devices_online: usize,
    /// Documents held in memory right now (the rest are on disk only).
    pub documents_in_memory: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct AccessEntry {
    pub client: String,
    pub client_name: String,
    pub access: Access,
}

#[derive(Clone, Debug, Serialize)]
pub struct ContainerSummary {
    pub id: String,
    pub name: String,
    pub owner_app: String,
    pub app_installed: bool,
    pub size_bytes: u64,
    pub records: usize,
    pub collections: BTreeMap<String, usize>,
    /// Files it lists and their total size (whether or not they are on this device).
    pub files: usize,
    pub file_bytes: u64,
    /// How many of those files are on this device.
    pub files_here: usize,
    /// What this device is asked to keep of it, and whether its records are here.
    pub keep: Keep,
    pub here: bool,
    /// Connected storage devices that hold it exactly as this device does.
    pub copies: usize,
    pub access: Vec<AccessEntry>,
    /// Whether the asking client may change it (a container someone shared
    /// with you can be view-only).
    pub can_write: bool,
    /// The person who shared it with you, if it is not yours.
    pub from: Option<PersonRef>,
    /// People you share it with.
    pub shared_with: Vec<SharedWith>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PersonRef {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct SharedWith {
    pub id: String,
    pub name: String,
    pub access: Access,
}

#[derive(Clone, Debug)]
pub(crate) struct DocChange {
    pub doc: String,
    /// False when only what this device holds changed (a file arrived or
    /// was dropped), not the document itself.
    pub content: bool,
}

/// Changes waiting in the store are folded into a full save once they pass
/// this size (and a third of the document).
const FOLD_AFTER: u64 = 256 * 1024;
/// How much may stay in memory: this many documents, or this many bytes as
/// stored on disk, whichever is reached first.
const MAX_LOADED: usize = 32;
const MAX_LOADED_BYTES: u64 = 64 * 1024 * 1024;

/// The documents in memory. Every change is saved as it happens, so dropping
/// one loses nothing; only the recently used ones are kept.
#[derive(Default)]
pub(crate) struct Docs {
    map: HashMap<String, Loaded>,
    clock: u64,
}

struct Loaded {
    doc: AutoCommit,
    size: u64,
    used: u64,
}

impl Docs {
    fn contains(&self, id: &str) -> bool {
        self.map.contains_key(id)
    }

    fn get(&mut self, id: &str) -> Option<&mut AutoCommit> {
        self.clock += 1;
        let l = self.map.get_mut(id)?;
        l.used = self.clock;
        Some(&mut l.doc)
    }

    fn insert(&mut self, id: &str, doc: AutoCommit, size: u64) {
        self.clock += 1;
        self.map.insert(id.to_string(), Loaded { doc, size, used: self.clock });
    }

    fn grew(&mut self, id: &str, by: u64) {
        if let Some(l) = self.map.get_mut(id) {
            l.size += by;
        }
    }

    fn remove(&mut self, id: &str) {
        self.map.remove(id);
    }

    /// Drop the least recently used documents beyond the budget. The network
    /// document and `keep` (the one being used) always stay.
    fn trim(&mut self, keep: &str) {
        loop {
            let bytes: u64 = self.map.values().map(|l| l.size).sum();
            if self.map.len() <= MAX_LOADED && bytes <= MAX_LOADED_BYTES {
                return;
            }
            let oldest = self
                .map
                .iter()
                .filter(|(id, _)| *id != NET_DOC && *id != keep)
                .min_by_key(|(_, l)| l.used)
                .map(|(id, _)| id.clone());
            match oldest {
                Some(id) => self.map.remove(&id),
                None => return,
            };
        }
    }
}

pub(crate) struct PeerEntry {
    pub session: u64,
    pub initiated_by_me: bool,
    pub conn: Connection,
    pub since: std::time::Instant,
}

pub(crate) struct Inner {
    pub(crate) me: EndpointId,
    pub(crate) device_name: Mutex<String>,
    pub(crate) platform: String,
    pub(crate) net_mode: NetMode,
    pub(crate) store: Mutex<Store>,
    pub(crate) docs: Mutex<Docs>,
    pub(crate) endpoint: Endpoint,
    pub(crate) router: Mutex<Option<Router>>,
    pub(crate) events: broadcast::Sender<Event>,
    pub(crate) doc_changes: broadcast::Sender<DocChange>,
    pub(crate) peers: Mutex<HashMap<EndpointId, PeerEntry>>,
    pub(crate) dialing: Mutex<HashSet<EndpointId>>,
    pub(crate) invites: Mutex<HashMap<String, crate::pairing::PendingInvite>>,
    pub(crate) app_code_failures: std::sync::atomic::AtomicU32,
    /// Where requests of apps on other devices go (see `remote.rs`).
    pub(crate) remote: Mutex<Option<tokio::sync::mpsc::UnboundedSender<crate::remote::Request>>>,
    pub(crate) wake: Notify,
    /// Where file bytes live (see `files.rs`), and the signal that some are missing.
    pub(crate) files_dir: PathBuf,
    pub(crate) fetch: Notify,
    /// Files being downloaded right now (one download per file).
    pub(crate) fetching: Mutex<HashSet<String>>,
    /// What connected devices said they hold: document → (fingerprint, all
    /// files there). See `keep.rs`.
    pub(crate) confirmed: Mutex<HashMap<EndpointId, HashMap<String, (Vec<u8>, bool)>>>,
    pub(crate) release: Notify,
    pub(crate) stream_over: std::sync::atomic::AtomicU64,
    /// Hash of the owner key (see [`owner_key`]), if this device has one.
    owner: Option<String>,
    pub(crate) shutdown: watch::Sender<bool>,
    pub(crate) next_session: std::sync::atomic::AtomicU64,
}

/// A running UUOB node. Cheap to clone.
#[derive(Clone)]
pub struct Node(pub(crate) Arc<Inner>);

impl Node {
    /// Open (or create) the node stored in `cfg.data_dir` and start networking.
    ///
    /// A fresh device starts as a one-device network of its own; linking it
    /// to other devices is done with [`Node::create_invite`] / [`Node::join`].
    pub async fn open(cfg: Config) -> Result<Node> {
        std::fs::create_dir_all(&cfg.data_dir).map_err(anyhow::Error::from)?;
        let keystore = cfg
            .keystore
            .clone()
            .unwrap_or_else(|| Arc::new(FileKeyStore::new(cfg.data_dir.join("keys"))));
        let master = keystore.load_or_create("master")?;
        let device_secret = SecretKey::from_bytes(&keystore.load_or_create("device")?);
        let store = Store::open(&cfg.data_dir.join("uuob.db"), master)?;
        // Documents saved by an older version get their fingerprint and counts once.
        for id in store.without_meta()? {
            if let Some(bytes) = store.load_doc(&id)? {
                store.set_meta(&id, &doc::meta(&mut AutoCommit::load(&bytes)?))?;
            }
        }
        let files_dir = cfg.data_dir.join("files");
        crate::files::sweep(&store, &files_dir)?;

        let endpoint = bind_endpoint(&cfg.net, device_secret, vec![SYNC_ALPN.to_vec(), APP_ALPN.to_vec()]).await?;
        let me = endpoint.id();
        let (events, _) = broadcast::channel(256);
        let (doc_changes, _) = broadcast::channel(1024);
        let (shutdown, _) = watch::channel(false);

        let inner = Arc::new(Inner {
            me,
            device_name: Mutex::new(cfg.device_name.clone()),
            platform: cfg.platform.clone(),
            net_mode: cfg.net.clone(),
            store: Mutex::new(store),
            docs: Mutex::new(Docs::default()),
            endpoint: endpoint.clone(),
            router: Mutex::new(None),
            events,
            doc_changes,
            peers: Mutex::new(HashMap::new()),
            dialing: Mutex::new(HashSet::new()),
            invites: Mutex::new(HashMap::new()),
            app_code_failures: Default::default(),
            remote: Mutex::new(None),
            wake: Notify::new(),
            files_dir,
            fetch: Notify::new(),
            fetching: Mutex::new(HashSet::new()),
            confirmed: Mutex::new(HashMap::new()),
            release: Notify::new(),
            stream_over: std::sync::atomic::AtomicU64::new(32 << 20),
            owner: owner_key(&cfg.data_dir)?.map(|key| crypto::sha256_hex(key.as_bytes())),
            shutdown,
            next_session: std::sync::atomic::AtomicU64::new(1),
        });
        let node = Node(inner);

        node.ensure_network()?;
        node.forget_old_asks()?;

        let router = Router::builder(endpoint)
            .accept(SYNC_ALPN, crate::sync::SyncHandler(Arc::downgrade(&node.0)))
            .accept(APP_ALPN, crate::remote::AppHandler(Arc::downgrade(&node.0)))
            .spawn();
        *node.0.router.lock().unwrap() = Some(router);

        tokio::spawn(crate::sync::dial_loop(Arc::downgrade(&node.0)));
        tokio::spawn(crate::files::fetch_loop(Arc::downgrade(&node.0)));
        tokio::spawn(crate::keep::release_loop(Arc::downgrade(&node.0)));
        info!(id = %me.fmt_short(), "uuob node started");
        Ok(node)
    }

    pub(crate) fn from_weak(w: &Weak<Inner>) -> Option<Node> {
        w.upgrade().map(Node)
    }

    pub async fn shutdown(&self) {
        let _ = self.0.shutdown.send(true);
        let invites: Vec<_> = self.0.invites.lock().unwrap().drain().collect();
        for (_, inv) in invites {
            if let Some(r) = inv.rendezvous {
                let _ = r.shutdown().await;
            }
        }
        let router = self.0.router.lock().unwrap().take();
        if let Some(r) = router {
            let _ = r.shutdown().await;
        }
        self.0.endpoint.close().await;
    }

    pub fn id(&self) -> EndpointId {
        self.0.me
    }

    /// This device's current address (useful for tests and diagnostics).
    pub fn addr(&self) -> EndpointAddr {
        self.0.endpoint.addr()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.0.events.subscribe()
    }

    pub(crate) fn emit(&self, e: Event) {
        let _ = self.0.events.send(e);
    }

    // ------------------------------------------------------------------
    // Document plumbing
    // ------------------------------------------------------------------

    /// The document, loaded from disk if it is not in memory.
    fn loaded<'a>(&self, docs: &'a mut Docs, id: &str) -> Result<&'a mut AutoCommit> {
        if !docs.contains(id) {
            let (d, size) = match self.0.store.lock().unwrap().load_doc(id)? {
                Some(bytes) => (AutoCommit::load(&bytes)?, bytes.len() as u64),
                None => (AutoCommit::new(), 0),
            };
            docs.insert(id, d, size);
            docs.trim(id);
        }
        Ok(docs.get(id).expect("loaded"))
    }

    pub(crate) fn doc_exists(&self, id: &str) -> Result<bool> {
        if self.0.docs.lock().unwrap().contains(id) {
            return Ok(true);
        }
        self.0.store.lock().unwrap().doc_exists(id)
    }

    /// The fingerprint of a stored document's current state, without loading it.
    pub(crate) fn fingerprint(&self, id: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.0.store.lock().unwrap().meta(id)?.map(|m| m.heads))
    }

    /// How many documents are in memory right now (diagnostics and tests).
    pub fn docs_in_memory(&self) -> usize {
        self.0.docs.lock().unwrap().map.len()
    }

    pub(crate) fn read_doc<R>(&self, id: &str, f: impl FnOnce(&AutoCommit) -> R) -> Result<R> {
        let mut docs = self.0.docs.lock().unwrap();
        Ok(f(self.loaded(&mut docs, id)?))
    }

    /// Run `f` on a document; if it changed, persist it and tell peers and the UI.
    pub(crate) fn write_doc<R>(
        &self,
        id: &str,
        from: Option<EndpointId>,
        f: impl FnOnce(&mut AutoCommit) -> Result<R>,
    ) -> Result<R> {
        let mut files_missing = false;
        let (result, changed) = {
            let mut docs = self.0.docs.lock().unwrap();
            let d = self.loaded(&mut docs, id)?;
            let before = d.get_heads();
            let result = f(d)?;
            let changed = d.get_heads() != before;
            if changed {
                // Store only what is new. Once the changes add up, fold them
                // into one full save so loading stays quick.
                let meta = doc::meta(d);
                let change = d.save_after(&before);
                let store = self.0.store.lock().unwrap();
                let pending = store.append_doc(id, &change, &meta)?;
                if pending > FOLD_AFTER && pending * 3 > store.doc_size(id)? {
                    store.save_doc(id, &d.save(), &meta)?;
                }
                if id != NET_DOC {
                    files_missing = crate::files::index(&store, &self.0.files_dir, id, d)?;
                }
                drop(store);
                docs.grew(id, change.len() as u64);
            }
            (result, changed)
        };
        if changed {
            self.after_change(id, from);
        }
        if files_missing {
            self.0.fetch.notify_one();
        }
        Ok(result)
    }

    fn after_change(&self, id: &str, from: Option<EndpointId>) {
        let _ = self.0.doc_changes.send(DocChange { doc: id.to_string(), content: true });
        if id == NET_DOC {
            if let Err(e) = self.reconcile() {
                warn!("reconcile failed: {e}");
            }
            self.0.wake.notify_one();
            self.0.release.notify_one();
            self.emit(Event::NetworkChanged);
            self.emit(Event::AsksChanged);
            self.drop_non_members();
            if from.is_some() && !self.read_net(|d| net::is_active_member(d, &self.0.me.to_string())) {
                // Another device removed this one from the network.
                if let Err(e) = self.reset_network() {
                    warn!("reset after removal failed: {e}");
                }
                self.emit(Event::RemovedFromNetwork);
            }
        } else if let Some((container, _)) = net::parse_doc_id(id) {
            self.emit(Event::ContainerChanged { container: container.to_string() });
        }
    }

    /// Close sessions with devices that are no longer members (or no longer
    /// a linked person's device), after a short grace period so they first
    /// learn they were removed.
    pub(crate) fn drop_non_members(&self) {
        let gone: Vec<EndpointId> = {
            let peers = self.0.peers.lock().unwrap();
            peers.keys().copied().filter(|p| !self.is_active_peer(p)).collect()
        };
        if gone.is_empty() {
            return;
        }
        let node = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(2)).await;
            for id in gone {
                let entry = node.0.peers.lock().unwrap().remove(&id);
                if let Some(p) = entry {
                    p.conn.close(0u32.into(), b"removed");
                    node.emit(Event::PeerDisconnected { id: id.to_string() });
                }
            }
        });
    }

    pub(crate) fn read_net<R>(&self, f: impl FnOnce(&AutoCommit) -> R) -> R {
        self.read_doc(NET_DOC, f).expect("network document is readable")
    }

    pub(crate) fn write_net<R>(&self, f: impl FnOnce(&mut AutoCommit) -> Result<R>) -> Result<R> {
        self.write_doc(NET_DOC, None, f)
    }

    /// All document ids held locally (cache + store).
    pub(crate) fn local_doc_ids(&self) -> Result<Vec<String>> {
        let mut ids: HashSet<String> = self.0.store.lock().unwrap().doc_ids()?.into_iter().collect();
        ids.extend(self.0.docs.lock().unwrap().map.keys().cloned());
        let mut v: Vec<_> = ids.into_iter().collect();
        v.sort();
        Ok(v)
    }

    /// Should a document with this id be stored and synced?
    pub(crate) fn wants_doc(&self, id: &str) -> bool {
        if id == NET_DOC {
            return true;
        }
        let Some((container, generation)) = net::parse_doc_id(id) else { return false };
        match self.read_net(|d| net::container(d, container)) {
            // Not registered yet: the network doc may simply not have arrived.
            None => true,
            // A container this device was asked not to keep stays (and
            // keeps syncing) until it has been let go of.
            Some(c) => {
                !c.deleted
                    && c.generation == generation
                    && (self.wish(container) != Keep::None || self.doc_exists(id).unwrap_or(false))
            }
        }
    }

    /// Drop local copies of deleted (or superseded) containers.
    fn reconcile(&self) -> Result<()> {
        for id in self.local_doc_ids()? {
            if !self.wants_doc(&id) {
                info!(doc = %id, "removing deleted container data");
                self.forget_doc(&id)?;
            }
        }
        Ok(())
    }

    /// Remove this device's copy of a document and of its files.
    pub(crate) fn forget_doc(&self, id: &str) -> Result<()> {
        self.0.docs.lock().unwrap().remove(id);
        let store = self.0.store.lock().unwrap();
        let files = store.files_of(id)?;
        store.delete_doc(id)?;
        // Its files go with it, unless another container lists them too.
        for file in files.keys() {
            if !store.file_known(file)? {
                let _ = std::fs::remove_file(self.0.files_dir.join(file));
                let _ = std::fs::remove_file(self.0.files_dir.join(format!("{file}.part")));
            }
        }
        Ok(())
    }

    fn ensure_network(&self) -> Result<()> {
        let exists = self.read_net(|d| net::network(d).is_some());
        if exists {
            // The stored name wins (the user may have renamed this device);
            // the configured name is only the default for a new device.
            let me = self.0.me.to_string();
            if let Some(m) = self.read_net(|d| net::members(d).get(&me).cloned()) {
                *self.0.device_name.lock().unwrap() = m.name.clone();
                if m.platform != self.0.platform {
                    let platform = self.0.platform.clone();
                    self.write_net(|d| net::set_member(d, &me, &Member { platform, ..m }))?;
                }
            }
            return Ok(());
        }
        self.write_net(|d| self.init_solo_network(d, None))
    }

    fn init_solo_network(&self, d: &mut AutoCommit, owner: Option<String>) -> Result<()> {
        let name = self.0.device_name.lock().unwrap().clone();
        net::set_network(
            d,
            &NetworkInfo { id: crypto::random_id(16), created_at: now_ms(), name: owner, copies: None },
        )?;
        net::set_member(
            d,
            &self.0.me.to_string(),
            &Member { name, platform: self.0.platform.clone(), added_at: now_ms(), removed: false, keeper: false, cache: None },
        )
    }

    /// Leave the current network and become a one-device network again.
    /// Apps, containers, data, grants and AI clients stay on this device.
    /// Links with other people belong to the network that was left, so they
    /// (and what those people shared) do not come along.
    pub fn reset_network(&self) -> Result<()> {
        let (owner, apps, containers, grants, clients) = self.read_net(|d| {
            (net::network(d).and_then(|n| n.name), net::apps(d), net::containers(d), net::grants(d), net::clients(d))
        });
        let mut fresh = AutoCommit::new();
        self.init_solo_network(&mut fresh, owner)?;
        for (id, a) in &apps {
            net::set_app(&mut fresh, id, a)?;
        }
        for (id, c) in containers.iter().filter(|(_, c)| c.from.is_none()) {
            net::set_container(&mut fresh, id, c)?;
        }
        for (client, m) in &grants {
            for (container, access) in m {
                net::set_grant(&mut fresh, client, container, Some(*access))?;
            }
        }
        for (id, c) in &clients {
            net::set_client(&mut fresh, id, c)?;
        }
        self.replace_net_doc(fresh)?;
        // Disconnect from everyone; they are no longer members of our network.
        let peers: Vec<_> = self.0.peers.lock().unwrap().drain().collect();
        for (_, p) in peers {
            p.conn.close(0u32.into(), b"left network");
        }
        Ok(())
    }

    pub(crate) fn replace_net_doc(&self, mut d: AutoCommit) -> Result<()> {
        let meta = doc::meta(&mut d);
        let bytes = d.save();
        self.0.store.lock().unwrap().save_doc(NET_DOC, &bytes, &meta)?;
        self.0.docs.lock().unwrap().insert(NET_DOC, d, bytes.len() as u64);
        self.reconcile()?;
        self.0.wake.notify_one();
        self.emit(Event::NetworkChanged);
        Ok(())
    }

    // ------------------------------------------------------------------
    // Status & devices
    // ------------------------------------------------------------------

    pub fn status(&self) -> Status {
        let (network, members) = self.read_net(|d| (net::network(d).expect("network"), net::members(d)));
        let online = self.0.peers.lock().unwrap().keys().filter(|p| members.contains_key(&p.to_string())).count();
        Status {
            device_id: self.0.me.to_string(),
            device_name: self.0.device_name.lock().unwrap().clone(),
            platform: self.0.platform.clone(),
            network,
            devices_total: members.values().filter(|m| !m.removed).count(),
            devices_online: online + 1,
            documents_in_memory: self.docs_in_memory(),
        }
    }

    pub fn devices(&self) -> Vec<Device> {
        let members = self.read_net(net::members);
        let peers = self.0.peers.lock().unwrap();
        let me = self.0.me.to_string();
        let mut out: Vec<Device> = members
            .into_iter()
            .filter(|(_, m)| !m.removed)
            .map(|(id, m)| {
                let is_self = id == me;
                let online =
                    is_self || id.parse::<EndpointId>().map(|eid| peers.contains_key(&eid)).unwrap_or(false);
                let cache_bytes = m.cache.unwrap_or(crate::keep::DEFAULT_CACHE);
                Device { id, name: m.name, platform: m.platform, added_at: m.added_at, is_self, online, keeper: m.keeper, cache_bytes }
            })
            .collect();
        out.sort_by_key(|d| (!d.is_self, d.added_at));
        out
    }

    pub fn set_device_name(&self, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() || name.len() > 64 {
            return Err(Error::Invalid("device name must be 1–64 characters".into()));
        }
        *self.0.device_name.lock().unwrap() = name.to_string();
        let me = self.0.me.to_string();
        self.write_net(|d| {
            let m = net::members(d).get(&me).cloned().ok_or_else(|| Error::NotFound("this device".into()))?;
            net::set_member(d, &me, &Member { name: name.to_string(), ..m })
        })
    }

    /// Remove another device from the network. It stops receiving updates
    /// immediately; data it already synced stays on it.
    pub fn remove_device(&self, id: &str) -> Result<()> {
        if id == self.0.me.to_string() {
            return Err(Error::Invalid("use reset_network to leave the network".into()));
        }
        self.write_net(|d| {
            let mut m = net::members(d).get(id).cloned().ok_or_else(|| Error::NotFound(id.into()))?;
            m.removed = true;
            net::set_member(d, id, &m)
        })?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Apps & containers
    // ------------------------------------------------------------------

    pub fn apps(&self) -> BTreeMap<String, AppInfo> {
        self.read_net(net::apps)
    }

    /// What is coming up in the user's apps (events, tasks that are due), for
    /// the device to show (a widget) and ring (alerts): the records of
    /// collection `upcoming` in each installed app's own container (which is
    /// never shared, so nobody else gets them), plus `app` and `app_name`.
    ///
    /// An item is `{ date: "YYYY-MM-DD", time?: "HH:MM", end_date?, end?, tz?,
    /// title, body?, alert?, color? }`: it starts at `date` `time` (all day without a
    /// time) on the wall clock of `tz`, or of the device without one, and is
    /// over after `end_date` `end` (never without them, like a task that is
    /// due). `alert` is how many minutes before the start to ring, if at all;
    /// `color` is one of the calendar colours ("blue", "red", "orange",
    /// "yellow", "green", "teal", "purple", "pink"). Other fields are passed on
    /// as they are. Each app keeps its list
    /// current (`uuob.upcoming` in the SDK).
    pub fn upcoming(&self) -> Vec<Json> {
        let apps = self.read_net(net::apps);
        let mut out = vec![];
        for (id, app) in apps.into_iter().filter(|(_, a)| a.installed) {
            // Not on this device (or not arrived yet): nothing to show from it.
            let Ok(records) = self.list(&Client::App(id.clone()), &id, "upcoming") else { continue };
            for mut r in records {
                let text = |k: &str| r.get(k).is_some_and(Json::is_string);
                if !(text("date") && text("title")) {
                    continue;
                }
                r["app"] = id.clone().into();
                r["app_name"] = app.name.clone().into();
                out.push(r);
            }
        }
        let when = |r: &Json| format!("{} {}", r["date"].as_str().unwrap_or(""), r["time"].as_str().unwrap_or(""));
        out.sort_by_cached_key(when);
        out
    }

    /// Install an app. It gets its own container (created if missing, or
    /// re-created fresh if its data was deleted earlier).
    pub fn install_app(&self, m: &AppManifest) -> Result<()> {
        validate_ident(&m.id)?;
        self.write_net(|d| {
            let current = net::apps(d).get(&m.id).cloned();
            let info = AppInfo {
                name: m.name.clone(),
                installed: true,
            };
            if current.as_ref() != Some(&info) {
                net::set_app(d, &m.id, &info)?;
            }
            match net::containers(d).get(&m.id).cloned() {
                None => net::set_container(
                    d,
                    &m.id,
                    &ContainerInfo {
                        owner_app: m.id.clone(),
                        name: m.name.clone(),
                        created_at: now_ms(),
                        generation: 0,
                        deleted: false,
                        from: None,
                        access: None,
                        inbox: None,
                    },
                )?,
                Some(c) if c.deleted => net::set_container(
                    d,
                    &m.id,
                    &ContainerInfo { deleted: false, created_at: now_ms(), generation: c.generation, ..c },
                )?,
                Some(_) => {}
            }
            Ok(())
        })
    }

    /// Uninstall an app. With `delete_data` its containers are deleted on
    /// every device; otherwise the data stays (visible on the Apps screen).
    pub fn uninstall_app(&self, id: &str, delete_data: bool) -> Result<()> {
        self.write_net(|d| {
            let mut app = net::apps(d).get(id).cloned().ok_or_else(|| Error::NotFound(format!("app {id}")))?;
            app.installed = false;
            net::set_app(d, id, &app)?;
            for (cid, _) in net::clients(d).into_iter().filter(|(_, c)| c.app.as_deref() == Some(id)) {
                net::remove_client(d, &cid)?;
            }
            net::remove_grants_for_client(d, &format!("app:{id}"))?;
            if delete_data {
                for (cid, c) in net::containers(d).into_iter().filter(|(_, c)| c.owner_app == id) {
                    Self::delete_container_in(d, &cid, c)?;
                }
                net::remove_grants_for_app(d, id)?;
            }
            Ok(())
        })
    }

    /// Give an app one more container, e.g. for a list or a calendar.
    /// Separate containers are what can be shared with other people.
    ///
    /// `key` lets an app that moves existing data into containers get the
    /// same container on every device: it must be a random id the app
    /// already has (16+ characters), and asking again returns the same one.
    pub fn create_container(&self, app: &str, name: &str, key: Option<&str>) -> Result<String> {
        let name = valid_name(name)?;
        let key = match key {
            Some(k) if k.len() >= 16 && !k.contains('.') && validate_ident(k).is_ok() => k.to_string(),
            Some(_) => return Err(Error::Invalid("a container key must be a random id of 16+ characters".into())),
            None => crypto::random_id(10),
        };
        let id = format!("{app}.{key}");
        validate_ident(&id)?;
        self.write_net(|d| {
            if !net::apps(d).get(app).is_some_and(|a| a.installed) {
                return Err(Error::NotFound(format!("app {app}")));
            }
            match net::container(d, &id) {
                None => net::set_container(
                    d,
                    &id,
                    &ContainerInfo {
                        owner_app: app.into(),
                        name,
                        created_at: now_ms(),
                        generation: 0,
                        deleted: false,
                        from: None,
                        access: None,
                        inbox: None,
                    },
                ),
                Some(c) if !c.deleted && c.from.is_none() => Ok(()),
                Some(_) => Err(Error::Invalid(format!("{id} was deleted"))),
            }
        })?;
        Ok(id)
    }

    /// Rename one of your own containers (people it is shared with see the new name).
    pub fn rename_container(&self, id: &str, name: &str) -> Result<()> {
        let name = valid_name(name)?;
        self.write_net(|d| match net::container(d, id) {
            Some(c) if !c.deleted && c.from.is_none() => net::set_container(d, id, &ContainerInfo { name, ..c }),
            Some(c) if !c.deleted => Err(Error::PermissionDenied("only its owner can rename this".into())),
            _ => Err(Error::NotFound(format!("container {id}"))),
        })
    }

    /// Delete a container and its data on every device of the network. For
    /// a container someone shared with you this means leaving it: your copy
    /// goes away and the owner stops sharing it with you.
    pub fn delete_container(&self, id: &str) -> Result<()> {
        self.write_net(|d| {
            let c = net::container(d, id).ok_or_else(|| Error::NotFound(format!("container {id}")))?;
            Self::delete_container_in(d, id, c)
        })
    }

    fn delete_container_in(d: &mut AutoCommit, id: &str, c: ContainerInfo) -> Result<()> {
        if c.from.is_some() {
            return match c.inbox {
                Some(Inbox::Offered | Inbox::Accepted) => {
                    net::set_container(d, id, &ContainerInfo { deleted: true, inbox: Some(Inbox::Declined), ..c })
                }
                _ => Ok(()),
            };
        }
        if c.deleted {
            return Ok(());
        }
        net::set_container(d, id, &ContainerInfo { deleted: true, generation: c.generation + 1, ..c })
    }

    /// The container, if `client` owns it (apps own the containers of their
    /// own app; the UUOB shell acts for the user).
    pub(crate) fn owned(&self, client: &Client, id: &str) -> Result<ContainerInfo> {
        let c = self
            .read_net(|d| net::container(d, id))
            .filter(|c| !c.deleted)
            .ok_or_else(|| Error::NotFound(format!("container {id}")))?;
        match client {
            Client::Shell => Ok(c),
            Client::App(a) if *a == c.owner_app => Ok(c),
            _ => Err(Error::PermissionDenied(format!("{} does not own {id}", client.key()))),
        }
    }

    pub(crate) fn active_doc_id(&self, container: &str) -> Result<String> {
        let c = self.read_net(|d| net::container(d, container));
        match c {
            Some(c) if !c.deleted => Ok(net::doc_id(container, c.generation)),
            _ => Err(Error::NotFound(format!("container {container}"))),
        }
    }

    /// Containers visible to `client`, with usage and who can access them.
    pub fn containers(&self, client: &Client) -> Result<Vec<ContainerSummary>> {
        let (containers, apps, grants, clients, people, shares) = self.read_net(|d| {
            (net::containers(d), net::apps(d), net::grants(d), net::clients(d), net::people(d), net::shares(d))
        });
        let person_name = |id: &String| people.get(id).map(|p| p.name.clone()).unwrap_or_default();
        let mut out = Vec::new();
        for (id, c) in containers.into_iter().filter(|(_, c)| !c.deleted) {
            if self.check(client, &id, Access::Read).is_err() {
                continue;
            }
            let doc_id = net::doc_id(&id, c.generation);
            // Counts come from the store, so listing loads no documents.
            let meta = self.0.store.lock().unwrap().meta(&doc_id)?;
            let mut collections: BTreeMap<String, usize> =
                meta.and_then(|m| serde_json::from_str(&m.stats).ok()).unwrap_or_default();
            collections.remove("~file");
            // (Each lock is taken and released on its own line: a guard
            // inside the struct below would be held while the other fields
            // take locks too.)
            let (files, file_bytes, files_here) = self.0.store.lock().unwrap().file_totals(&doc_id)?;
            let size_bytes = self.0.store.lock().unwrap().doc_size(&doc_id)?;
            let keep = self.wish(&id);
            // Not here only if it was let go of (or never fetched) on request.
            let here = keep != Keep::None || self.doc_exists(&doc_id)?;
            let copies = self.confirmations(&doc_id);
            let can_write = self.check(client, &id, Access::Write).is_ok();
            let mut access = vec![];
            if let Some(app) = apps.get(&c.owner_app).filter(|a| a.installed) {
                access.push(AccessEntry {
                    client: format!("app:{}", c.owner_app),
                    client_name: app.name.clone(),
                    access: Access::Write,
                });
            }
            for (ck, m) in &grants {
                if let Some(a) = m.get(&c.owner_app) {
                    let name = match ck.split_once(':') {
                        Some(("app", a)) => apps.get(a).map(|x| x.name.clone()),
                        Some((_, cid)) => clients.get(cid).map(|x| x.name.clone()),
                        None => None,
                    }
                    .unwrap_or_else(|| ck.clone());
                    access.push(AccessEntry { client: ck.clone(), client_name: name, access: *a });
                }
            }
            out.push(ContainerSummary {
                app_installed: apps.get(&c.owner_app).is_some_and(|a| a.installed),
                size_bytes,
                records: collections.values().sum(),
                collections,
                files,
                file_bytes,
                files_here,
                keep,
                here,
                copies,
                access,
                can_write,
                from: c.from.as_ref().map(|p| PersonRef { id: p.clone(), name: person_name(p) }),
                shared_with: shares
                    .get(&id)
                    .into_iter()
                    .flatten()
                    .filter(|(p, _)| people.get(*p).is_some_and(|x| !x.removed))
                    .filter_map(|(p, a)| Some(SharedWith { id: p.clone(), name: person_name(p), access: (*a)? }))
                    .collect(),
                id,
                name: c.name,
                owner_app: c.owner_app,
            });
        }
        Ok(out)
    }

    /// Everything in a container, as JSON (`{"<collection>/<id>": record}`).
    pub fn export_container(&self, client: &Client, container: &str) -> Result<Json> {
        self.authorize(client, container, Access::Read)?;
        let doc_id = self.local_doc_id(container)?;
        self.read_doc(&doc_id, |d| doc::to_json(d, &ROOT))
    }

    // ------------------------------------------------------------------
    // Permissions
    // ------------------------------------------------------------------

    pub(crate) fn check(&self, client: &Client, container: &str, wanted: Access) -> Result<()> {
        let has = self.read_net(|d| {
            let Some(c) = net::container(d, container).filter(|c| !c.deleted) else {
                // The shell acts for the user; a missing container is reported as such.
                return (*client == Client::Shell).then_some(Access::Write);
            };
            let own = match client {
                Client::Shell => Some(Access::Write),
                // An app owns every container of its own app.
                Client::App(a) if *a == c.owner_app => {
                    net::apps(d).get(a).is_some_and(|x| x.installed).then_some(Access::Write)
                }
                // Anything else needs a grant, which covers all of that app's data.
                _ => net::grant(d, &client.key(), &c.owner_app),
            }?;
            // What someone else shared with us may be view-only, for everyone here.
            Some(own.min(c.access.unwrap_or(Access::Write)))
        });
        match has {
            Some(a) if a.allows(wanted) => Ok(()),
            _ => Err(Error::PermissionDenied(format!(
                "{} has no {} access to {container}",
                client.key(),
                if wanted == Access::Write { "write" } else { "read" }
            ))),
        }
    }

    fn authorize(&self, client: &Client, container: &str, wanted: Access) -> Result<()> {
        self.check(client, container, wanted)
    }

    /// Whether `client` may access `container` with `wanted`.
    pub fn allowed(&self, client: &Client, container: &str, wanted: Access) -> bool {
        self.check(client, container, wanted).is_ok()
    }

    /// Grant (or with `None`, revoke) `client_key` access to all data of `app`.
    pub fn set_grant(&self, client_key: &str, app: &str, access: Option<Access>) -> Result<()> {
        let (kind, id) =
            client_key.split_once(':').ok_or_else(|| Error::Invalid(format!("bad client {client_key}")))?;
        let known = self.read_net(|d| match kind {
            "app" => net::apps(d).contains_key(id),
            "ai" => net::clients(d).contains_key(id),
            _ => false,
        });
        if !known {
            return Err(Error::NotFound(format!("client {client_key}")));
        }
        if kind == "app" && id == app {
            return Err(Error::Invalid("an app always has full access to its own data".into()));
        }
        if !self.read_net(|d| net::apps(d).contains_key(app)) {
            return Err(Error::NotFound(format!("app {app}")));
        }
        self.write_net(|d| net::set_grant(d, client_key, app, access))
    }

    pub fn grants(&self) -> BTreeMap<String, BTreeMap<String, Access>> {
        self.read_net(net::grants)
    }

    // ------------------------------------------------------------------
    // AI / API clients
    // ------------------------------------------------------------------

    /// Create an AI/API client. Returns `(id, token)`. The token is shown
    /// once; only its hash is stored. New clients have no access at all.
    pub fn create_client(&self, name: &str) -> Result<(String, String)> {
        let name = name.trim();
        if name.is_empty() || name.len() > 64 {
            return Err(Error::Invalid("name must be 1–64 characters".into()));
        }
        let id = crypto::random_id(8);
        let token = format!("uuob_{}", crypto::random_id(32));
        let info = ClientInfo {
            name: name.into(),
            token_sha256: crypto::sha256_hex(token.as_bytes()),
            created_at: now_ms(),
            app: None,
            endpoint: None,
            device: None,
            // An AI uses the MCP server of the device it was set up on.
            on: Some(self.0.me.to_string()),
        };
        self.write_net(|d| net::set_client(d, &id, &info))?;
        Ok((id, token))
    }

    pub fn clients(&self) -> BTreeMap<String, ClientInfo> {
        self.read_net(net::clients)
    }

    /// Cut off one AI client, or one copy of an app. Without its last copy
    /// an app is disconnected (its data stays).
    pub fn delete_client(&self, id: &str) -> Result<()> {
        let last_of = self.read_net(|d| {
            let clients = net::clients(d);
            let app = clients.get(id)?.app.clone()?;
            (clients.values().filter(|c| c.app.as_ref() == Some(&app)).count() == 1).then_some(app)
        });
        match last_of {
            Some(app) => self.uninstall_app(&app, false),
            None => self.write_net(|d| net::remove_client(d, id)),
        }
    }

    /// Resolve a bearer token to a client.
    pub fn authenticate(&self, token: &str) -> Option<Client> {
        self.authenticate_from(token, None)
    }

    /// Resolve a bearer token. `linked`: the request came over the tunnel
    /// from the app with this endpoint id (see `remote.rs`); then only that
    /// app's own token counts, never the owner key or anyone else's token.
    pub fn authenticate_from(&self, token: &str, linked: Option<&str>) -> Option<Client> {
        let hash = crypto::sha256_hex(token.as_bytes());
        if linked.is_none() && self.0.owner.as_deref() == Some(hash.as_str()) {
            return Some(Client::Shell);
        }
        let (id, c) = self
            .read_net(net::clients)
            .into_iter()
            .find(|(_, c)| c.token_sha256 == hash && (linked.is_none() || c.endpoint.as_deref() == linked))?;
        // Made before UUOB recorded where: it is here, since its token is used here.
        // (Read again inside the write, so a connection removed meanwhile stays removed.)
        if c.on.is_none() && c.endpoint.is_none() {
            let me = self.0.me.to_string();
            let _ = self.write_net(|d| match net::clients(d).remove(&id) {
                Some(now) if now.on.is_none() => net::set_client(d, &id, &ClientInfo { on: Some(me), ..now }),
                _ => Ok(()),
            });
        }
        Some(match c.app {
            Some(app) => Client::App(app),
            None => Client::Ai(id),
        })
    }

    // ------------------------------------------------------------------
    // Records
    // ------------------------------------------------------------------

    /// Records are flat root keys `<collection>/<id>`.
    fn record_obj_json(d: &AutoCommit, collection: &str, id: &str) -> Option<Json> {
        let rec = doc::get_map(d, &ROOT, &format!("{collection}/{id}"))?;
        let mut json = doc::to_json(d, &rec);
        if let Json::Object(m) = &mut json {
            m.insert("id".into(), Json::String(id.into()));
        }
        Some(json)
    }

    fn prepare(value: Json) -> Result<Map<String, Json>> {
        match value {
            Json::Object(mut m) => {
                m.remove("id");
                Ok(m)
            }
            _ => Err(Error::Invalid("a record must be a JSON object".into())),
        }
    }

    /// Create or replace a record (only changed fields are written, so
    /// concurrent edits of different fields on two devices both survive).
    pub fn put(&self, client: &Client, container: &str, collection: &str, id: Option<&str>, value: Json) -> Result<Json> {
        crate::files::check_collection(collection)?;
        self.authorize(client, container, Access::Write)?;
        validate_key(collection)?;
        let id = match id {
            Some(i) => {
                validate_key(i)?;
                i.to_string()
            }
            None => crypto::random_id(10),
        };
        let fields = Self::prepare(value)?;
        let doc_id = self.local_doc_id(container)?;
        self.write_doc(&doc_id, None, |d| {
            doc::set_record(d, &ROOT, &format!("{collection}/{id}"), &fields)?;
            Ok(Self::record_obj_json(d, collection, &id).expect("just written"))
        })
    }

    pub fn get(&self, client: &Client, container: &str, collection: &str, id: &str) -> Result<Option<Json>> {
        crate::files::check_collection(collection)?;
        self.authorize(client, container, Access::Read)?;
        let doc_id = self.local_doc_id(container)?;
        self.read_doc(&doc_id, |d| Self::record_obj_json(d, collection, id))
    }

    pub fn delete(&self, client: &Client, container: &str, collection: &str, id: &str) -> Result<bool> {
        crate::files::check_collection(collection)?;
        self.authorize(client, container, Access::Write)?;
        let doc_id = self.local_doc_id(container)?;
        self.write_doc(&doc_id, None, |d| {
            use automerge::transaction::Transactable;
            let key = format!("{collection}/{id}");
            if doc::get_map(d, &ROOT, &key).is_none() {
                return Ok(false);
            }
            d.delete(&ROOT, key.as_str())?;
            Ok(true)
        })
    }

    /// All records of a collection.
    pub fn list(&self, client: &Client, container: &str, collection: &str) -> Result<Vec<Json>> {
        crate::files::check_collection(collection)?;
        self.authorize(client, container, Access::Read)?;
        let doc_id = self.local_doc_id(container)?;
        self.read_doc(&doc_id, |d| {
            let prefix = format!("{collection}/");
            d.keys(&ROOT)
                .filter_map(|k| Self::record_obj_json(d, collection, k.strip_prefix(&prefix)?))
                .collect()
        })
    }

    // ------------------------------------------------------------------
    // Peers (used by sync.rs)
    // ------------------------------------------------------------------

    /// Register a sync session with `remote`. Exactly one session per peer is
    /// kept; when both sides dial at once, the one started by the device with
    /// the smaller id wins on both ends.
    pub(crate) fn register_peer(&self, remote: EndpointId, conn: &Connection, initiated_by_me: bool) -> Option<u64> {
        let session = self.0.next_session.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let preferred_is_mine = self.0.me < remote;
        let mut peers = self.0.peers.lock().unwrap();
        if let Some(existing) = peers.get(&remote) {
            let existing_preferred = existing.initiated_by_me == preferred_is_mine;
            let new_preferred = initiated_by_me == preferred_is_mine;
            // The tie-break is only for two devices dialing each other at
            // the same moment. A connection that arrives later means the
            // other device lost the old one (it was restarted) while this
            // one has not noticed yet: the new connection replaces it.
            let same_moment = existing.since.elapsed() < Duration::from_secs(5);
            if same_moment && existing_preferred && !new_preferred && existing.conn.close_reason().is_none() {
                return None;
            }
            existing.conn.close(0u32.into(), b"superseded");
        }
        peers.insert(remote, PeerEntry { session, initiated_by_me, conn: conn.clone(), since: std::time::Instant::now() });
        drop(peers);
        // It may have files this device is missing.
        self.0.fetch.notify_one();
        self.emit(Event::PeerConnected { id: remote.to_string() });
        Some(session)
    }

    pub(crate) fn unregister_peer(&self, remote: EndpointId, session: u64) {
        let mut peers = self.0.peers.lock().unwrap();
        if peers.get(&remote).is_some_and(|p| p.session == session) {
            peers.remove(&remote);
            drop(peers);
            self.0.confirmed.lock().unwrap().remove(&remote);
            self.emit(Event::PeerDisconnected { id: remote.to_string() });
        }
    }

    pub fn online_peers(&self) -> Vec<String> {
        self.0.peers.lock().unwrap().keys().map(|k| k.to_string()).collect()
    }

    /// Wait until `remote` has an active sync session (tests, UI waits).
    pub async fn wait_for_peer(&self, remote: EndpointId, timeout: Duration) -> bool {
        let mut rx = self.subscribe();
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if self.0.peers.lock().unwrap().contains_key(&remote) {
                return true;
            }
            match tokio::time::timeout_at(deadline, rx.recv()).await {
                Err(_) => return false,
                Ok(_) => continue,
            }
        }
    }

}

pub(crate) fn validate_ident(id: &str) -> Result<()> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.')
        && !id.starts_with('_');
    if ok { Ok(()) } else { Err(Error::Invalid(format!("invalid app id {id:?} (use a-z, 0-9, '-', '.')"))) }
}

/// A name shown to people: trimmed, 1–64 characters.
pub(crate) fn valid_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(Error::Invalid("a name must be 1–64 characters".into()));
    }
    Ok(name.to_string())
}

/// The owner key: whoever has it acts as the user (like the UUOB window
/// does). It lets the `uuob` command control the UUOB running on the same
/// computer. It sits in the data folder next to the device's own keys,
/// readable only by that user, so it gives nothing to anyone who could not
/// already read everything. Phones have no command line and get none.
fn owner_key(data_dir: &std::path::Path) -> Result<Option<String>> {
    if cfg!(any(target_os = "android", target_os = "ios")) {
        return Ok(None);
    }
    let path = data_dir.join("owner.key");
    if let Ok(key) = std::fs::read_to_string(&path) {
        return Ok(Some(key.trim().to_string()));
    }
    let key = format!("uuob_owner_{}", crypto::random_id(32));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    std::io::Write::write_all(&mut options.open(&path)?, key.as_bytes())?;
    Ok(Some(key))
}

fn validate_key(k: &str) -> Result<()> {
    if k.is_empty() || k.len() > 128 || k.contains('/') {
        return Err(Error::Invalid(format!("invalid name {k:?}")));
    }
    Ok(())
}

/// Bind an iroh endpoint for the given mode.
pub(crate) async fn bind_endpoint(
    mode: &NetMode,
    secret: SecretKey,
    alpns: Vec<Vec<u8>>,
) -> Result<Endpoint> {
    bind_endpoint_with(mode, secret, alpns, None).await
}

/// `idle`: give up on a connection whose other end has been silent this
/// long (the default is half a minute).
pub(crate) async fn bind_endpoint_with(
    mode: &NetMode,
    secret: SecretKey,
    alpns: Vec<Vec<u8>>,
    idle: Option<Duration>,
) -> Result<Endpoint> {
    let transport = |b: iroh::endpoint::Builder| match idle {
        Some(idle) => {
            let timeout = iroh::endpoint::VarInt::from_u32(idle.as_millis() as u32);
            let cfg = iroh::endpoint::QuicTransportConfig::builder()
                .max_idle_timeout(Some(timeout.into()))
                .keep_alive_interval(idle / 3)
                .build();
            b.transport_config(cfg)
        }
        None => b,
    };
    let ep = match mode {
        NetMode::Internet => {
            let ep = transport(Endpoint::builder(presets::N0))
                .secret_key(secret)
                .alpns(alpns)
                .bind()
                .await
                .map_err(|e| anyhow::anyhow!("bind endpoint: {e}"))?;
            ep
        }
        NetMode::Local(shared) => {
            let ep = transport(Endpoint::builder(presets::Minimal))
                .relay_mode(RelayMode::Disabled)
                .secret_key(secret)
                .alpns(alpns)
                .bind_addr("127.0.0.1:0")
                .map_err(|e| anyhow::anyhow!("bind addr: {e}"))?
                .bind()
                .await
                .map_err(|e| anyhow::anyhow!("bind endpoint: {e}"))?;
            if let Ok(services) = ep.address_lookup() {
                services.add(shared.clone());
            }
            // Publish our direct address to the shared lookup.
            for _ in 0..100 {
                let addr = ep.addr();
                if addr.ip_addrs().next().is_some() {
                    shared.add_endpoint_info(addr);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            ep
        }
    };
    Ok(ep)
}
