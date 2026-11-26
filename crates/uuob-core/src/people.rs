//! Sharing with other people.
//!
//! A *person* is someone else's UUOB network (all of their devices). Linking
//! with a person shares nothing by itself; the owner of a container then
//! shares it with them, view-only or editable.
//!
//! A shared container syncs between the owner's devices and the person's
//! devices only, never between two people it is shared with. The owner's
//! devices are therefore the one place where access is enforced, and no keys
//! beyond the device-to-device encryption of every connection are needed.

use automerge::AutoCommit;
use iroh::EndpointId;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::net::{self, Access, AppInfo, ContainerInfo, Inbox, NET_DOC, Person, PersonDevice};
use crate::node::{Node, valid_name, validate_ident};
use crate::store::now_ms;

/// One shared container, as listed under a person.
#[derive(Clone, Debug, Serialize)]
pub struct ShareRef {
    pub container: String,
    pub name: String,
    pub app: String,
    pub app_name: String,
    pub access: Access,
    /// Shared with us and still waiting for our answer.
    pub offered: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct PersonSummary {
    pub id: String,
    pub name: String,
    pub added_at: i64,
    pub online: bool,
    /// What we share with them.
    pub sharing: Vec<ShareRef>,
    /// What they share with us.
    pub receiving: Vec<ShareRef>,
}

/// Who we are, exchanged once when two people link.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Card {
    pub network: String,
    pub name: String,
    pub devices: Vec<String>,
}

/// What each side tells the other when their devices connect, and again
/// whenever it changes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct About {
    pub name: String,
    /// Our devices, and whether each was removed.
    pub devices: Vec<(String, bool)>,
    /// What we share with you. A share that ended is listed with no access.
    pub shares: Vec<Shared>,
    /// Of what you share with us: the containers we accepted…
    pub accepted: Vec<String>,
    /// …and the ones we declined or left.
    pub declined: Vec<String>,
    /// We removed you: forget everything of ours.
    pub gone: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Shared {
    pub container: String,
    pub name: String,
    pub app: String,
    pub app_name: String,
    pub generation: i64,
    pub access: Option<Access>,
}

/// End everything between us and `person`, in both directions.
fn end_person(d: &mut AutoCommit, person: &str) -> Result<()> {
    if let Some(p) = net::person(d, person).filter(|p| !p.removed) {
        net::set_person(d, person, &Person { removed: true, ..p })?;
    }
    for (container, with) in net::shares(d) {
        if with.get(person).is_some_and(Option::is_some) {
            net::set_share(d, &container, person, None)?;
        }
    }
    for (id, c) in net::containers(d) {
        if c.from.as_deref() == Some(person) && c.inbox != Some(Inbox::Ended) {
            net::set_container(d, &id, &ContainerInfo { deleted: true, inbox: Some(Inbox::Ended), ..c })?;
        }
    }
    Ok(())
}

impl Node {
    /// The name other people see for you. Needed before linking with anyone.
    pub fn set_name(&self, name: &str) -> Result<()> {
        let name = valid_name(name)?;
        self.write_net(|d| {
            let n = net::network(d).expect("network");
            net::set_network(d, &net::NetworkInfo { name: Some(name), ..n })
        })
    }

    pub(crate) fn card(&self) -> Result<Card> {
        self.read_net(|d| {
            let n = net::network(d).expect("network");
            let name = n.name.ok_or_else(|| Error::Invalid("choose the name other people will see first".into()))?;
            let devices = net::members(d).into_iter().filter(|(_, m)| !m.removed).map(|(id, _)| id).collect();
            Ok(Card { network: n.id, name, devices })
        })
    }

    /// Record a person we just linked with (or link again with one we removed).
    pub(crate) fn add_person(&self, card: &Card) -> Result<()> {
        let name = valid_name(&card.name)?;
        self.write_net(|d| {
            if net::network(d).is_some_and(|n| n.id == card.network) {
                return Err(Error::Pairing("that code is from one of your own devices".into()));
            }
            let added_at = net::person(d, &card.network).map_or_else(now_ms, |p| p.added_at);
            net::set_person(d, &card.network, &Person { name, added_at, removed: false })?;
            // A device already known as someone else's stays theirs (as in `apply_about`).
            for device in &card.devices {
                let taken = net::person_device(d, device).is_some_and(|pd| pd.person != card.network);
                if net::member(d, device).is_none() && !taken {
                    net::set_person_device(d, device, &PersonDevice { person: card.network.clone(), removed: false })?;
                }
            }
            Ok(())
        })?;
        self.0.wake.notify_one();
        Ok(())
    }

    /// The people you are linked with and what is shared in each direction.
    pub fn people(&self) -> Vec<PersonSummary> {
        let (people, devices, containers, shares, apps) = self.read_net(|d| {
            (net::people(d), net::person_devices(d), net::containers(d), net::shares(d), net::apps(d))
        });
        let peers = self.0.peers.lock().unwrap();
        let app_name = |id: &str| apps.get(id).map_or_else(|| id.to_string(), |a| a.name.clone());
        let mut out: Vec<PersonSummary> = people
            .into_iter()
            .filter(|(_, p)| !p.removed)
            .map(|(id, p)| {
                let online = devices
                    .iter()
                    .any(|(dev, pd)| pd.person == id && dev.parse::<EndpointId>().is_ok_and(|e| peers.contains_key(&e)));
                let sharing = shares
                    .iter()
                    .filter_map(|(cid, with)| {
                        let access = (*with.get(&id)?)?;
                        let c = containers.get(cid).filter(|c| !c.deleted)?;
                        Some(ShareRef {
                            container: cid.clone(),
                            name: c.name.clone(),
                            app: c.owner_app.clone(),
                            app_name: app_name(&c.owner_app),
                            access,
                            offered: false,
                        })
                    })
                    .collect();
                let receiving = containers
                    .iter()
                    .filter(|(_, c)| c.from.as_deref() == Some(id.as_str()))
                    .filter(|(_, c)| matches!(c.inbox, Some(Inbox::Offered | Inbox::Accepted)))
                    .map(|(cid, c)| ShareRef {
                        container: cid.clone(),
                        name: c.name.clone(),
                        app: c.owner_app.clone(),
                        app_name: app_name(&c.owner_app),
                        access: c.access.unwrap_or(Access::Read),
                        offered: c.inbox == Some(Inbox::Offered),
                    })
                    .collect();
                PersonSummary { id, name: p.name, added_at: p.added_at, online, sharing, receiving }
            })
            .collect();
        out.sort_by_key(|p| p.added_at);
        out
    }

    /// Unlink a person. Everything shared in either direction ends.
    pub fn remove_person(&self, id: &str) -> Result<()> {
        self.write_net(|d| {
            net::person(d, id).ok_or_else(|| Error::NotFound(format!("person {id}")))?;
            end_person(d, id)
        })
    }

    /// Share one of your containers with a person (`None` stops sharing).
    /// Only containers an app created with [`Node::create_container`] can be
    /// shared; an app's first container stays private.
    pub fn share(&self, container: &str, person: &str, access: Option<Access>) -> Result<()> {
        self.write_net(|d| {
            let c = net::container(d, container)
                .filter(|c| !c.deleted)
                .ok_or_else(|| Error::NotFound(format!("container {container}")))?;
            if c.from.is_some() {
                return Err(Error::PermissionDenied("only its owner can share this".into()));
            }
            if container == c.owner_app {
                return Err(Error::Invalid("this data is private to the app and cannot be shared".into()));
            }
            if !net::person(d, person).is_some_and(|p| !p.removed) {
                return Err(Error::NotFound(format!("person {person}")));
            }
            net::set_share(d, container, person, access)
        })
    }

    /// Accept something a person offered to share with you.
    pub fn accept_share(&self, container: &str) -> Result<()> {
        self.write_net(|d| match net::container(d, container) {
            Some(c) if c.inbox == Some(Inbox::Offered) => {
                net::set_container(d, container, &ContainerInfo { deleted: false, inbox: Some(Inbox::Accepted), ..c })
            }
            Some(c) if c.inbox == Some(Inbox::Accepted) => Ok(()),
            _ => Err(Error::NotFound(format!("nothing is offered as {container}"))),
        })
    }

    // ------------------------------------------------------------------
    // Used by sync.rs
    // ------------------------------------------------------------------

    /// The person a device belongs to, if it is a linked person's device
    /// (whether or not it is still welcome).
    pub(crate) fn person_of(&self, device: &EndpointId) -> Option<String> {
        self.read_net(|d| net::person_device(d, &device.to_string())).map(|p| p.person)
    }

    /// Is this a device we should be connected to: one of ours, or one of a
    /// linked person's?
    pub(crate) fn is_active_peer(&self, peer: &EndpointId) -> bool {
        let id = peer.to_string();
        self.read_net(|d| {
            net::is_active_member(d, &id)
                || net::person_device(d, &id)
                    .is_some_and(|pd| !pd.removed && net::person(d, &pd.person).is_some_and(|p| !p.removed))
        })
    }

    /// Every device to keep a connection with.
    pub(crate) fn dial_targets(&self) -> Vec<EndpointId> {
        let me = self.0.me.to_string();
        self.read_net(|d| {
            let people = net::people(d);
            let ours = net::members(d).into_iter().filter(|(_, m)| !m.removed).map(|(id, _)| id);
            let theirs = net::person_devices(d)
                .into_iter()
                .filter(|(_, pd)| !pd.removed && people.get(&pd.person).is_some_and(|p| !p.removed))
                .map(|(id, _)| id);
            ours.chain(theirs).filter(|id| *id != me).filter_map(|id| id.parse().ok()).collect()
        })
    }

    /// What `peer` may do with our copy of `doc`: `Read` means it receives
    /// it, `Write` that we also take its changes, `None` that it gets nothing.
    pub(crate) fn peer_access(&self, peer: &EndpointId, doc: &str) -> Option<Access> {
        let id = peer.to_string();
        self.read_net(|d| {
            if let Some(m) = net::member(d, &id) {
                // Our own devices sync everything. A removed one still gets
                // the network document, which is how it learns it was removed.
                return if !m.removed {
                    Some(Access::Write)
                } else if doc == NET_DOC {
                    Some(Access::Read)
                } else {
                    None
                };
            }
            let pd = net::person_device(d, &id).filter(|pd| !pd.removed)?;
            net::person(d, &pd.person).filter(|p| !p.removed)?;
            let (container, generation) = net::parse_doc_id(doc)?;
            let c = net::container(d, container).filter(|c| !c.deleted && c.generation == generation)?;
            match &c.from {
                // Theirs, shared with us: the owner's word is final.
                Some(owner) if *owner == pd.person => Some(Access::Write),
                Some(_) => None,
                // Ours: whatever we granted that person.
                None => net::share(d, container, &pd.person),
            }
        })
    }

    /// Does `doc` hold a container that `person` shared with us?
    pub(crate) fn is_from(&self, doc: &str, person: &str) -> bool {
        net::parse_doc_id(doc)
            .and_then(|(c, _)| self.read_net(|d| net::container(d, c)))
            .is_some_and(|c| c.from.as_deref() == Some(person))
    }

    pub(crate) fn about(&self, person: &str) -> About {
        self.read_net(|d| {
            let gone = !net::person(d, person).is_some_and(|p| !p.removed);
            if gone {
                return About { gone, ..About::default() };
            }
            let containers = net::containers(d);
            let apps = net::apps(d);
            let shares = net::shares(d)
                .into_iter()
                .filter_map(|(cid, with)| {
                    let access = *with.get(person)?;
                    let c = containers.get(&cid)?;
                    Some(Shared {
                        name: c.name.clone(),
                        app_name: apps.get(&c.owner_app).map_or_else(|| c.owner_app.clone(), |a| a.name.clone()),
                        app: c.owner_app.clone(),
                        generation: c.generation,
                        access: access.filter(|_| !c.deleted),
                        container: cid,
                    })
                })
                .collect();
            let theirs = |state: fn(Option<Inbox>) -> bool| -> Vec<String> {
                containers
                    .iter()
                    .filter(|(_, c)| c.from.as_deref() == Some(person) && state(c.inbox))
                    .map(|(id, _)| id.clone())
                    .collect()
            };
            About {
                name: net::network(d).and_then(|n| n.name).unwrap_or_default(),
                devices: net::members(d).into_iter().map(|(id, m)| (id, m.removed)).collect(),
                shares,
                accepted: theirs(|s| s == Some(Inbox::Accepted)),
                declined: theirs(|s| s == Some(Inbox::Declined)),
                gone,
            }
        })
    }

    /// Take in what a person's device told us about them.
    pub(crate) fn apply_about(&self, person: &str, about: &About) -> Result<()> {
        self.write_net(|d| {
            if about.gone {
                return end_person(d, person);
            }
            let Some(p) = net::person(d, person).filter(|p| !p.removed) else { return Ok(()) };
            if let Ok(name) = valid_name(&about.name) {
                net::set_person(d, person, &Person { name, ..p })?;
            }

            // Their devices: new ones are added, and a removal is final.
            for (device, removed) in &about.devices {
                if device.parse::<EndpointId>().is_err() || net::member(d, device).is_some() {
                    continue;
                }
                match net::person_device(d, device) {
                    None => net::set_person_device(d, device, &PersonDevice { person: person.into(), removed: *removed })?,
                    Some(pd) if pd.person == person && *removed && !pd.removed => {
                        net::set_person_device(d, device, &PersonDevice { removed: true, ..pd })?
                    }
                    Some(_) => {}
                }
            }

            // What they share with us.
            for s in &about.shares {
                let existing = net::container(d, &s.container);
                let ours = existing.as_ref().is_some_and(|c| c.from.as_deref() != Some(person));
                let well_formed = validate_ident(&s.app).is_ok()
                    && validate_ident(&s.container).is_ok()
                    && s.container.strip_prefix(&s.app).is_some_and(|rest| rest.len() > 1 && rest.starts_with('.'));
                let Ok(name) = valid_name(&s.name) else { continue };
                if ours || !well_formed {
                    continue;
                }
                let state = existing.as_ref().and_then(|c| c.inbox);
                let next = match (s.access, state) {
                    (None, None) => continue,
                    (None, _) => Inbox::Ended,
                    (Some(_), None | Some(Inbox::Ended)) => Inbox::Offered,
                    (Some(_), Some(state)) => state,
                };
                if !net::apps(d).contains_key(&s.app) {
                    let app_name = valid_name(&s.app_name).unwrap_or_else(|_| s.app.clone());
                    net::set_app(d, &s.app, &AppInfo { name: app_name, installed: false })?;
                }
                net::set_container(
                    d,
                    &s.container,
                    &ContainerInfo {
                        owner_app: s.app.clone(),
                        name,
                        created_at: existing.as_ref().map_or_else(now_ms, |c| c.created_at),
                        generation: s.generation,
                        deleted: next != Inbox::Accepted,
                        from: Some(person.into()),
                        access: s.access.or(existing.and_then(|c| c.access)),
                        inbox: Some(next),
                    },
                )?;
            }

            // What they no longer want from us.
            for container in &about.declined {
                if net::share(d, container, person).is_some() {
                    net::set_share(d, container, person, None)?;
                }
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::AppManifest;
    use crate::{Config, MemoryLookup, NetMode};

    /// A linked person's devices tell us what they share. A hostile one must
    /// not be able to take over our own containers or our own devices by
    /// saying so.
    #[tokio::test]
    async fn a_person_cannot_claim_what_is_ours() {
        let dir = tempfile::TempDir::new().unwrap();
        let config = Config {
            data_dir: dir.path().into(),
            device_name: "Mac".into(),
            platform: "macos".into(),
            net: NetMode::Local(MemoryLookup::new()),
        };
        let node = Node::open(config).await.unwrap();
        node.install_app(&AppManifest { id: "tasks".into(), name: "Tasks".into() }).unwrap();
        let ours = node.create_container("tasks", "Groceries", Some("abcdefghijklmnop")).unwrap();
        let me = node.status().device_id;
        node.add_person(&Card { network: "mallory-net".into(), name: "Mallory".into(), devices: vec![] }).unwrap();

        let claim = |container: &str, app: &str| Shared {
            container: container.into(),
            name: "Mine now".into(),
            app: app.into(),
            app_name: "Tasks".into(),
            generation: 7,
            access: Some(Access::Write),
        };
        let about = About {
            name: "Mallory".into(),
            devices: vec![(me.clone(), false)],
            shares: vec![claim(&ours, "tasks"), claim("tasks", "tasks"), claim("photos.abcdef", "tasks")],
            ..About::default()
        };
        node.apply_about("mallory-net", &about).unwrap();

        node.read_net(|d| {
            let c = net::container(d, &ours).unwrap();
            assert_eq!((c.from, c.name.as_str(), c.generation), (None, "Groceries", 0), "our list stays ours");
            assert!(net::container(d, "tasks").unwrap().from.is_none(), "an app's own container stays ours");
            assert!(net::container(d, "photos.abcdef").is_none(), "a container not of the app it names is ignored");
            assert!(net::person_device(d, &me).is_none(), "our own device stays ours");
            assert!(net::member(d, &me).is_some_and(|m| !m.removed));
        });
        assert_eq!(node.peer_access(&node.id(), &net::doc_id(&ours, 0)), Some(Access::Write), "this device, not Mallory's");

        // Nor another person's devices, neither when linking nor later.
        node.add_person(&Card { network: "alice-net".into(), name: "Alice".into(), devices: vec!["alices-phone".into()] }).unwrap();
        node.add_person(&Card { network: "mallory-net".into(), name: "Mallory".into(), devices: vec!["alices-phone".into()] }).unwrap();
        let about = About { name: "Mallory".into(), devices: vec![("alices-phone".into(), false)], ..About::default() };
        node.apply_about("mallory-net", &about).unwrap();
        node.read_net(|d| assert_eq!(net::person_device(d, "alices-phone").unwrap().person, "alice-net", "Alice's phone stays hers"));
    }
}
