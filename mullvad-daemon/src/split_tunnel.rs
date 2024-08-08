//! Glue between the tunnel state machine split tunnel state and the Mullvad daemon.
//! On most platforms, the split tunnel settings should only be updated *after* they've been
//! successfully excluded in the tunnel state machine.

use crate::settings::SettingsManager;

pub struct SplitTunnelManager<'a> {
    // TODO: tunnel command channel
    // TODO: daemon msg
    settings: &'a SettingsManager,
}

impl SplitTunnelManager {
    // TODO: channel that sends ExcludedPathsEvent

    pub fn new(
        settings: &SettingsManager,
    ) -> (Self, Vec<SplitApp>) {
        let manager = Self {
            settings,
        };
        let initial_paths = manager.get_tunnel_paths();
        (manager, initial_paths)
    }

    fn get_tunnel_paths(&self) -> Vec<SplitApp> {
        if self.settings.split_tunnel.enable_exclusions {
            self.settings
                .split_tunnel
                .apps
                .iter()
                .cloned()
                .map(SplitApp::to_tunnel_command_repr)
                .collect()
        } else {
            vec![]
        }
    }

    pub async fn add_app(&self, ) {

    }
}
