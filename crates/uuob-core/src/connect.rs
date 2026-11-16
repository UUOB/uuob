//! Apps are separate programs that connect to UUOB, normally on the same
//! device.
//!
//! An app connects either by asking (the user sees "Calendar wants to use
//! your network" in UUOB and taps Allow) or with a code the user copies from
//! UUOB (see `pairing.rs`; with a code, UUOB may be on another device, see
//! `remote.rs`). Either way it gets a token and its own container, nothing else.
//! Asking for more (e.g. Calendar reading Tasks) is another question to the user.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::crypto;
use crate::error::{Error, Result};
use crate::net::{self, Access, ClientInfo};
use crate::node::{AppManifest, Client, Node, validate_ident};
use crate::store::now_ms;

/// How long a question waits for the user before it counts as "no".
const ASK_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// Devices' clocks differ a little; a question is shown this much longer.
const CLOCK_SLACK_MS: i64 = 2 * 60 * 1000;

/// A question for the user, shown by UUOB.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Ask {
    /// An app wants to connect and get its own container.
    Connect { app: String, name: String },
    /// A connected app wants access to another app's data.
    Access { app: String, app_name: String, container: String, container_name: String, access: Access },
    /// The user tapped Share in an app: UUOB shows who it is shared with.
    Share { app: String, app_name: String, container: String, container_name: String },
}

#[derive(Clone, Debug, Serialize)]
pub struct PendingAsk {
    pub id: String,
    /// Asked on this device (and not on another one of the network).
    pub here: bool,
    #[serde(flatten)]
    pub ask: Ask,
}

/// A question as it sits in the network document (`ask/<id>`), so that
/// every device of the network shows it and any of them can answer: the
/// device that asks may have no screen, or the user may be at another one.
#[derive(Clone, Serialize, Deserialize)]
struct Asked {
    #[serde(flatten)]
    ask: Ask,
    /// The device that asks, and waits for the answer.
    device: String,
    at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    answer: Option<bool>,
}

/// Removes the question when the asker stops waiting (answered, timed out,
/// or the app that asked went away).
struct Pending<'a>(&'a Node, String);

impl Drop for Pending<'_> {
    fn drop(&mut self) {
        let _ = self.0.write_net(|d| net::remove_ask(d, &self.1));
    }
}

impl Node {
    /// Ask the user; `false` if they say no or don't answer in time.
    pub async fn ask(&self, ask: Ask) -> bool {
        let id = crypto::random_id(8);
        let mut events = self.subscribe();
        let asked = Asked { ask, device: self.0.me.to_string(), at: now_ms(), answer: None };
        if self.write_net(|d| net::set_ask(d, &id, &asked)).is_err() {
            return false;
        }
        let _pending = Pending(self, id.clone());
        let deadline = tokio::time::Instant::now() + ASK_TIMEOUT;
        loop {
            match self.read_net(|d| net::asks::<Asked>(d).remove(&id)) {
                Some(Asked { answer: Some(allow), .. }) => return allow,
                Some(_) => {}
                None => return false,
            }
            tokio::select! {
                _ = tokio::time::sleep_until(deadline) => return false,
                e = events.recv() => {
                    if matches!(e, Err(tokio::sync::broadcast::error::RecvError::Closed)) {
                        return false;
                    }
                }
            }
        }
    }

    /// Questions waiting for the user, oldest first: this device's and
    /// those asked on the network's other devices.
    pub fn asks(&self) -> Vec<PendingAsk> {
        let me = self.0.me.to_string();
        let fresh = now_ms() - ASK_TIMEOUT.as_millis() as i64 - CLOCK_SLACK_MS;
        let mut asks: Vec<(String, Asked)> =
            self.read_net(|d| net::asks::<Asked>(d)).into_iter().filter(|(_, a)| a.answer.is_none() && a.at > fresh).collect();
        asks.sort_by_key(|(_, a)| a.at);
        asks.into_iter().map(|(id, a)| PendingAsk { id, here: a.device == me, ask: a.ask }).collect()
    }

    pub fn answer(&self, id: &str, allow: bool) -> Result<()> {
        self.write_net(|d| {
            let mut asked = net::asks::<Asked>(d).remove(id).ok_or_else(|| Error::NotFound("question".into()))?;
            asked.answer = Some(allow);
            net::set_ask(d, id, &asked)
        })
    }

    /// Questions this device asked before it stopped (nobody waits for them
    /// any more), and old ones of devices that never came back for theirs.
    pub(crate) fn forget_old_asks(&self) -> Result<()> {
        let me = self.0.me.to_string();
        let old = now_ms() - 24 * 60 * 60 * 1000;
        let gone: Vec<String> =
            self.read_net(|d| net::asks::<Asked>(d)).into_iter().filter(|(_, a)| a.device == me || a.at < old).map(|(id, _)| id).collect();
        if gone.is_empty() {
            return Ok(());
        }
        self.write_net(|d| gone.iter().try_for_each(|id| net::remove_ask(d, id)))
    }

    /// Connect an app: with a code from UUOB, or by asking the user. Returns
    /// the app's token. The app gets its own container (created if needed).
    pub async fn connect_app(&self, m: &AppManifest, code: Option<&str>) -> Result<String> {
        validate_ident(&m.id)?;
        let name = m.name.trim();
        if name.is_empty() || name.len() > 64 {
            return Err(Error::Invalid("app name must be 1–64 characters".into()));
        }
        match code {
            Some(c) => self.take_app_code(c).await?,
            None => {
                if !self.ask(Ask::Connect { app: m.id.clone(), name: name.into() }).await {
                    return Err(Error::PermissionDenied("the connection was not allowed".into()));
                }
            }
        }
        self.grant_app(&AppManifest { id: m.id.clone(), name: name.into() }, None)
    }

    /// The user agreed: register the app (its own container is created if
    /// needed) and return a new token for it. `endpoint`: the app is on a
    /// device without UUOB and reaches us from this endpoint id only.
    pub(crate) fn grant_app(&self, m: &AppManifest, linked: Option<(String, String)>) -> Result<String> {
        validate_ident(&m.id)?;
        let name = m.name.trim();
        if name.is_empty() || name.len() > 64 {
            return Err(Error::Invalid("app name must be 1–64 characters".into()));
        }
        self.install_app(&AppManifest { id: m.id.clone(), name: name.into() })?;
        let token = format!("uuob_{}", crypto::random_id(32));
        // A linked app (see `remote.rs`): its endpoint id and its device's name.
        let (endpoint, device) = linked.unzip();
        let device = device.map(|d: String| match d.trim() {
            "" => "Another device".to_string(),
            d => d.chars().take(64).collect(),
        });
        let info = ClientInfo {
            name: name.into(),
            token_sha256: crypto::sha256_hex(token.as_bytes()),
            created_at: now_ms(),
            app: Some(m.id.clone()),
            endpoint,
            device,
        };
        self.write_net(|d| net::set_client(d, &crypto::random_id(8), &info))?;
        Ok(token)
    }

    /// An app asks for access to another app's data (`target` is that app's id).
    pub async fn request_access(&self, client: &Client, target: &str, access: Access) -> Result<bool> {
        let Client::App(app) = client else {
            return Err(Error::PermissionDenied("only apps can request access".into()));
        };
        let (app_name, target_name, granted) = self.read_net(|d| {
            let apps = net::apps(d);
            (apps.get(app).map(|a| a.name.clone()), apps.get(target).map(|a| a.name.clone()), net::grant(d, &client.key(), target))
        });
        let target_name = target_name.ok_or_else(|| Error::NotFound(format!("app {target}")))?;
        if app == target || granted.is_some_and(|g| g.allows(access)) {
            return Ok(true);
        }
        let ask = Ask::Access {
            app: app.clone(),
            app_name: app_name.unwrap_or_else(|| app.clone()),
            container: target.into(),
            container_name: target_name,
            access,
        };
        if !self.ask(ask).await {
            return Ok(false);
        }
        self.set_grant(&client.key(), target, Some(access))?;
        Ok(true)
    }

    /// An app opens UUOB's share sheet for one of its containers. The user
    /// picks people there; the app never sees who they are linked with.
    pub async fn request_share(&self, client: &Client, container: &str) -> Result<bool> {
        let Client::App(app) = client else {
            return Err(Error::PermissionDenied("only apps can ask to share".into()));
        };
        let c = self.owned(client, container)?;
        if c.from.is_some() || container == c.owner_app {
            return Err(Error::Invalid("this cannot be shared".into()));
        }
        let app_name = self.read_net(|d| net::apps(d).get(app).map(|a| a.name.clone())).unwrap_or_else(|| app.clone());
        Ok(self.ask(Ask::Share { app: app.clone(), app_name, container: container.into(), container_name: c.name }).await)
    }
}
