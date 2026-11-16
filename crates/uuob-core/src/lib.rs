//! uuob-core: the user-owned backend that runs on each of your devices.
//!
//! - Every app gets its own **container** (one Automerge document), stored
//!   encrypted on disk and synced to your other devices.
//! - **Permissions** are default-deny: an app can only touch its own
//!   container; anything else (another app, an AI client) needs a grant.
//! - Apps are separate programs that **connect** to UUOB on the same device
//!   (the user allows it, or types a code) and get a token for the local API.
//!   An app on a device without UUOB reaches one on another device through
//!   an encrypted tunnel, after the user typed a code into it.
//! - Devices link with a short **code**; sync is direct and end-to-end
//!   encrypted between your devices (iroh/QUIC), relays only see ciphertext.
//! - You can link with other **people** (their own UUOB networks) and share
//!   single containers with them, view-only or editable.
//! - A container can also hold **files** of any size (photos, video), stored
//!   encrypted outside the documents and fetched between devices as needed.
//! - Each device **keeps** everything by default; an app can have its device
//!   keep less, and storage devices chosen by the user keep it all.

mod api;
mod connect;
mod crypto;
mod doc;
mod error;
mod files;
mod keep;
mod net;
mod node;
mod pairing;
mod people;
mod remote;
mod store;
mod sync;
mod wire;
mod words;

pub use api::DATA_METHODS;
pub use connect::{Ask, PendingAsk};
pub use crypto::{FileKeyStore, KeyStore};
pub use error::{Error, Result};
pub use files::{FileInfo, FileReader, FileWriter};
pub use iroh::address_lookup::MemoryLookup;
pub use net::{Access, AppInfo, ClientInfo, Keep, NetworkInfo};
pub use people::{PersonSummary, ShareRef};
pub use node::{
    AccessEntry, AppManifest, Client, Config, ContainerSummary, Device, Event, NetMode, Node, PersonRef, SharedWith, Status,
};
pub use pairing::{Invite, normalize_code};
pub use remote::{Request, Tunnel, device_name};
