//! Files: photos, video, anything that is not a record.
//!
//! A file is stored once, outside the documents, encrypted with a key of
//! its own in 64 KiB pieces, and is never held in memory whole. Its id is
//! the SHA-256 of the encrypted bytes, so every device stores the same bytes
//! and can check its copy. The container's document lists its files at root
//! keys `~file/<id>` (size, type, key): removing the entry removes the file
//! on every device, and whoever may read the document may read its files.
//!
//! Devices fetch the files they are missing over extra streams on the sync
//! connection: "send me this file of this document from byte N".

use std::collections::HashSet;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::sync::Weak;
use std::time::Duration;

use automerge::{AutoCommit, ROOT, ReadDoc, transaction::Transactable};
use iroh::EndpointId;
use iroh::endpoint::{Connection, RecvStream, SendStream};
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use sha2::{Digest, Sha256};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tracing::debug;

use crate::crypto;
use crate::doc;
use crate::error::{Error, Result};
use crate::net::{self, Access, Keep};
use crate::node::{Client, DocChange, Event, Inner, Node};
use crate::store::{Store, now_ms};
use crate::wire::{read_frame, write_frame};

/// Plaintext bytes per piece.
const PIECE: usize = 64 * 1024;
/// What sealing adds to a piece: a nonce and an authentication tag.
const SEAL: usize = 40;
/// Root keys of a container document that list its files.
pub(crate) const PREFIX: &str = "~file/";

/// A file as apps see it.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct FileInfo {
    pub id: String,
    pub size: u64,
    #[serde(rename = "type")]
    pub content_type: String,
    pub added: i64,
    /// Whether the bytes are on this device.
    pub here: bool,
}

/// What a document says about one of its files.
#[derive(Serialize, Deserialize)]
struct Entry {
    size: u64,
    key: String,
    #[serde(rename = "type")]
    content_type: String,
    added: i64,
}

/// File ids become file names, so nothing but a SHA-256 in hex is accepted.
fn valid_id(id: &str) -> bool {
    id.len() == 64 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Binds a piece to its place in the file, so pieces cannot be reordered
/// and a file cannot be cut short unnoticed.
fn aad(index: u64, last: bool) -> [u8; 9] {
    let mut a = [0u8; 9];
    a[..8].copy_from_slice(&index.to_be_bytes());
    a[8] = last as u8;
    a
}

fn pieces(size: u64) -> u64 {
    size.div_ceil(PIECE as u64).max(1)
}

/// Bytes a file of `size` takes when stored.
fn stored_len(size: u64) -> u64 {
    size + pieces(size) * SEAL as u64
}

fn entry(d: &AutoCommit, id: &str) -> Option<Entry> {
    let obj = doc::get_map(d, &ROOT, &format!("{PREFIX}{id}"))?;
    serde_json::from_value(doc::to_json(d, &obj)).ok()
}

fn path(dir: &Path, id: &str) -> PathBuf {
    dir.join(id)
}

fn part(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.part"))
}

/// Bring the store's list of a document's files in line with the document.
/// Called whenever the document changed. True if a listed file is not on
/// this device yet.
pub(crate) fn index(store: &Store, dir: &Path, doc_id: &str, d: &AutoCommit) -> Result<bool> {
    let listed: HashSet<String> =
        d.keys(ROOT).filter_map(|k| k.strip_prefix(PREFIX).filter(|id| valid_id(id)).map(str::to_string)).collect();
    let known = store.files_of(doc_id)?;
    let mut missing = false;
    for id in listed.iter().filter(|id| !known.contains_key(*id)) {
        let Some(e) = entry(d, id) else { continue };
        let here = path(dir, id).exists();
        store.add_file(doc_id, id, e.size, here)?;
        missing |= !here;
    }
    for id in known.keys().filter(|id| !listed.contains(*id)) {
        if store.remove_file(doc_id, id)? {
            remove_bytes(dir, id);
        }
    }
    Ok(missing)
}

fn remove_bytes(dir: &Path, id: &str) {
    let _ = std::fs::remove_file(path(dir, id));
    let _ = std::fs::remove_file(part(dir, id));
}

/// Tidy the files folder when the node starts: drop what no document lists
/// (a deleted container, an upload that never finished) and notice files
/// that have gone missing.
pub(crate) fn sweep(store: &Store, dir: &Path) -> Result<()> {
    let known: std::collections::HashMap<String, bool> = store.all_files()?.into_iter().collect();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let id = name.strip_suffix(".part").unwrap_or(&name);
            if !known.contains_key(id) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    for (id, here) in &known {
        if *here && !path(dir, id).exists() {
            store.set_file_here(id, false)?;
        }
    }
    Ok(())
}

async fn hash_file(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path).await?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; PIECE];
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            return Ok(hex::encode(hash.finalize()));
        }
        hash.update(&buf[..n]);
    }
}

/// A file being stored. Write its bytes in any chunks, then [`finish`](Self::finish).
pub struct FileWriter {
    node: Node,
    container: String,
    content_type: String,
    key: [u8; 32],
    tmp: PathBuf,
    file: File,
    hash: Sha256,
    buf: Vec<u8>,
    piece: u64,
    size: u64,
}

impl FileWriter {
    pub async fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.buf.extend_from_slice(bytes);
        self.size += bytes.len() as u64;
        // The last piece is sealed in `finish`, so one piece is always held back.
        while self.buf.len() > PIECE {
            let piece: Vec<u8> = self.buf.drain(..PIECE).collect();
            self.seal(&piece, false).await?;
        }
        Ok(())
    }

    async fn seal(&mut self, piece: &[u8], last: bool) -> Result<()> {
        let sealed = crypto::seal(&self.key, piece, &aad(self.piece, last));
        self.hash.update(&sealed);
        self.file.write_all(&sealed).await?;
        self.piece += 1;
        Ok(())
    }

    /// Finish the file and list it in its container.
    pub async fn finish(mut self) -> Result<FileInfo> {
        let rest = std::mem::take(&mut self.buf);
        self.seal(&rest, true).await?;
        self.file.flush().await?;
        self.file.sync_all().await?;
        let id = hex::encode(self.hash.clone().finalize());
        tokio::fs::rename(&self.tmp, path(&self.node.0.files_dir, &id)).await?;

        let e = Entry { size: self.size, key: hex::encode(self.key), content_type: self.content_type.clone(), added: now_ms() };
        let Ok(Json::Object(fields)) = serde_json::to_value(&e) else { unreachable!("structs serialize to objects") };
        let doc_id = self.node.active_doc_id(&self.container)?;
        self.node.write_doc(&doc_id, None, |d| doc::set_record(d, &ROOT, &format!("{PREFIX}{id}"), &fields))?;
        Ok(FileInfo { id, size: e.size, content_type: e.content_type, added: e.added, here: true })
    }
}

impl Drop for FileWriter {
    fn drop(&mut self) {
        // Gone already if the file was finished.
        let _ = std::fs::remove_file(&self.tmp);
    }
}

/// Where a file's stored bytes are read from.
enum Source {
    /// This device's copy.
    Here(File),
    /// Another device, piece by piece as they are needed. Each piece is
    /// sealed on its own, so it can be checked without the rest of the file.
    There(RecvStream),
}

/// Reads a range of a file, piece by piece.
pub struct FileReader {
    node: Node,
    id: String,
    file: Source,
    key: [u8; 32],
    size: u64,
    piece: u64,
    skip: usize,
    left: u64,
}

impl FileReader {
    /// The next bytes of the range, or `None` at its end.
    pub async fn next(&mut self) -> Result<Option<Vec<u8>>> {
        if self.left == 0 {
            return Ok(None);
        }
        let last = self.piece + 1 == pieces(self.size);
        let len = if last { (self.size - self.piece * PIECE as u64) as usize } else { PIECE };
        let mut sealed = vec![0u8; len + SEAL];
        let read = match &mut self.file {
            Source::Here(file) => file.read_exact(&mut sealed).await.is_ok(),
            Source::There(stream) => stream.read_exact(&mut sealed).await.is_ok(),
        };
        let plain = if read { crypto::open(&self.key, &sealed, &aad(self.piece, last)).ok() } else { None };
        let Some(plain) = plain else {
            if matches!(self.file, Source::There(_)) {
                return Err(Error::Unavailable("the device that has this file stopped sending it".into()));
            }
            // This copy is damaged: drop it and fetch a good one.
            self.node.forget_file_bytes(&self.id)?;
            return Err(Error::Unavailable("this copy of the file was damaged; it is being fetched again".into()));
        };
        self.piece += 1;
        let from = std::mem::take(&mut self.skip).min(plain.len());
        let take = ((plain.len() - from) as u64).min(self.left) as usize;
        self.left -= take as u64;
        Ok(Some(plain[from..from + take].to_vec()))
    }
}

impl Node {
    fn file_entry(&self, client: &Client, container: &str, id: &str, wanted: Access) -> Result<(String, Entry)> {
        self.check(client, container, wanted)?;
        let doc_id = self.local_doc_id(container)?;
        let e = if valid_id(id) { self.read_doc(&doc_id, |d| entry(d, id))? } else { None };
        Ok((doc_id, e.ok_or_else(|| Error::NotFound(format!("file {id}")))?))
    }

    /// Start storing a file in `container`.
    pub async fn new_file(&self, client: &Client, container: &str, content_type: &str) -> Result<FileWriter> {
        self.check(client, container, Access::Write)?;
        self.local_doc_id(container)?;
        let content_type = content_type.trim();
        if content_type.len() > 128 || !content_type.bytes().all(|b| b.is_ascii_graphic() || b == b' ') {
            return Err(Error::Invalid("invalid content type".into()));
        }
        let dir = &self.0.files_dir;
        tokio::fs::create_dir_all(dir).await?;
        let tmp = dir.join(format!("{}.tmp", crypto::random_id(10)));
        Ok(FileWriter {
            node: self.clone(),
            container: container.into(),
            content_type: if content_type.is_empty() { "application/octet-stream".into() } else { content_type.into() },
            key: crypto::random32(),
            file: File::create(&tmp).await?,
            tmp,
            hash: Sha256::new(),
            buf: Vec::new(),
            piece: 0,
            size: 0,
        })
    }

    /// Store a file that is already in memory.
    pub async fn put_file(&self, client: &Client, container: &str, content_type: &str, bytes: &[u8]) -> Result<FileInfo> {
        let mut w = self.new_file(client, container, content_type).await?;
        w.write(bytes).await?;
        w.finish().await
    }

    /// Open a file to read bytes `start..end` of it (`end`: its end if `None`).
    pub async fn read_file(
        &self,
        client: &Client,
        container: &str,
        id: &str,
        start: u64,
        end: Option<u64>,
    ) -> Result<(FileInfo, FileReader)> {
        let (doc_id, e) = self.file_entry(client, container, id, Access::Read)?;
        let key: [u8; 32] = hex::decode(&e.key)
            .ok()
            .and_then(|k| k.try_into().ok())
            .ok_or_else(|| Error::Invalid("file has no usable key".into()))?;
        let end = end.unwrap_or(e.size).min(e.size);
        let start = start.min(end);
        let file_path = path(&self.0.files_dir, id);
        let piece = start / PIECE as u64;
        let from = piece * (PIECE + SEAL) as u64;
        let away = || Error::Unavailable("this file is on a device that cannot be reached right now".into());
        // Big files, and files that would not fit this device's cache anyway.
        let cached = net::parse_doc_id(&doc_id).is_some_and(|(container, _)| self.wish(container) != Keep::All);
        let streamed = e.size > self.0.stream_over.load(std::sync::atomic::Ordering::Relaxed)
            || (cached && e.size > self.cache_limit());
        let file = match File::open(&file_path).await {
            Ok(mut file) => {
                file.seek(SeekFrom::Start(from)).await?;
                Source::Here(file)
            }
            // Not on this device, and big: read it from a device that has
            // it, starting where the reader starts (a video plays at once).
            Err(_) if streamed => Source::There(self.remote_file(&doc_id, id, e.size, from).await.ok_or_else(away)?),
            // Not on this device, and small: fetch it whole and keep it.
            Err(_) => {
                if !self.fetch_file(&doc_id, id, e.size, false).await {
                    return Err(away());
                }
                match File::open(&file_path).await {
                    Ok(mut file) => {
                        file.seek(SeekFrom::Start(from)).await?;
                        Source::Here(file)
                    }
                    // Dropped again already (the cache was full): read it from there.
                    Err(_) => Source::There(self.remote_file(&doc_id, id, e.size, from).await.ok_or_else(away)?),
                }
            }
        };
        self.0.store.lock().unwrap().touch_file(id)?;
        let info = FileInfo { id: id.into(), size: e.size, content_type: e.content_type, added: e.added, here: true };
        let skip = (start - piece * PIECE as u64) as usize;
        Ok((info, FileReader { node: self.clone(), id: id.into(), file, key, size: e.size, piece, skip, left: end - start }))
    }

    /// A whole file in memory (small files, tests).
    pub async fn get_file(&self, client: &Client, container: &str, id: &str) -> Result<Vec<u8>> {
        let (_, mut r) = self.read_file(client, container, id, 0, None).await?;
        let mut out = Vec::new();
        while let Some(bytes) = r.next().await? {
            out.extend(bytes);
        }
        Ok(out)
    }

    /// The files a container lists.
    pub fn files(&self, client: &Client, container: &str) -> Result<Vec<FileInfo>> {
        self.check(client, container, Access::Read)?;
        let doc_id = self.local_doc_id(container)?;
        let here = self.0.store.lock().unwrap().files_of(&doc_id)?;
        self.read_doc(&doc_id, |d| {
            d.keys(ROOT)
                .filter_map(|k| {
                    let id = k.strip_prefix(PREFIX)?;
                    let e = entry(d, id)?;
                    Some(FileInfo {
                        id: id.into(),
                        size: e.size,
                        content_type: e.content_type,
                        added: e.added,
                        here: here.get(id).copied().unwrap_or(false),
                    })
                })
                .collect()
        })
    }

    /// Remove a file from its container, on every device.
    pub fn delete_file(&self, client: &Client, container: &str, id: &str) -> Result<bool> {
        let Ok((doc_id, _)) = self.file_entry(client, container, id, Access::Write) else {
            // Not listed is not an error; no permission is.
            self.check(client, container, Access::Write)?;
            return Ok(false);
        };
        self.write_doc(&doc_id, None, |d| Ok(d.delete(ROOT, format!("{PREFIX}{id}"))?))?;
        Ok(true)
    }

    /// Drop this device's copy of a file's bytes. Its entry stays, so it
    /// can be fetched again.
    pub(crate) fn evict_file(&self, id: &str) -> Result<()> {
        remove_bytes(&self.0.files_dir, id);
        let docs = self.0.store.lock().unwrap().set_file_here(id, false)?;
        self.files_changed(docs);
        Ok(())
    }

    /// This device's copy is damaged: fetch it again.
    fn forget_file_bytes(&self, id: &str) -> Result<()> {
        self.evict_file(id)?;
        self.0.fetch.notify_one();
        Ok(())
    }

    /// Which of these documents' files are on this device has changed.
    fn files_changed(&self, docs: Vec<String>) {
        for doc in docs {
            if let Some((container, _)) = net::parse_doc_id(&doc) {
                self.emit(Event::ContainerChanged { container: container.to_string() });
            }
            let _ = self.0.doc_changes.send(DocChange { doc, content: false });
        }
        self.0.release.notify_one();
    }

    /// The stored bytes of a file from `offset` on, from a connected device
    /// that has it.
    async fn remote_file(&self, doc: &str, id: &str, size: u64, offset: u64) -> Option<RecvStream> {
        let peers: Vec<(EndpointId, Connection)> =
            self.0.peers.lock().unwrap().iter().map(|(id, p)| (*id, p.conn.clone())).collect();
        for (peer, conn) in peers {
            if self.peer_access(&peer, doc).is_none() {
                continue;
            }
            let asked = async {
                let (mut send, mut recv) = conn.open_bi().await?;
                write_frame(&mut send, &FileReq { doc: doc.into(), id: id.into(), offset }).await?;
                send.finish()?;
                anyhow::Ok(matches!(read_frame(&mut recv).await?, FileResp::Found { size: s } if s == stored_len(size)).then_some(recv))
            };
            if let Ok(Some(stream)) = asked.await {
                return Some(stream);
            }
        }
        None
    }

    /// Files larger than this are read from another device piece by piece
    /// when they are not here, instead of being fetched whole first
    /// (default 32 MiB).
    pub fn set_stream_threshold(&self, bytes: u64) {
        self.0.stream_over.store(bytes, std::sync::atomic::Ordering::Relaxed);
    }

    /// Try to get one missing file from a connected device that may have it.
    async fn fetch_file(&self, doc: &str, id: &str, size: u64, ask_anyone: bool) -> bool {
        // One download per file at a time: a second asker waits for the first.
        let _busy = loop {
            if let Some(busy) = Fetching::start(self, id) {
                break busy;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        };
        if !ask_anyone && path(&self.0.files_dir, id).exists() {
            return true;
        }
        let peers: Vec<(EndpointId, Connection)> =
            self.0.peers.lock().unwrap().iter().map(|(id, p)| (*id, p.conn.clone())).collect();
        for (peer, conn) in peers {
            if !ask_anyone && self.peer_access(&peer, doc).is_none() {
                continue;
            }
            // A part left by an earlier attempt may be bad; then start over once.
            for _ in 0..2 {
                match self.download(&conn, doc, id, size).await {
                    Ok(Got::Whole) => return true,
                    Ok(Got::BadPart) => continue,
                    Ok(Got::Nothing) => break,
                    Err(e) => {
                        debug!(peer = %peer.fmt_short(), "fetching file failed: {e:#}");
                        break;
                    }
                }
            }
        }
        false
    }

    /// For tests: ask every connected device for a file, whether or not it
    /// should have it and whether or not this device's documents list it.
    #[doc(hidden)]
    pub async fn fetch_for_test(&self, doc: &str, id: &str, size: u64) -> bool {
        self.fetch_file(doc, id, size, true).await
    }

    /// Download a file from one device, continuing where an earlier attempt
    /// stopped. The bytes are accepted only if they hash to the file's id.
    async fn download(&self, conn: &Connection, doc: &str, id: &str, size: u64) -> anyhow::Result<Got> {
        let dir = &self.0.files_dir;
        tokio::fs::create_dir_all(dir).await?;
        let part = part(dir, id);
        let mut file = tokio::fs::OpenOptions::new().create(true).append(true).open(&part).await?;
        let offset = file.metadata().await?.len();
        let expected = stored_len(size);

        let (mut send, mut recv) = conn.open_bi().await?;
        write_frame(&mut send, &FileReq { doc: doc.into(), id: id.into(), offset }).await?;
        send.finish()?;
        let found = matches!(read_frame(&mut recv).await?, FileResp::Found { size } if size == expected);
        if offset > expected {
            drop(file);
            tokio::fs::remove_file(&part).await?;
            return Ok(Got::BadPart);
        }
        if !found {
            return Ok(Got::Nothing);
        }
        let mut got = offset;
        let mut buf = vec![0u8; PIECE];
        while let Some(n) = recv.read(&mut buf).await? {
            got += n as u64;
            if got > expected {
                break;
            }
            file.write_all(&buf[..n]).await?;
        }
        file.flush().await?;
        drop(file);
        if got < expected {
            // Cut off; the part stays and the next attempt continues it.
            return Ok(Got::Nothing);
        }
        if got > expected || hash_file(&part).await? != id {
            tokio::fs::remove_file(&part).await?;
            return Ok(if offset > 0 { Got::BadPart } else { Got::Nothing });
        }
        tokio::fs::rename(&part, path(dir, id)).await?;
        let docs = self.0.store.lock().unwrap().set_file_here(id, true)?;
        if docs.is_empty() {
            // Removed from its container while it was on its way.
            remove_bytes(dir, id);
        }
        self.files_changed(docs);
        Ok(Got::Whole)
    }
}

/// Marks a file as being downloaded until dropped.
struct Fetching(Node, String);

impl Fetching {
    fn start(node: &Node, id: &str) -> Option<Self> {
        node.0.fetching.lock().unwrap().insert(id.to_string()).then(|| Fetching(node.clone(), id.to_string()))
    }
}

impl Drop for Fetching {
    fn drop(&mut self) {
        self.0.0.fetching.lock().unwrap().remove(&self.1);
    }
}

/// How an attempt to download a file ended.
enum Got {
    Whole,
    Nothing,
    /// What an earlier attempt left behind was unusable and has been removed.
    BadPart,
}

#[derive(Serialize, Deserialize)]
struct FileReq {
    doc: String,
    id: String,
    /// Where to start in the stored bytes (what the asker already has).
    offset: u64,
}

#[derive(Serialize, Deserialize)]
enum FileResp {
    Found { size: u64 },
    NotHere,
}

/// Answer file requests, which arrive on extra streams of a sync connection.
pub(crate) async fn serve(node: Node, remote: EndpointId, conn: Connection) {
    while let Ok((send, recv)) = conn.accept_bi().await {
        let node = node.clone();
        tokio::spawn(async move {
            if let Err(e) = answer(&node, remote, send, recv).await {
                debug!(peer = %remote.fmt_short(), "file request failed: {e:#}");
            }
        });
    }
}

async fn answer(node: &Node, remote: EndpointId, mut send: SendStream, mut recv: RecvStream) -> anyhow::Result<()> {
    let req: FileReq = read_frame(&mut recv).await?;
    // Whoever may have the document may have its files.
    let allowed = valid_id(&req.id)
        && node.peer_access(&remote, &req.doc).is_some()
        && node.0.store.lock().unwrap().file_here(&req.doc, &req.id)? == Some(true);
    let file = if allowed { File::open(path(&node.0.files_dir, &req.id)).await.ok() } else { None };
    let Some(mut file) = file else {
        write_frame(&mut send, &FileResp::NotHere).await?;
        send.finish()?;
        return Ok(());
    };
    let size = file.metadata().await?.len();
    write_frame(&mut send, &FileResp::Found { size }).await?;
    file.seek(SeekFrom::Start(req.offset.min(size))).await?;
    let mut buf = vec![0u8; PIECE];
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        send.write_all(&buf[..n]).await?;
    }
    send.finish()?;
    Ok(())
}

/// Keep fetching the files this device is missing, whenever a document
/// lists a new one or a device connects.
pub(crate) async fn fetch_loop(weak: Weak<Inner>) {
    let mut shutdown = match weak.upgrade() {
        Some(i) => i.shutdown.subscribe(),
        None => return,
    };
    loop {
        let Some(node) = Node::from_weak(&weak) else { return };
        let missing = node.0.store.lock().unwrap().missing_files().unwrap_or_default();
        for (doc, id, size) in missing {
            if *shutdown.borrow() {
                return;
            }
            // Only for containers this device keeps in full; other files
            // come when they are opened.
            if net::parse_doc_id(&doc).is_some_and(|(container, _)| node.wish(container) == Keep::All) {
                node.fetch_file(&doc, &id, size, false).await;
            }
        }
        let inner = node.0.clone();
        drop(node);
        tokio::select! {
            _ = inner.fetch.notified() => {}
            _ = tokio::time::sleep(Duration::from_secs(30)) => {}
            _ = shutdown.changed() => return,
        }
    }
}

/// `put` and friends must not write where files are listed.
pub(crate) fn check_collection(collection: &str) -> Result<()> {
    if collection.starts_with('~') {
        return Err(Error::Invalid("collection names starting with ~ are reserved".into()));
    }
    Ok(())
}
