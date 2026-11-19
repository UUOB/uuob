//! The network document (`_net`): who is in the network, which apps and
//! containers exist, and who may access what. It syncs to every member device
//! like any other container, so a change made on one device (a grant, an
//! uninstall, a removed device) applies everywhere.

use std::collections::BTreeMap;

use automerge::{AutoCommit, ROOT, ReadDoc, transaction::Transactable};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value as Json;

use crate::doc;
use crate::error::Result;

pub const NET_DOC: &str = "_net";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkInfo {
    pub id: String,
    pub created_at: i64,
    /// The owner's name, as shown to people they share with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// How many storage devices must hold something before another device
    /// may let go of it (1 if unset). See `keep.rs`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copies: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Member {
    pub name: String,
    pub platform: String,
    pub added_at: i64,
    #[serde(default)]
    pub removed: bool,
    /// A storage device: it keeps everything, whatever apps ask.
    #[serde(default)]
    pub keeper: bool,
    /// How many bytes of files it may cache for containers of which it
    /// keeps only the list (`None`: the default, see `keep.rs`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<u64>,
}

/// How much of a container a device keeps.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Keep {
    /// Records and files (the default).
    All,
    /// Records; files come when they are opened and are cached.
    List,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContainerInfo {
    /// The app that owns this container.
    pub owner_app: String,
    pub name: String,
    pub created_at: i64,
    /// Bumped every time the container is deleted, so data from devices that
    /// were offline during the delete never resurrects into a new container.
    pub generation: i64,
    #[serde(default)]
    pub deleted: bool,
    /// Set when the container belongs to another person who shared it with
    /// us: their network id. Such a container counts as `deleted` (hidden,
    /// not stored) unless `inbox` is `Accepted`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// What we may do with a shared container (`None`: it is ours).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access: Option<Access>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inbox: Option<Inbox>,
}

/// Where a container shared with us stands.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Inbox {
    /// Waiting for the user to accept or decline.
    Offered,
    Accepted,
    /// We said no, or left later. The owner is told and stops sharing.
    Declined,
    /// The owner stopped sharing.
    Ended,
}

/// Someone else's UUOB network that we are linked with.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Person {
    pub name: String,
    pub added_at: i64,
    #[serde(default)]
    pub removed: bool,
}

/// A device of a linked person, keyed by device id.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PersonDevice {
    pub person: String,
    #[serde(default)]
    pub removed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppInfo {
    pub name: String,
    pub installed: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Access {
    Read,
    Write,
}

impl Access {
    pub fn allows(self, wanted: Access) -> bool {
        self >= wanted
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClientInfo {
    pub name: String,
    pub token_sha256: String,
    pub created_at: i64,
    /// Set for an app's connection: the client acts as that app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    /// Set for an app on a device without UUOB (see `remote.rs`): the app's
    /// own endpoint id. Its token works only over a connection from there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// For such an app: the name of the device it runs on, as the app gave it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    /// For an app or AI on a device with UUOB: that device's id, so the
    /// screens can show it there. Connections made before UUOB recorded it
    /// get it the first time their token is used (`Node::authenticate_from`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on: Option<String>,
}

/// Every entry is a flat root key `<kind>/<id>`, so devices that add entries
/// concurrently never create competing parent objects.
fn read_prefix<T: DeserializeOwned>(d: &AutoCommit, kind: &str) -> BTreeMap<String, T> {
    let prefix = format!("{kind}/");
    d.keys(&ROOT)
        .filter_map(|k| {
            let id = k.strip_prefix(&prefix)?.to_string();
            let obj = doc::get_map(d, &ROOT, &k)?;
            Some((id, serde_json::from_value(doc::to_json(d, &obj)).ok()?))
        })
        .collect()
}

fn write_entry<T: Serialize>(d: &mut AutoCommit, kind: &str, id: &str, value: &T) -> Result<()> {
    let json = serde_json::to_value(value).map_err(anyhow::Error::from)?;
    let Json::Object(map) = json else { unreachable!("structs serialize to objects") };
    doc::set_record(d, &ROOT, &format!("{kind}/{id}"), &map)
}

fn delete_key(d: &mut AutoCommit, key: &str) -> Result<()> {
    if d.get(&ROOT, key)?.is_some() {
        d.delete(&ROOT, key)?;
    }
    Ok(())
}

fn read_one<T: DeserializeOwned>(d: &AutoCommit, kind: &str, id: &str) -> Option<T> {
    let obj = doc::get_map(d, &ROOT, &format!("{kind}/{id}"))?;
    serde_json::from_value(doc::to_json(d, &obj)).ok()
}

fn scalar_str(d: &AutoCommit, key: &str) -> Option<String> {
    match d.get(&ROOT, key) {
        Ok(Some((automerge::Value::Scalar(v), _))) => v.as_str().map(str::to_string),
        _ => None,
    }
}

/// Write a scalar root key only when it differs (an identical write would
/// still count as a change and be synced).
fn put_str(d: &mut AutoCommit, key: &str, value: &str) -> Result<()> {
    if scalar_str(d, key).as_deref() != Some(value) {
        d.put(&ROOT, key, value)?;
    }
    Ok(())
}

pub fn network(d: &AutoCommit) -> Option<NetworkInfo> {
    let obj = doc::get_map(d, &ROOT, "network")?;
    serde_json::from_value(doc::to_json(d, &obj)).ok()
}

pub fn set_network(d: &mut AutoCommit, info: &NetworkInfo) -> Result<()> {
    let json = serde_json::to_value(info).map_err(anyhow::Error::from)?;
    let Json::Object(map) = json else { unreachable!() };
    doc::set_record(d, &ROOT, "network", &map)
}

/// Choices that apply to all of the user's apps on all of their devices.
/// Every field is optional in the document: a device on an older version
/// ignores this record, and a missing choice means the default.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    /// Times as "2:30 PM" instead of "14:30".
    #[serde(default)]
    pub hour12: bool,
    /// Weeks start on Sunday instead of Monday.
    #[serde(default)]
    pub sunday_first: bool,
}

pub fn settings(d: &AutoCommit) -> Settings {
    doc::get_map(d, &ROOT, "settings").and_then(|o| serde_json::from_value(doc::to_json(d, &o)).ok()).unwrap_or_default()
}

pub fn set_settings(d: &mut AutoCommit, s: &Settings) -> Result<()> {
    let json = serde_json::to_value(s).map_err(anyhow::Error::from)?;
    let Json::Object(map) = json else { unreachable!() };
    doc::set_record(d, &ROOT, "settings", &map)
}

pub fn members(d: &AutoCommit) -> BTreeMap<String, Member> {
    read_prefix(d, "member")
}

pub fn set_member(d: &mut AutoCommit, id: &str, m: &Member) -> Result<()> {
    write_entry(d, "member", id, m)
}

pub fn member(d: &AutoCommit, id: &str) -> Option<Member> {
    read_one(d, "member", id)
}

pub fn is_active_member(d: &AutoCommit, id: &str) -> bool {
    member(d, id).is_some_and(|m| !m.removed)
}

pub fn people(d: &AutoCommit) -> BTreeMap<String, Person> {
    read_prefix(d, "person")
}

pub fn person(d: &AutoCommit, id: &str) -> Option<Person> {
    read_one(d, "person", id)
}

pub fn set_person(d: &mut AutoCommit, id: &str, p: &Person) -> Result<()> {
    write_entry(d, "person", id, p)
}

pub fn person_devices(d: &AutoCommit) -> BTreeMap<String, PersonDevice> {
    read_prefix(d, "pdev")
}

pub fn person_device(d: &AutoCommit, device: &str) -> Option<PersonDevice> {
    read_one(d, "pdev", device)
}

pub fn set_person_device(d: &mut AutoCommit, device: &str, p: &PersonDevice) -> Result<()> {
    write_entry(d, "pdev", device, p)
}

/// What we share with whom: container → person → access. A share that was
/// stopped stays as `None`, so every device can tell the person it ended.
/// Stored as scalar root keys `share/<container>/<person>`.
pub fn shares(d: &AutoCommit) -> BTreeMap<String, BTreeMap<String, Option<Access>>> {
    let mut out: BTreeMap<String, BTreeMap<String, Option<Access>>> = BTreeMap::new();
    for k in d.keys(&ROOT) {
        let Some(rest) = k.strip_prefix("share/") else { continue };
        let Some((container, person)) = rest.rsplit_once('/') else { continue };
        let access = match scalar_str(d, &k).as_deref() {
            Some("read") => Some(Access::Read),
            Some("write") => Some(Access::Write),
            Some("none") => None,
            _ => continue,
        };
        out.entry(container.to_string()).or_default().insert(person.to_string(), access);
    }
    out
}

pub fn share(d: &AutoCommit, container: &str, person: &str) -> Option<Access> {
    match scalar_str(d, &format!("share/{container}/{person}")).as_deref() {
        Some("read") => Some(Access::Read),
        Some("write") => Some(Access::Write),
        _ => None,
    }
}

pub fn set_share(d: &mut AutoCommit, container: &str, person: &str, access: Option<Access>) -> Result<()> {
    let value = match access {
        Some(Access::Read) => "read",
        Some(Access::Write) => "write",
        None => "none",
    };
    put_str(d, &format!("share/{container}/{person}"), value)
}

/// What a device is asked to keep of a container: scalar root keys
/// `keep/<device>/<container>`, absent for "everything".
pub fn keep(d: &AutoCommit, device: &str, container: &str) -> Keep {
    match scalar_str(d, &format!("keep/{device}/{container}")).as_deref() {
        Some("list") => Keep::List,
        Some("none") => Keep::None,
        _ => Keep::All,
    }
}

pub fn set_keep(d: &mut AutoCommit, device: &str, container: &str, keep: Keep) -> Result<()> {
    let key = format!("keep/{device}/{container}");
    match keep {
        Keep::All => delete_key(d, &key),
        Keep::List => put_str(d, &key, "list"),
        Keep::None => put_str(d, &key, "none"),
    }
}

pub fn copies(d: &AutoCommit) -> usize {
    network(d).and_then(|n| n.copies).unwrap_or(1).max(1) as usize
}

pub fn containers(d: &AutoCommit) -> BTreeMap<String, ContainerInfo> {
    read_prefix(d, "container")
}

pub fn container(d: &AutoCommit, id: &str) -> Option<ContainerInfo> {
    read_one(d, "container", id)
}

pub fn set_container(d: &mut AutoCommit, id: &str, c: &ContainerInfo) -> Result<()> {
    write_entry(d, "container", id, c)
}

pub fn apps(d: &AutoCommit) -> BTreeMap<String, AppInfo> {
    read_prefix(d, "app")
}

pub fn set_app(d: &mut AutoCommit, id: &str, a: &AppInfo) -> Result<()> {
    write_entry(d, "app", id, a)
}

/// Questions for the user (see `connect.rs`), keyed by a random id.
pub fn asks<T: DeserializeOwned>(d: &AutoCommit) -> BTreeMap<String, T> {
    read_prefix(d, "ask")
}

pub fn set_ask<T: Serialize>(d: &mut AutoCommit, id: &str, ask: &T) -> Result<()> {
    write_entry(d, "ask", id, ask)
}

pub fn remove_ask(d: &mut AutoCommit, id: &str) -> Result<()> {
    delete_key(d, &format!("ask/{id}"))
}

pub fn clients(d: &AutoCommit) -> BTreeMap<String, ClientInfo> {
    read_prefix(d, "client")
}

pub fn set_client(d: &mut AutoCommit, id: &str, c: &ClientInfo) -> Result<()> {
    write_entry(d, "client", id, c)
}

/// Was the linked app with this endpoint id cut off?
pub fn unlinked(d: &AutoCommit, endpoint: &str) -> bool {
    scalar_str(d, &format!("unlinked/{endpoint}")).is_some()
}

pub fn remove_client(d: &mut AutoCommit, id: &str) -> Result<()> {
    // Remember a linked app that was cut off, so that every device can tell
    // it so (a device that never heard of it could not).
    if let Some(endpoint) = read_one::<ClientInfo>(d, "client", id).and_then(|c| c.endpoint) {
        put_str(d, &format!("unlinked/{endpoint}"), "1")?;
    }
    delete_key(d, &format!("client/{id}"))?;
    remove_grants_for_client(d, &format!("ai:{id}"))
}

/// All grants: client key (`app:<id>`, `ai:<id>`) → app → access. A grant
/// covers every container of that app.
/// Stored as scalar root keys `grant/<client>/<app>`.
pub fn grants(d: &AutoCommit) -> BTreeMap<String, BTreeMap<String, Access>> {
    let mut out: BTreeMap<String, BTreeMap<String, Access>> = BTreeMap::new();
    for k in d.keys(&ROOT) {
        let Some(rest) = k.strip_prefix("grant/") else { continue };
        let Some((client, container)) = rest.rsplit_once('/') else { continue };
        let access = match d.get(&ROOT, k.as_str()) {
            Ok(Some((automerge::Value::Scalar(v), _))) => match v.as_str() {
                Some("read") => Access::Read,
                Some("write") => Access::Write,
                _ => continue,
            },
            _ => continue,
        };
        out.entry(client.to_string()).or_default().insert(container.to_string(), access);
    }
    out
}

pub fn grant(d: &AutoCommit, client: &str, app: &str) -> Option<Access> {
    match scalar_str(d, &format!("grant/{client}/{app}")).as_deref() {
        Some("read") => Some(Access::Read),
        Some("write") => Some(Access::Write),
        _ => None,
    }
}

pub fn set_grant(d: &mut AutoCommit, client: &str, container: &str, access: Option<Access>) -> Result<()> {
    let key = format!("grant/{client}/{container}");
    match access {
        Some(Access::Read) => put_str(d, &key, "read"),
        Some(Access::Write) => put_str(d, &key, "write"),
        None => delete_key(d, &key),
    }
}

pub fn remove_grants_for_client(d: &mut AutoCommit, client: &str) -> Result<()> {
    let prefix = format!("grant/{client}/");
    let keys: Vec<String> = d.keys(&ROOT).filter(|k| k.starts_with(&prefix)).collect();
    for k in keys {
        d.delete(&ROOT, k.as_str())?;
    }
    Ok(())
}

/// Remove every grant that points at `app`'s data.
pub fn remove_grants_for_app(d: &mut AutoCommit, app: &str) -> Result<()> {
    let suffix = format!("/{app}");
    let keys: Vec<String> = d.keys(&ROOT).filter(|k| k.starts_with("grant/") && k.ends_with(&suffix)).collect();
    for k in keys {
        d.delete(&ROOT, k.as_str())?;
    }
    Ok(())
}

/// Storage/sync id for a container generation, e.g. `tasks~0`.
pub fn doc_id(container: &str, generation: i64) -> String {
    format!("{container}~{generation}")
}

pub fn parse_doc_id(id: &str) -> Option<(&str, i64)> {
    let (c, g) = id.rsplit_once('~')?;
    Some((c, g.parse().ok()?))
}
