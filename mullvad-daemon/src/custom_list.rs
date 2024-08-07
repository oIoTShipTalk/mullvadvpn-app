use crate::{Daemon, Error};
use mullvad_types::{
    constraints::Constraint,
    custom_list::{CustomList, Id},
    relay_constraints::{
        BridgeState, LocationConstraint, RelayConstraints, RelaySettings, ResolvedBridgeSettings,
    },
    settings::Settings,
    Intersection,
};
use talpid_types::net::TunnelType;

pub(crate) fn change_needs_reconnect(prev: &Settings, new: &Settings) -> bool {
    let mut need_to_reconnect = false;

    let RelaySettings::Normal(relay_settings) = &new.relay_settings else {
        // Ignore custom relay settings
        return false;
    };

    if let Constraint::Only(LocationConstraint::CustomList { list_id }) = &relay_settings.location {
        need_to_reconnect |= list_changed(prev, new, list_id);
    }

    if constraints_are_multihop_compatible(relay_settings) {
        if let Constraint::Only(LocationConstraint::CustomList { list_id }) =
            &relay_settings.wireguard_constraints.entry_location
        {
            need_to_reconnect |= list_changed(prev, new, list_id);
        }
    }

    if constraints_are_bridge_compatible(new) {
        if let Ok(ResolvedBridgeSettings::Normal(bridge_settings)) = new.bridge_settings.resolve() {
            if let Constraint::Only(LocationConstraint::CustomList { list_id }) =
                &bridge_settings.location
            {
                need_to_reconnect |= list_changed(prev, new, list_id);
            }
        }
    }

    need_to_reconnect
}

fn list_changed(prev: &Settings, new: &Settings, id: &Id) -> bool {
    prev.custom_lists.find_by_id(id) != new.custom_lists.find_by_id(id)
}

fn constraints_are_multihop_compatible(relay_settings: &RelayConstraints) -> bool {
    relay_settings.wireguard_constraints.multihop()
        && constraints_are_wireguard_compatible(relay_settings)
}

fn constraints_are_wireguard_compatible(relay_settings: &RelayConstraints) -> bool {
    relay_settings
        .tunnel_protocol
        .intersection(Constraint::Only(TunnelType::Wireguard))
        .is_some()
}

fn constraints_are_bridge_compatible(settings: &Settings) -> bool {
    let RelaySettings::Normal(relay_settings) = &settings.relay_settings else {
        // Ignore custom relay settings
        return false;
    };

    settings.bridge_state != BridgeState::Off && constraints_are_openvpn_compatible(relay_settings)
}

fn constraints_are_openvpn_compatible(relay_settings: &RelayConstraints) -> bool {
    relay_settings
        .tunnel_protocol
        .intersection(Constraint::Only(TunnelType::OpenVpn))
        .is_some()
}

impl Daemon {
    /// Create a new custom list.
    ///
    /// Returns an error if the name is not unique.
    pub async fn create_custom_list(&mut self, name: String) -> Result<Id, crate::Error> {
        let new_list = CustomList::new(name).map_err(crate::Error::CustomListError)?;
        let id = new_list.id;

        let _ = self
            .settings
            .try_update(|settings| settings.custom_lists.add(new_list))
            .await
            .map_err(Error::SettingsError)?;

        Ok(id)
    }

    /// Update a custom list.
    ///
    /// Returns an error if the list doesn't exist.
    pub async fn delete_custom_list(&mut self, id: Id) -> Result<(), Error> {
        let _ = self
            .settings
            .try_update(|settings| {
                // NOTE: Not using swap remove because it would make user output slightly
                // more confusing and the cost is so small.
                settings.custom_lists.remove(&id)
            })
            .await
            .map_err(Error::SettingsError);
        Ok(())
    }

    /// Update a custom list.
    ///
    /// Returns an error if...
    /// - there is no existing list with the same ID,
    /// - or the existing list has a different name.
    pub async fn update_custom_list(&mut self, new_list: CustomList) -> Result<(), Error> {
        let _ = self
            .settings
            .try_update(|settings| settings.custom_lists.update(new_list))
            .await
            .map_err(Error::SettingsError)?;
        Ok(())
    }

    /// Remove all custom lists.
    pub async fn clear_custom_lists(&mut self) -> Result<(), Error> {
        let _ = self
            .settings
            .update(|settings| {
                settings.custom_lists.clear();
            })
            .await
            .map_err(Error::SettingsError)?;
        Ok(())
    }
}
