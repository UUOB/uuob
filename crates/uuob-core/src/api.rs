//! One entry point for everything: `node.call(client, method, params)`.
//!
//! The UUOB app (Tauri), connected apps (local API) and AI clients (MCP) all go
//! through this, so permissions are enforced in exactly one place.

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value as Json, json};

use crate::error::{Error, Result};
use crate::net::Access;
use crate::node::{Client, Node};

/// Methods any client may call (permissions still apply per container).
pub const DATA_METHODS: &[&str] = &[
    "put",
    "get",
    "delete",
    "list",
    "containers",
    "request_access",
    "create_container",
    "rename_container",
    "delete_container",
    "share",
    "files",
    "delete_file",
    "keep",
    "settings",
    "set_settings",
];

#[derive(Deserialize)]
struct Rec {
    container: Option<String>,
    collection: String,
    id: Option<String>,
    value: Option<Json>,
}

fn params<T: DeserializeOwned>(p: Json) -> Result<T> {
    serde_json::from_value(p).map_err(|e| Error::Invalid(e.to_string()))
}

fn ok<T: serde::Serialize>(v: T) -> Result<Json> {
    Ok(serde_json::to_value(v).map_err(anyhow::Error::from)?)
}

impl Node {
    pub async fn call(&self, client: &Client, method: &str, p: Json) -> Result<Json> {
        let p = if p.is_null() { json!({}) } else { p };
        if *client != Client::Shell && !DATA_METHODS.contains(&method) {
            return Err(Error::PermissionDenied(format!("{method} is only available to the UUOB app itself")));
        }
        // Apps default to their own container.
        let container_of = |r: &Rec| -> Result<String> {
            match (&r.container, client) {
                (Some(c), _) => Ok(c.clone()),
                (None, Client::App(a)) => Ok(a.clone()),
                (None, _) => Err(Error::Invalid("container is required".into())),
            }
        };
        match method {
            "put" => {
                let r: Rec = params(p)?;
                let c = container_of(&r)?;
                let value = r.value.ok_or_else(|| Error::Invalid("value is required".into()))?;
                self.put(client, &c, &r.collection, r.id.as_deref(), value)
            }
            "get" => {
                let r: Rec = params(p)?;
                let id = r.id.clone().ok_or_else(|| Error::Invalid("id is required".into()))?;
                ok(self.get(client, &container_of(&r)?, &r.collection, &id)?)
            }
            "delete" => {
                let r: Rec = params(p)?;
                let id = r.id.clone().ok_or_else(|| Error::Invalid("id is required".into()))?;
                ok(self.delete(client, &container_of(&r)?, &r.collection, &id)?)
            }
            "list" => {
                let r: Rec = params(p)?;
                ok(self.list(client, &container_of(&r)?, &r.collection)?)
            }
            "containers" => ok(self.containers(client)?),
            // Choices for all apps (time format, first day of the week). Any app
            // may read and change them: the user changes them in whichever app.
            "settings" => ok(self.read_net(crate::net::settings)),
            "set_settings" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct P { hour12: Option<bool>, sunday_first: Option<bool> }
                let P { hour12, sunday_first } = params(p)?;
                ok(self.write_net(|d| {
                    let mut s = crate::net::settings(d);
                    if let Some(v) = hour12 { s.hour12 = v; }
                    if let Some(v) = sunday_first { s.sunday_first = v; }
                    crate::net::set_settings(d, &s)?;
                    Ok(s)
                })?)
            }
            // How much of one of its containers this device keeps.
            "keep" => {
                #[derive(Deserialize)]
                struct P { container: String, keep: crate::net::Keep }
                let P { container, keep } = params(p)?;
                ok(self.set_keep(client, &container, keep)?)
            }
            // File bytes go over their own routes (see uuob-api); these list and remove.
            "files" | "delete_file" => {
                #[derive(Deserialize)]
                struct P { container: Option<String>, id: Option<String> }
                let P { container, id } = params(p)?;
                let container = match (container, client) {
                    (Some(c), _) => c,
                    (None, Client::App(a)) => a.clone(),
                    (None, _) => return Err(Error::Invalid("container is required".into())),
                };
                match (method, id) {
                    ("files", _) => ok(self.files(client, &container)?),
                    (_, Some(id)) => ok(self.delete_file(client, &container, &id)?),
                    (_, None) => Err(Error::Invalid("id is required".into())),
                }
            }
            "request_access" => {
                #[derive(Deserialize)]
                struct P { container: String, #[serde(default = "read")] access: Access }
                fn read() -> Access { Access::Read }
                let P { container, access } = params(p)?;
                ok(self.request_access(client, &container, access).await?)
            }
            "create_container" => {
                #[derive(Deserialize)]
                struct P { name: String, key: Option<String> }
                let P { name, key } = params(p)?;
                let Client::App(app) = client else {
                    return Err(Error::Invalid("only apps create containers".into()));
                };
                ok(self.create_container(app, &name, key.as_deref())?)
            }
            "rename_container" => {
                #[derive(Deserialize)]
                struct P { id: String, name: String }
                let P { id, name } = params(p)?;
                self.owned(client, &id)?;
                ok(self.rename_container(&id, &name)?)
            }
            "delete_container" => {
                #[derive(Deserialize)]
                struct P { id: String }
                let P { id } = params(p)?;
                // An app's first container goes only when the user deletes the app's data.
                if matches!(client, Client::App(app) if *app == id) {
                    return Err(Error::PermissionDenied("an app cannot delete its main container".into()));
                }
                if *client != Client::Shell || self.read_net(|d| crate::net::container(d, &id)).is_some_and(|c| !c.deleted) {
                    self.owned(client, &id)?;
                }
                ok(self.delete_container(&id)?)
            }
            "share" => {
                #[derive(Deserialize)]
                struct P { container: String }
                let P { container } = params(p)?;
                ok(self.request_share(client, &container).await?)
            }

            // ---- shell only ----
            "status" => ok(self.status()),
            "devices" => ok(self.devices()),
            "set_device_name" => {
                #[derive(Deserialize)]
                struct P { name: String }
                let P { name } = params(p)?;
                ok(self.set_device_name(&name)?)
            }
            "remove_device" => {
                #[derive(Deserialize)]
                struct P { id: String }
                let P { id } = params(p)?;
                ok(self.remove_device(&id)?)
            }
            "leave_network" => ok(self.reset_network()?),
            "set_keeper" => {
                #[derive(Deserialize)]
                struct P { device: String, on: bool }
                let P { device, on } = params(p)?;
                ok(self.set_keeper(&device, on)?)
            }
            "set_cache" => {
                #[derive(Deserialize)]
                struct P { device: String, bytes: u64 }
                let P { device, bytes } = params(p)?;
                ok(self.set_cache_limit(&device, bytes)?)
            }
            "set_copies" => {
                #[derive(Deserialize)]
                struct P { copies: u8 }
                let P { copies } = params(p)?;
                ok(self.set_copies(copies)?)
            }
            "create_invite" => ok(self.create_invite().await?),
            "cancel_invite" => {
                #[derive(Deserialize)]
                struct P { code: String }
                let P { code } = params(p)?;
                self.cancel_invite(&code).await;
                ok(())
            }
            "join" => {
                #[derive(Deserialize)]
                struct P { code: String }
                let P { code } = params(p)?;
                ok(self.join(&code).await?)
            }
            "apps" => ok(self.apps()),
            "asks" => ok(self.asks()),
            "answer" => {
                #[derive(Deserialize)]
                struct P { id: String, allow: bool }
                let P { id, allow } = params(p)?;
                ok(self.answer(&id, allow)?)
            }
            "create_app_code" => ok(self.create_app_code().await?),
            "uninstall_app" => {
                #[derive(Deserialize)]
                struct P { id: String, #[serde(default)] delete_data: bool }
                let P { id, delete_data } = params(p)?;
                ok(self.uninstall_app(&id, delete_data)?)
            }
            "export" => {
                #[derive(Deserialize)]
                struct P { container: String }
                let P { container } = params(p)?;
                self.export_container(client, &container)
            }
            "set_grant" => {
                #[derive(Deserialize)]
                struct P { client: String, app: String, access: Option<Access> }
                let P { client: c, app, access } = params(p)?;
                ok(self.set_grant(&c, &app, access)?)
            }
            "set_name" => {
                #[derive(Deserialize)]
                struct P { name: String }
                let P { name } = params(p)?;
                ok(self.set_name(&name)?)
            }
            "people" => ok(self.people()),
            "create_person_invite" => {
                #[derive(Deserialize)]
                struct P { container: Option<String>, access: Option<Access> }
                let P { container, access } = params(p)?;
                ok(self.create_person_invite(container.map(|c| (c, access.unwrap_or(Access::Read)))).await?)
            }
            "meet" => {
                #[derive(Deserialize)]
                struct P { code: String }
                let P { code } = params(p)?;
                ok(self.meet(&code).await?)
            }
            "remove_person" => {
                #[derive(Deserialize)]
                struct P { id: String }
                let P { id } = params(p)?;
                ok(self.remove_person(&id)?)
            }
            "share_with" => {
                #[derive(Deserialize)]
                struct P { container: String, person: String, access: Option<Access> }
                let P { container, person, access } = params(p)?;
                ok(self.share(&container, &person, access)?)
            }
            "accept_share" => {
                #[derive(Deserialize)]
                struct P { container: String }
                let P { container } = params(p)?;
                ok(self.accept_share(&container)?)
            }
            "clients" => ok(self.clients()),
            "create_client" => {
                #[derive(Deserialize)]
                struct P { name: String }
                let P { name } = params(p)?;
                let (id, token) = self.create_client(&name)?;
                ok(json!({ "id": id, "token": token }))
            }
            "delete_client" => {
                #[derive(Deserialize)]
                struct P { id: String }
                let P { id } = params(p)?;
                ok(self.delete_client(&id)?)
            }
            _ => Err(Error::Invalid(format!("unknown method {method}"))),
        }
    }
}
