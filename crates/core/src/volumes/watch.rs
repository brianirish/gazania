//! A stream of "something about drives or mounts changed" events from udisks2.
//! Consumers re-enumerate; the events carry no payload on purpose.

use crate::error::{Error, Result};
use crate::volumes::udisks::{IF_FS, ROOT};
use futures_lite::{Stream, StreamExt};
use std::collections::HashMap;
use zbus::zvariant::OwnedValue;
use zbus::{MatchRule, MessageStream};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    ObjectsAdded,
    ObjectsRemoved,
    MountPointsChanged,
}

pub async fn watch(conn: &zbus::Connection) -> Result<impl Stream<Item = Change>> {
    let rule = MatchRule::builder()
        .msg_type(zbus::message::Type::Signal)
        .path_namespace(ROOT)
        .map_err(|e| Error::Dbus(e.to_string()))?
        .build();
    let stream = MessageStream::for_match_rule(rule, conn, None)
        .await
        .map_err(|e| Error::Dbus(e.to_string()))?;

    Ok(stream.filter_map(|msg| {
        let msg = msg.ok()?;
        let header = msg.header();
        let interface = header.interface()?.as_str().to_string();
        let member = header.member()?.as_str().to_string();
        let (props_iface, changed_keys) = if member == "PropertiesChanged" {
            let (iface, changed, invalidated): (String, HashMap<String, OwnedValue>, Vec<String>) =
                msg.body().deserialize().ok()?;
            let mut keys: Vec<String> = changed.into_keys().collect();
            keys.extend(invalidated);
            (Some(iface), keys)
        } else {
            (None, Vec::new())
        };
        classify(&interface, &member, props_iface.as_deref(), &changed_keys)
    }))
}

pub fn classify(
    interface: &str,
    member: &str,
    props_interface: Option<&str>,
    changed_keys: &[String],
) -> Option<Change> {
    match (interface, member) {
        ("org.freedesktop.DBus.ObjectManager", "InterfacesAdded") => Some(Change::ObjectsAdded),
        ("org.freedesktop.DBus.ObjectManager", "InterfacesRemoved") => Some(Change::ObjectsRemoved),
        ("org.freedesktop.DBus.Properties", "PropertiesChanged")
            if props_interface == Some(IF_FS)
                && changed_keys.iter().any(|k| k == "MountPoints") =>
        {
            Some(Change::MountPointsChanged)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_manager_signals_map_to_added_and_removed() {
        assert_eq!(
            classify(
                "org.freedesktop.DBus.ObjectManager",
                "InterfacesAdded",
                None,
                &[]
            ),
            Some(Change::ObjectsAdded)
        );
        assert_eq!(
            classify(
                "org.freedesktop.DBus.ObjectManager",
                "InterfacesRemoved",
                None,
                &[]
            ),
            Some(Change::ObjectsRemoved)
        );
    }

    #[test]
    fn only_filesystem_mount_point_property_changes_count() {
        let keys = vec!["MountPoints".to_string()];
        assert_eq!(
            classify(
                "org.freedesktop.DBus.Properties",
                "PropertiesChanged",
                Some(IF_FS),
                &keys
            ),
            Some(Change::MountPointsChanged)
        );
        let other = vec!["SmartUpdated".to_string()];
        assert_eq!(
            classify(
                "org.freedesktop.DBus.Properties",
                "PropertiesChanged",
                Some("org.freedesktop.UDisks2.NVMe.Controller"),
                &other
            ),
            None
        );
        assert_eq!(
            classify(
                "org.freedesktop.DBus.Properties",
                "PropertiesChanged",
                Some(IF_FS),
                &other
            ),
            None
        );
    }

    #[test]
    fn unrelated_signals_are_ignored() {
        assert_eq!(
            classify("org.freedesktop.UDisks2.Job", "Completed", None, &[]),
            None
        );
    }
}
