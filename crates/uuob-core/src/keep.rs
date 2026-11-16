//! What each device stores.
//!
//! By default every device keeps everything. An app can say that the device
//! it runs on keeps only a container's records (its files then come when
//! they are opened, and are cached) or nothing of it. A device the user
//! marked as a *storage device* keeps everything regardless.
//!
//! These wishes live in the network document (`keep/<device>/<container>`),
//! so every device knows who means to hold what. Acting on a wish to keep
//! less waits until that is safe: as many storage devices as the user chose
//! (`copies`) have told this device, over a live connection, that they hold
//! the same document and every file it lists. Without storage devices
//! nothing is ever let go.

use std::sync::Weak;
use std::time::Duration;

use iroh::EndpointId;
use tracing::{info, warn};

use crate::error::{Error, Result};
use crate::net::{self, Keep, NET_DOC};
use crate::node::{Client, DocChange, Event, Inner, Node};

/// How many bytes of files a device caches for containers of which it keeps
/// only the list, unless the user chose otherwise.
pub(crate) const DEFAULT_CACHE: u64 = 1 << 30;

impl Node {
    fn is_keeper(&self, device: &str) -> bool {
        self.read_net(|d| net::member(d, device).is_some_and(|m| m.keeper && !m.removed))
    }

    /// Is this device a storage device?
    pub(crate) fn keeps_everything(&self) -> bool {
        self.is_keeper(&self.0.me.to_string())
    }

    /// What `device` is asked to keep of a container.
    fn wish_of(&self, device: &str, container: &str) -> Keep {
        self.read_net(|d| match net::member(d, device) {
            Some(m) if m.keeper => Keep::All,
            _ => net::keep(d, device, container),
        })
    }

    /// What this device is asked to keep of a container.
    pub(crate) fn wish(&self, container: &str) -> Keep {
        self.wish_of(&self.0.me.to_string(), container)
    }

    /// Does `peer` (a device of this network) mean to hold `doc`?
    pub(crate) fn peer_keeps(&self, peer: &EndpointId, doc: &str) -> bool {
        net::parse_doc_id(doc).is_none_or(|(container, _)| self.wish_of(&peer.to_string(), container) != Keep::None)
    }

    /// The document of a container, for reading or writing its data here.
    pub(crate) fn local_doc_id(&self, container: &str) -> Result<String> {
        let doc = self.active_doc_id(container)?;
        if self.wish(container) == Keep::None && !self.doc_exists(&doc)? {
            return Err(Error::Unavailable("it is not kept on this device".into()));
        }
        Ok(doc)
    }

    /// An app says how much of one of its containers this device keeps.
    pub fn set_keep(&self, client: &Client, container: &str, keep: Keep) -> Result<()> {
        self.owned(client, container)?;
        let me = self.0.me.to_string();
        self.write_net(|d| net::set_keep(d, &me, container, keep))?;
        self.0.release.notify_one();
        Ok(())
    }

    /// Make a device a storage device (it keeps everything), or stop that.
    pub fn set_keeper(&self, device: &str, on: bool) -> Result<()> {
        self.write_net(|d| {
            let m = net::member(d, device).filter(|m| !m.removed).ok_or_else(|| Error::NotFound(device.into()))?;
            net::set_member(d, device, &net::Member { keeper: on, ..m })
        })
    }

    /// How many storage devices must hold something before another device
    /// may let go of it.
    pub fn set_copies(&self, copies: u8) -> Result<()> {
        if !(1..=5).contains(&copies) {
            return Err(Error::Invalid("copies must be between 1 and 5".into()));
        }
        self.write_net(|d| {
            let n = net::network(d).ok_or_else(|| Error::NotFound("network".into()))?;
            net::set_network(d, &net::NetworkInfo { copies: Some(copies), ..n })
        })
    }

    /// What this device holds of a document: its fingerprint, and whether
    /// all of its files are here. `None` if it does not hold it.
    pub(crate) fn holds(&self, doc: &str) -> Result<Option<(Vec<u8>, bool)>> {
        let store = self.0.store.lock().unwrap();
        let Some(meta) = store.meta(doc)? else { return Ok(None) };
        Ok(Some((meta.heads, store.files_complete(doc)?)))
    }

    /// Another device told us what it holds of a document.
    pub(crate) fn confirm(&self, peer: EndpointId, doc: &str, held: Option<(Vec<u8>, bool)>) {
        let mut all = self.0.confirmed.lock().unwrap();
        let of_peer = all.entry(peer).or_default();
        match held {
            Some(h) => of_peer.insert(doc.to_string(), h),
            None => of_peer.remove(doc),
        };
        drop(all);
        self.0.release.notify_one();
    }

    /// How many connected storage devices hold `doc` exactly as this device
    /// has it, with all of its files.
    pub(crate) fn confirmations(&self, doc: &str) -> usize {
        let Ok(Some(mine)) = self.fingerprint(doc) else { return 0 };
        let peers: Vec<EndpointId> = self
            .0
            .confirmed
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, docs)| docs.get(doc).is_some_and(|(fp, complete)| *complete && *fp == mine))
            .map(|(peer, _)| *peer)
            .collect();
        peers.iter().filter(|p| self.is_keeper(&p.to_string())).count()
    }

    /// Let go of what this device was asked not to keep, where that is safe.
    pub(crate) fn release(&self) -> Result<()> {
        if self.keeps_everything() {
            return Ok(());
        }
        let copies = self.read_net(net::copies);
        // Files of list-only containers: (id, size, last used, safe to drop).
        let mut cached: Vec<(String, u64, i64, bool)> = Vec::new();
        for doc in self.local_doc_ids()? {
            let Some((container, _)) = net::parse_doc_id(&doc) else { continue };
            if doc == NET_DOC || !self.doc_exists(&doc)? {
                continue;
            }
            let wish = self.wish(container);
            if wish == Keep::All {
                continue;
            }
            let safe = self.confirmations(&doc) >= copies;
            if wish == Keep::None {
                if safe {
                    info!(doc = %doc, "letting go: storage devices have it");
                    self.forget_doc(&doc)?;
                    let _ = self.0.doc_changes.send(DocChange { doc: doc.clone(), content: false });
                    self.emit(Event::ContainerChanged { container: container.to_string() });
                }
                continue;
            }
            let here = self.0.store.lock().unwrap().files_here(&doc)?;
            cached.extend(here.into_iter().map(|(id, size, used)| (id, size, used, safe)));
        }
        let limit = self.cache_limit();
        let mut total: u64 = cached.iter().map(|f| f.1).sum();
        cached.sort_by_key(|f| f.2);
        for (id, size, _, safe) in cached {
            if total <= limit {
                break;
            }
            if safe {
                self.evict_file(&id)?;
                total -= size;
            }
        }
        Ok(())
    }

    /// This device's file cache, in bytes.
    pub(crate) fn cache_limit(&self) -> u64 {
        let me = self.0.me.to_string();
        self.read_net(|d| net::member(d, &me).and_then(|m| m.cache)).unwrap_or(DEFAULT_CACHE)
    }

    /// How many bytes of files a device may hold for containers of which
    /// it keeps only the list, before the least recently opened ones are
    /// dropped (1 GiB unless set). Like the storage switch, it can be set
    /// for any device from any device.
    pub fn set_cache_limit(&self, device: &str, bytes: u64) -> Result<()> {
        self.write_net(|d| {
            let m = net::member(d, device).filter(|m| !m.removed).ok_or_else(|| Error::NotFound(device.into()))?;
            net::set_member(d, device, &net::Member { cache: Some(bytes), ..m })
        })
    }
}

pub(crate) async fn release_loop(weak: Weak<Inner>) {
    let mut shutdown = match weak.upgrade() {
        Some(i) => i.shutdown.subscribe(),
        None => return,
    };
    loop {
        let Some(node) = Node::from_weak(&weak) else { return };
        if let Err(e) = node.release() {
            warn!("release failed: {e}");
        }
        let inner = node.0.clone();
        drop(node);
        tokio::select! {
            _ = inner.release.notified() => {}
            _ = tokio::time::sleep(Duration::from_secs(60)) => {}
            _ = shutdown.changed() => return,
        }
    }
}
