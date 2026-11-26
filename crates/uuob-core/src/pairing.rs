//! Linking devices, linking with other people, and connecting apps, with a
//! short code like `42-amber-river-falcon`.
//!
//! The same kind of code does three different things, and whoever enters it
//! says which one it means: *join* makes a device one of your own, *meet*
//! links two people who stay separate (see `people.rs`), *link* connects an
//! app, also one on a device that has no UUOB (see `remote.rs`). A code made
//! for one is refused for the others.
//!
//! The code is the whole invite (the QR code just shows it). Both devices
//! stretch it with Argon2 into two secrets:
//! - a rendezvous key: the inviting device runs a temporary endpoint with
//!   this key, so the joining device can find and dial it by public key
//!   (iroh discovery) and knows it is talking to someone holding the code;
//! - a join secret, which the joining device sends over that encrypted
//!   connection to prove it holds the code.
//!
//! The invite is single-use, expires after 10 minutes and is burned after
//! 3 wrong attempts.

use std::collections::BTreeMap;
use std::sync::Weak;
use std::time::Duration;

use automerge::AutoCommit;
use iroh::endpoint::Connection;
use iroh::protocol::{AcceptError, ProtocolHandler, Router};
use iroh::{Endpoint, EndpointAddr, SecretKey};
use rand::Rng;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::error::{Error, Result};
use crate::net::{self, Access, ContainerInfo, Member};
use crate::node::{AppManifest, Event, Inner, Node, PAIR_ALPN, bind_endpoint};
use crate::people::Card;
use crate::store::now_ms;
use crate::wire::{read_frame, write_frame};
use crate::words::WORDS;

pub const INVITE_TTL: Duration = Duration::from_secs(10 * 60);
const MAX_FAILURES: u32 = 3;
/// Wrong app codes handed in on this device before every open one is cancelled.
const MAX_APP_CODE_FAILURES: u32 = 5;

#[derive(Debug, Clone, Serialize)]
pub struct Invite {
    pub code: String,
    pub expires_at: i64,
}

/// What an invite is for.
#[derive(Clone)]
enum Kind {
    /// A new device of ours.
    Device,
    /// Another person; optionally share a container with them right away.
    Person { share: Option<(String, Access)> },
    /// An app, on this device or another one.
    App,
}

pub(crate) struct PendingInvite {
    kind: Kind,
    join_secret: [u8; 32],
    expires_at: i64,
    failures: u32,
    pub(crate) rendezvous: Option<Router>,
}

#[derive(Serialize, Deserialize)]
enum PairMsg {
    Join { proof: [u8; 32], name: String, platform: String },
    Welcome { net_doc: Vec<u8> },
    Rejected { reason: String },
    /// Link as two people: "here is who I am".
    Meet { proof: [u8; 32], card: Card },
    Met { card: Card },
    /// An app on a device without UUOB connects. It gets its token and the
    /// ids of the network's devices, any of which will serve it.
    Link { proof: [u8; 32], app: AppManifest, device: String },
    Linked { token: String, devices: Vec<String> },
}

/// Normalize user input: case, spaces and separators don't matter.
pub fn normalize_code(input: &str) -> String {
    input
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|p| !p.is_empty())
        .map(|p| p.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("-")
}

fn generate_code() -> String {
    let mut rng = rand::rngs::OsRng;
    let n: u8 = rng.gen_range(2..100);
    let w = |rng: &mut rand::rngs::OsRng| WORDS[rng.gen_range(0..WORDS.len())];
    format!("{n}-{}-{}-{}", w(&mut rng), w(&mut rng), w(&mut rng))
}

/// Code → (rendezvous key, join secret).
fn derive(code: &str) -> Result<(SecretKey, [u8; 32])> {
    let params = argon2::Params::new(19 * 1024, 2, 1, Some(64)).map_err(|e| anyhow::anyhow!("{e}"))?;
    let argon = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let mut out = [0u8; 64];
    argon
        .hash_password_into(code.as_bytes(), b"uuob-pairing-v1", &mut out)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let rendezvous: [u8; 32] = out[..32].try_into().unwrap();
    let join: [u8; 32] = out[32..].try_into().unwrap();
    Ok((SecretKey::from_bytes(&rendezvous), join))
}

/// Find the device showing `code` and connect to it. Also returns the proof
/// that we hold the code.
pub(crate) async fn dial_code(endpoint: &Endpoint, code: &str) -> Result<(Connection, [u8; 32])> {
    let code = normalize_code(code);
    let (rendezvous_key, proof) = tokio::task::spawn_blocking(move || derive(&code)).await.map_err(anyhow::Error::from)??;
    let target = EndpointAddr::new(rendezvous_key.public());

    // Discovery records can take a moment to appear; retry briefly.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        match tokio::time::timeout(Duration::from_secs(10), endpoint.connect(target.clone(), PAIR_ALPN)).await {
            Ok(Ok(c)) => return Ok((c, proof)),
            _ if tokio::time::Instant::now() < deadline => tokio::time::sleep(Duration::from_millis(500)).await,
            _ => return Err(Error::Pairing("no device is showing this code (check it, or create a new one)".into())),
        }
    }
}

/// An app on a device without UUOB uses a code from UUOB → Apps: returns its
/// token and the devices that will serve it (see `remote.rs`).
pub(crate) async fn link_app(endpoint: &Endpoint, code: &str, app: &AppManifest) -> Result<(String, Vec<String>)> {
    let (conn, proof) = dial_code(endpoint, code).await?;
    let (mut send, mut recv) = conn.open_bi().await.map_err(anyhow::Error::from)?;
    write_frame(&mut send, &PairMsg::Link { proof, app: app.clone(), device: crate::device_name() }).await?;
    let reply: PairMsg = read_frame(&mut recv).await?;
    conn.close(0u32.into(), b"done");
    match reply {
        PairMsg::Linked { token, devices } => Ok((token, devices)),
        PairMsg::Rejected { reason } => Err(Error::Pairing(reason)),
        _ => Err(Error::Pairing("unexpected reply".into())),
    }
}

fn ct_eq(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

impl Node {
    /// Start inviting a new device. Show the returned code (and/or a QR code
    /// of it) and enter it on the other device.
    pub async fn create_invite(&self) -> Result<Invite> {
        self.new_invite(Kind::Device).await
    }

    /// Start linking with another person: they enter the code under People
    /// in their UUOB. With `share`, that container is shared with them as
    /// soon as they do.
    pub async fn create_person_invite(&self, share: Option<(String, Access)>) -> Result<Invite> {
        self.card()?;
        if let Some((container, _)) = &share {
            let c = self.owned(&crate::node::Client::Shell, container)?;
            if c.from.is_some() || *container == c.owner_app {
                return Err(Error::Invalid("this cannot be shared".into()));
            }
        }
        self.new_invite(Kind::Person { share }).await
    }

    /// A code that connects an app: the user types it into the app, on this
    /// device or on one that has no UUOB.
    pub async fn create_app_code(&self) -> Result<Invite> {
        self.new_invite(Kind::App).await
    }

    /// An app on this device hands in a code from [`Node::create_app_code`].
    pub(crate) async fn take_app_code(&self, code: &str) -> Result<()> {
        let code = normalize_code(code);
        let taken = {
            let mut invites = self.0.invites.lock().unwrap();
            let good = invites.get(&code).is_some_and(|i| matches!(i.kind, Kind::App) && i.expires_at >= now_ms());
            if good {
                // Typos count only until a code is right, not forever.
                self.0.app_code_failures.store(0, std::sync::atomic::Ordering::Relaxed);
                invites.remove(&code)
            } else {
                // Too many wrong guesses cancel every open app code.
                use std::sync::atomic::Ordering::Relaxed;
                if self.0.app_code_failures.fetch_add(1, Relaxed) + 1 >= MAX_APP_CODE_FAILURES {
                    self.0.app_code_failures.store(0, Relaxed);
                    invites.retain(|_, i| !matches!(i.kind, Kind::App));
                }
                None
            }
        };
        let invite = taken.ok_or_else(|| Error::PermissionDenied("that code is wrong or has expired".into()))?;
        if let Some(r) = invite.rendezvous {
            let _ = r.shutdown().await;
        }
        Ok(())
    }

    async fn new_invite(&self, kind: Kind) -> Result<Invite> {
        let code = generate_code();
        let (rendezvous_key, join_secret) = tokio::task::spawn_blocking({
            let code = code.clone();
            move || derive(&code)
        })
        .await
        .map_err(anyhow::Error::from)??;

        let ep = bind_endpoint(&self.0.net_mode, rendezvous_key, vec![PAIR_ALPN.to_vec()]).await?;
        let router = Router::builder(ep)
            .accept(PAIR_ALPN, PairHandler { node: std::sync::Arc::downgrade(&self.0), code: code.clone() })
            .spawn();

        let expires_at = now_ms() + INVITE_TTL.as_millis() as i64;
        self.0.invites.lock().unwrap().insert(
            code.clone(),
            PendingInvite { kind, join_secret, expires_at, failures: 0, rendezvous: Some(router) },
        );

        let node = self.clone();
        let c = code.clone();
        tokio::spawn(async move {
            tokio::time::sleep(INVITE_TTL).await;
            node.cancel_invite(&c).await;
        });
        info!("invite created");
        Ok(Invite { code, expires_at })
    }

    pub async fn cancel_invite(&self, code: &str) {
        let inv = self.0.invites.lock().unwrap().remove(code);
        if let Some(r) = inv.and_then(|i| i.rendezvous) {
            let _ = r.shutdown().await;
        }
    }

    /// Join the network of the device that showed `code`.
    ///
    /// Data already on this device is kept and merged into the network.
    pub async fn join(&self, code: &str) -> Result<()> {
        let me = self.0.me.to_string();
        let others = self.read_net(|d| net::members(d).iter().filter(|(id, m)| !m.removed && **id != me).count());
        if others > 0 {
            return Err(Error::Pairing(
                "this device is already linked to other devices; leave that network first".into(),
            ));
        }
        let (conn, proof) = self.dial_invite(code).await?;
        let (mut send, mut recv) = conn.open_bi().await.map_err(anyhow::Error::from)?;
        let name = self.0.device_name.lock().unwrap().clone();
        write_frame(&mut send, &PairMsg::Join { proof, name, platform: self.0.platform.clone() }).await?;
        let reply: PairMsg = read_frame(&mut recv).await?;
        conn.close(0u32.into(), b"done");
        match reply {
            PairMsg::Welcome { net_doc } => {
                let d = AutoCommit::load(&net_doc)?;
                if !net::is_active_member(&d, &me) {
                    return Err(Error::Pairing("inviting device did not add us".into()));
                }
                self.adopt_network(d)?;
                info!("joined network");
                Ok(())
            }
            PairMsg::Rejected { reason } => Err(Error::Pairing(reason)),
            _ => Err(Error::Pairing("unexpected reply".into())),
        }
    }

    /// Link with the person whose device showed `code`. Nothing is shared
    /// by this; your networks stay separate.
    pub async fn meet(&self, code: &str) -> Result<()> {
        let card = self.card()?;
        let (conn, proof) = self.dial_invite(code).await?;
        let (mut send, mut recv) = conn.open_bi().await.map_err(anyhow::Error::from)?;
        write_frame(&mut send, &PairMsg::Meet { proof, card }).await?;
        let reply: PairMsg = read_frame(&mut recv).await?;
        conn.close(0u32.into(), b"done");
        match reply {
            PairMsg::Met { card } => {
                self.add_person(&card)?;
                info!("linked with a person");
                Ok(())
            }
            PairMsg::Rejected { reason } => Err(Error::Pairing(reason)),
            _ => Err(Error::Pairing("unexpected reply".into())),
        }
    }

    async fn dial_invite(&self, code: &str) -> Result<(Connection, [u8; 32])> {
        dial_code(&self.0.endpoint, code).await
    }

    /// Switch to a network document received while joining, keeping our own
    /// apps and data: unknown apps/containers are registered in the network,
    /// and our container data is merged into the network's current generation.
    fn adopt_network(&self, mut new: AutoCommit) -> Result<()> {
        let (old_apps, old_containers) = self.read_net(|d| (net::apps(d), net::containers(d)));
        let new_apps = net::apps(&new);
        let new_containers = net::containers(&new);
        for (id, a) in &old_apps {
            if !new_apps.contains_key(id) {
                net::set_app(&mut new, id, a)?;
            }
        }
        let mut moves: BTreeMap<String, String> = BTreeMap::new();
        for (id, c) in &old_containers {
            // What other people shared with this device belonged to the
            // network it is leaving, and does not come along.
            if c.deleted || c.from.is_some() {
                continue;
            }
            match new_containers.get(id) {
                None => net::set_container(&mut new, id, c)?,
                Some(nc) if nc.deleted => {
                    let revived = ContainerInfo { deleted: false, ..nc.clone() };
                    net::set_container(&mut new, id, &revived)?;
                    moves.insert(net::doc_id(id, c.generation), net::doc_id(id, nc.generation));
                }
                Some(nc) if nc.generation != c.generation => {
                    moves.insert(net::doc_id(id, c.generation), net::doc_id(id, nc.generation));
                }
                Some(_) => {}
            }
        }
        // Merge data whose generation differs into the network's generation.
        for (from, to) in &moves {
            if !self.doc_exists(from)? {
                continue;
            }
            let mut src = self.read_doc(from, |d| d.clone())?;
            self.write_doc(to, None, |dst| {
                dst.merge(&mut src)?;
                Ok(())
            })?;
        }
        self.replace_net_doc(new)
    }

    fn add_member(&self, id: &str, name: String, platform: String) -> Result<Vec<u8>> {
        self.write_net(|d| net::set_member(d, id, &Member { name, platform, added_at: now_ms(), removed: false, keeper: false, cache: None }))?;
        self.read_doc(net::NET_DOC, |d| d.clone().save())
    }
}

/// Serves one invite on its temporary rendezvous endpoint.
#[derive(Debug)]
pub(crate) struct PairHandler {
    node: Weak<Inner>,
    code: String,
}

impl ProtocolHandler for PairHandler {
    async fn accept(&self, conn: Connection) -> Result<(), AcceptError> {
        let Some(node) = Node::from_weak(&self.node) else { return Ok(()) };
        if let Err(e) = self.handle(&node, &conn).await {
            warn!("pairing attempt failed: {e:#}");
        }
        Ok(())
    }
}

impl PairHandler {
    async fn handle(&self, node: &Node, conn: &Connection) -> anyhow::Result<()> {
        let joiner = conn.remote_id();
        let (mut send, mut recv) = conn.accept_bi().await?;
        let msg: PairMsg = read_frame(&mut recv).await?;
        let proof = match &msg {
            PairMsg::Join { proof, .. } | PairMsg::Meet { proof, .. } | PairMsg::Link { proof, .. } => *proof,
            _ => anyhow::bail!("unexpected message"),
        };

        let verdict = {
            let mut invites = node.0.invites.lock().unwrap();
            match invites.get_mut(&self.code) {
                None => Err("this code is no longer valid"),
                Some(inv) if inv.expires_at < now_ms() => Err("this code has expired"),
                Some(inv) if !ct_eq(&inv.join_secret, &proof) => {
                    inv.failures += 1;
                    if inv.failures >= MAX_FAILURES {
                        invites.remove(&self.code);
                    }
                    Err("wrong code")
                }
                // The code is right but was made for the other purpose. It
                // stays valid for the right one.
                Some(inv) => match (&inv.kind, &msg) {
                    (Kind::Device, PairMsg::Meet { .. }) => {
                        Err("this code adds a device to someone's own network; ask them for a code from People instead")
                    }
                    (Kind::Person { .. }, PairMsg::Join { .. }) => {
                        Err("this code is for linking with a person; enter it under People")
                    }
                    (Kind::App, PairMsg::Join { .. } | PairMsg::Meet { .. }) => {
                        Err("this code connects an app; type it into the app")
                    }
                    (Kind::Device | Kind::Person { .. }, PairMsg::Link { .. }) => {
                        Err("this code does not connect an app; in UUOB, open Apps and choose Connect with a code")
                    }
                    _ => Ok(invites.remove(&self.code).expect("present")),
                },
            }
        };

        let invite = match verdict {
            Err(reason) => {
                write_frame(&mut send, &PairMsg::Rejected { reason: reason.into() }).await?;
                send.finish()?;
                conn.closed().await;
                return Ok(());
            }
            Ok(invite) => invite,
        };

        let reply = match (msg, invite.kind) {
            (PairMsg::Join { name, platform, .. }, _) => {
                let net_doc = node.add_member(&joiner.to_string(), name.clone(), platform)?;
                node.emit(Event::DeviceJoined { id: joiner.to_string(), name });
                info!(peer = %joiner.fmt_short(), "device joined");
                Ok(PairMsg::Welcome { net_doc })
            }
            (PairMsg::Meet { card, .. }, Kind::Person { share }) => node
                .card()
                .and_then(|mine| {
                    node.add_person(&card)?;
                    if let Some((container, access)) = share {
                        node.share(&container, &card.network, Some(access))?;
                    }
                    info!("linked with a person");
                    Ok(PairMsg::Met { card: mine })
                }),
            (PairMsg::Link { app, device, .. }, Kind::App) => node.grant_app(&app, Some((joiner.to_string(), device))).map(|token| {
                info!(app = %app.id, "app linked from another device");
                PairMsg::Linked { token, devices: node.devices().into_iter().map(|d| d.id).collect() }
            }),
            _ => unreachable!("checked above"),
        }
        .unwrap_or_else(|e: Error| PairMsg::Rejected { reason: e.to_string() });
        write_frame(&mut send, &reply).await?;
        send.finish()?;
        conn.closed().await;
        // The invite is used up; stop the rendezvous endpoint.
        if let Some(r) = invite.rendezvous {
            tokio::spawn(async move {
                let _ = r.shutdown().await;
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_normalize() {
        assert_eq!(normalize_code("  42 Amber_River-falcon "), "42-amber-river-falcon");
        let c = generate_code();
        assert_eq!(normalize_code(&c), c);
        assert_eq!(c.split('-').count(), 4);
    }
}
