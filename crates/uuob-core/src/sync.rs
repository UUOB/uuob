//! Device-to-device sync.
//!
//! Every pair of online devices keeps one QUIC connection. Over it, each
//! container document is kept in sync with Automerge's sync protocol: on
//! connect both sides exchange a fingerprint of each document, sync the
//! ones that differ, and push every later change immediately. Documents
//! that are the same on both sides are never loaded.
//!
//! Two kinds of device may connect: the members of this network, which sync
//! everything, and the devices of people we are linked with, which get only
//! what was shared with them (see `people.rs`).

use std::collections::{HashMap, HashSet};
use std::sync::Weak;
use std::time::Duration;

use anyhow::bail;
use iroh::EndpointId;
use iroh::endpoint::Connection;
use iroh::protocol::{AcceptError, ProtocolHandler};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::error::RecvError;
use tracing::{debug, warn};

use crate::net::{self, Access, NET_DOC};
use crate::node::{Inner, Node, SYNC_ALPN};
use crate::people::About;
use crate::wire::{read_frame, write_frame};

#[derive(Serialize, Deserialize)]
enum Msg {
    Hello { network: String },
    Sync { doc: String, data: Vec<u8> },
    /// Between two people's devices: who we are and what we share.
    About(About),
    /// Sent after connecting: each document the other device may have,
    /// with its fingerprint and whether all of its files are here.
    Heads(Vec<(String, Vec<u8>, bool)>),
    /// A storage device confirms what it holds of a document now.
    Have { doc: String, fingerprint: Vec<u8>, complete: bool },
    /// This device no longer holds the document (it was asked not to keep it).
    Gone { doc: String },
}

#[derive(Debug)]
pub(crate) struct SyncHandler(pub Weak<Inner>);

impl ProtocolHandler for SyncHandler {
    async fn accept(&self, conn: Connection) -> Result<(), AcceptError> {
        let Some(node) = Node::from_weak(&self.0) else { return Ok(()) };
        let remote = conn.remote_id();
        // Removed devices may still connect, but only to learn they were removed.
        let known = node.read_net(|d| net::member(d, &remote.to_string()).is_some()) || node.person_of(&remote).is_some();
        if !known {
            debug!(peer = %remote.fmt_short(), "rejecting unknown device");
            conn.close(1u32.into(), b"not a member");
            return Ok(());
        }
        if let Err(e) = run_session(node, conn, false).await {
            debug!(peer = %remote.fmt_short(), "session ended: {e:#}");
        }
        Ok(())
    }
}

/// Keep trying to connect to every device we should be connected to: our
/// own, and those of the people we are linked with.
pub(crate) async fn dial_loop(weak: Weak<Inner>) {
    let mut shutdown = match weak.upgrade() {
        Some(i) => i.shutdown.subscribe(),
        None => return,
    };
    loop {
        {
            let Some(node) = Node::from_weak(&weak) else { return };
            for eid in node.dial_targets() {
                let busy = node.0.peers.lock().unwrap().contains_key(&eid)
                    || !node.0.dialing.lock().unwrap().insert(eid);
                if busy {
                    continue;
                }
                let node = node.clone();
                tokio::spawn(async move {
                    let res = tokio::time::timeout(Duration::from_secs(20), node.0.endpoint.connect(eid, SYNC_ALPN)).await;
                    node.0.dialing.lock().unwrap().remove(&eid);
                    match res {
                        Ok(Ok(conn)) => {
                            if let Err(e) = run_session(node, conn, true).await {
                                debug!(peer = %eid.fmt_short(), "session ended: {e:#}");
                            }
                        }
                        Ok(Err(e)) => debug!(peer = %eid.fmt_short(), "connect failed: {e}"),
                        Err(_) => debug!(peer = %eid.fmt_short(), "connect timed out"),
                    }
                });
            }
        }
        let wake = weak.upgrade();
        let Some(inner) = wake else { return };
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(10)) => {}
            _ = inner.wake.notified() => {}
            _ = shutdown.changed() => return,
        }
    }
}

async fn run_session(node: Node, conn: Connection, initiated_by_me: bool) -> anyhow::Result<()> {
    let remote = conn.remote_id();
    let network = node.read_net(|d| net::network(d).map(|n| n.id).unwrap_or_default());
    // `Some` when the other device belongs to a person we are linked with.
    let person = node.person_of(&remote);

    let (mut send, recv) = if initiated_by_me {
        let (mut s, r) = conn.open_bi().await?;
        write_frame(&mut s, &Msg::Hello { network: network.clone() }).await?;
        (s, r)
    } else {
        let (s, mut r) = conn.accept_bi().await?;
        let expected = person.as_ref().unwrap_or(&network);
        match read_frame::<Msg>(&mut r).await? {
            Msg::Hello { network: theirs } if theirs == *expected => {}
            _ => bail!("peer is in a different network"),
        }
        (s, r)
    };

    if let Some(p) = &person {
        if !node.is_active_peer(&remote) {
            // No longer welcome. If we unlinked the person, say so; then hang up.
            let about = node.about(p);
            if about.gone {
                write_frame(&mut send, &Msg::About(about)).await?;
            }
            send.finish()?;
            tokio::time::timeout(Duration::from_secs(5), conn.closed()).await.ok();
            return Ok(());
        }
    }

    let Some(session) = node.register_peer(remote, &conn, initiated_by_me) else {
        conn.close(0u32.into(), b"duplicate");
        return Ok(());
    };
    if !node.is_active_peer(&remote) {
        node.drop_non_members();
    }
    // File requests arrive on further streams of this connection.
    let files = tokio::spawn(crate::files::serve(node.clone(), remote, conn.clone()));
    let result = session_loop(&node, &conn, remote, person, &mut send, recv).await;
    files.abort();
    node.unregister_peer(remote, session);
    result
}

/// One side of a sync session.
struct Session<'a> {
    node: &'a Node,
    remote: EndpointId,
    /// The person the other device belongs to, if it is not one of ours.
    person: Option<String>,
    send: &'a mut iroh::endpoint::SendStream,
    states: HashMap<String, automerge::sync::State>,
    /// Person sessions: the containers of ours that they accepted. Nothing
    /// is sent before that.
    accepted: HashSet<String>,
    last_about: Option<About>,
    /// Documents found identical on both sides when the session started and
    /// untouched since. They are skipped, so they stay out of memory.
    level: HashSet<String>,
    /// The other device's fingerprints have arrived.
    compared: bool,
    /// Documents the other device holds.
    theirs: HashSet<String>,
    /// Documents held back because the other device was asked not to keep
    /// them; offered again when the network document changes.
    skipped: HashSet<String>,
    /// This device announced itself as a storage device in this session.
    announced: bool,
}

impl Session<'_> {
    /// Send our next sync message for `doc`, if the other device may have
    /// it and there is anything to say.
    async fn push(&mut self, doc: &str) -> anyhow::Result<()> {
        if self.level.contains(doc)
            || self.node.peer_access(&self.remote, doc).is_none()
            || !self.node.doc_exists(doc)?
        {
            return Ok(());
        }
        if self.person.is_none() && !self.theirs.contains(doc) && !self.node.peer_keeps(&self.remote, doc) {
            self.skipped.insert(doc.to_string());
            return Ok(());
        }
        if let Some(p) = &self.person {
            let container = net::parse_doc_id(doc).map(|(c, _)| c).unwrap_or_default();
            if !self.accepted.contains(container) && !self.node.is_from(doc, p) {
                return Ok(());
            }
        }
        let state = self.states.entry(doc.to_string()).or_default();
        let data = self.node.write_doc(doc, None, |d| {
            use automerge::sync::SyncDoc;
            Ok(d.sync().generate_sync_message(state).map(|m| m.encode()))
        })?;
        if let Some(data) = data {
            if let Err(e) = write_frame(self.send, &Msg::Sync { doc: doc.to_string(), data }).await {
                warn!("send failed: {e}");
                return Err(e);
            }
        }
        Ok(())
    }

    async fn push_all(&mut self) -> anyhow::Result<()> {
        // Start with the network document so membership/registry arrive first.
        let mut ids = self.node.local_doc_ids()?;
        ids.sort_by_key(|id| id != NET_DOC);
        for id in ids {
            self.push(&id).await?;
        }
        Ok(())
    }

    /// Tell the other device the state of every document it may have.
    async fn compare(&mut self) -> anyhow::Result<()> {
        let mut heads = Vec::new();
        for id in self.node.local_doc_ids()? {
            if self.node.peer_access(&self.remote, &id).is_some() {
                if let Some((fingerprint, complete)) = self.node.holds(&id)? {
                    heads.push((id, fingerprint, complete));
                }
            }
        }
        self.announced = self.node.keeps_everything();
        write_frame(self.send, &Msg::Heads(heads)).await
    }

    /// Tell one of our own devices what this one holds of a document now: a
    /// storage device confirms what it has, and any device says when a
    /// document is gone from it.
    async fn report(&mut self, doc: &str) -> anyhow::Result<()> {
        if self.person.is_some() || doc == NET_DOC {
            return Ok(());
        }
        match self.node.holds(doc)? {
            None => write_frame(self.send, &Msg::Gone { doc: doc.into() }).await,
            Some((fingerprint, complete)) if self.node.keeps_everything() => {
                write_frame(self.send, &Msg::Have { doc: doc.into(), fingerprint, complete }).await
            }
            Some(_) => Ok(()),
        }
    }

    /// The other device holds this much of a document (`None`: nothing).
    fn theirs(&mut self, doc: String, held: Option<(Vec<u8>, bool)>) {
        if self.person.is_none() {
            self.node.confirm(self.remote, &doc, held.clone());
        }
        match held {
            Some(_) => self.theirs.insert(doc),
            None => {
                self.states.remove(&doc);
                self.level.remove(&doc);
                self.theirs.remove(&doc)
            }
        };
    }

    /// The network document changed: wishes may have, too.
    async fn rethink(&mut self) -> anyhow::Result<()> {
        if self.person.is_some() {
            return Ok(());
        }
        // Offer again what was held back.
        for doc in std::mem::take(&mut self.skipped) {
            self.push(&doc).await?;
        }
        // Newly a storage device: say what is here.
        if !self.announced && self.node.keeps_everything() {
            self.compare().await?;
        }
        Ok(())
    }

    /// The other device's document states arrived: leave alone what is
    /// identical, sync the rest.
    async fn compared(&mut self, theirs: Vec<(String, Vec<u8>, bool)>) -> anyhow::Result<()> {
        for (doc, fingerprint, complete) in theirs {
            if self.node.fingerprint(&doc)?.is_some_and(|mine| mine == fingerprint) {
                self.level.insert(doc.clone());
            }
            self.theirs(doc, Some((fingerprint, complete)));
        }
        // A device may send this again later (it became a storage device).
        if std::mem::replace(&mut self.compared, true) {
            return Ok(());
        }
        self.push_all().await
    }

    /// Tell a person's device who we are and what we share, if that changed.
    async fn tell(&mut self) -> anyhow::Result<()> {
        let Some(p) = &self.person else { return Ok(()) };
        let about = self.node.about(p);
        if self.last_about.as_ref() != Some(&about) {
            write_frame(self.send, &Msg::About(about.clone())).await?;
            self.last_about = Some(about);
        }
        Ok(())
    }

    async fn receive(&mut self, doc: String, data: Vec<u8>) -> anyhow::Result<()> {
        use automerge::sync::SyncDoc;
        self.level.remove(&doc);
        self.theirs.insert(doc.clone());
        if !self.node.wants_doc(&doc) {
            return Ok(());
        }
        let Some(access) = self.node.peer_access(&self.remote, &doc) else { return Ok(()) };
        let message = automerge::sync::Message::decode(&data)?;
        let state = self.states.entry(doc.clone()).or_default();
        match access {
            Access::Write => self.node.write_doc(&doc, Some(self.remote), |d| {
                d.sync().receive_sync_message(state, message)?;
                Ok(())
            })?,
            // It may read but not change this (a removed device and the
            // network document, or a person with view-only access): keep the
            // sync dialogue going, but apply its changes to a throwaway copy.
            Access::Read => self.node.read_doc(&doc, |d| {
                let mut scratch = d.clone();
                scratch.sync().receive_sync_message(state, message)
            })??,
        }
        self.push(&doc).await
    }

    async fn hear(&mut self, about: About) -> anyhow::Result<()> {
        let Some(p) = self.person.clone() else { return Ok(()) };
        self.node.apply_about(&p, &about)?;
        // Start from scratch for anything they accepted just now.
        let accepted: HashSet<String> = about.accepted.into_iter().collect();
        let fresh: Vec<String> = accepted.difference(&self.accepted).cloned().collect();
        let is_fresh = |doc: &String| net::parse_doc_id(doc).is_some_and(|(c, _)| fresh.iter().any(|f| f == c));
        self.states.retain(|doc, _| !is_fresh(doc));
        self.level.retain(|doc| !is_fresh(doc));
        self.accepted = accepted;
        self.tell().await?;
        // Before the fingerprints are compared nothing is pushed.
        if self.compared { self.push_all().await } else { Ok(()) }
    }
}

async fn session_loop(
    node: &Node,
    conn: &Connection,
    remote: EndpointId,
    person: Option<String>,
    send: &mut iroh::endpoint::SendStream,
    mut recv: iroh::endpoint::RecvStream,
) -> anyhow::Result<()> {
    let mut changes = node.0.doc_changes.subscribe();
    let mut s = Session {
        node,
        remote,
        person,
        send,
        states: HashMap::new(),
        accepted: HashSet::new(),
        last_about: None,
        level: HashSet::new(),
        compared: false,
        theirs: HashSet::new(),
        skipped: HashSet::new(),
        announced: false,
    };

    // Reader task: decouples (non cancel-safe) frame reads from the select loop.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Msg>(64);
    let reader = tokio::spawn(async move {
        while let Ok(msg) = read_frame::<Msg>(&mut recv).await {
            if tx.send(msg).await.is_err() {
                break;
            }
        }
    });

    s.tell().await?;
    s.compare().await?;

    let result = loop {
        tokio::select! {
            msg = rx.recv() => match msg {
                None => break Ok(()),
                Some(Msg::Sync { doc, data }) => s.receive(doc, data).await?,
                Some(Msg::About(about)) => s.hear(about).await?,
                Some(Msg::Heads(theirs)) => s.compared(theirs).await?,
                Some(Msg::Have { doc, fingerprint, complete }) => s.theirs(doc, Some((fingerprint, complete))),
                Some(Msg::Gone { doc }) => s.theirs(doc, None),
                Some(Msg::Hello { .. }) => {}
            },
            change = changes.recv() => match change {
                Ok(c) if c.doc == NET_DOC && s.person.is_some() => s.tell().await?,
                Ok(c) => {
                    if c.content {
                        s.level.remove(&c.doc);
                        if node.wants_doc(&c.doc) {
                            s.push(&c.doc).await?;
                        }
                        if c.doc == NET_DOC {
                            s.rethink().await?;
                        }
                    }
                    s.report(&c.doc).await?;
                }
                Err(RecvError::Lagged(_)) => {
                    // Changes were missed, so nothing is known to be identical.
                    s.level.clear();
                    s.tell().await?;
                    s.push_all().await?;
                }
                Err(RecvError::Closed) => break Ok(()),
            },
            _ = conn.closed() => break Ok(()),
        }
    };
    reader.abort();
    result
}
