use ipnetwork::Ipv6Network;
use mullvad_types::settings::{DnsOptions, DnsState};
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    sync::LazyLock,
};
use talpid_core::{dns::DnsConfig, firewall::is_local_address};

/// When we want to block certain contents with the help of DNS server side,
/// we compute the resolver IP to use based on these constants. The last
/// byte can be ORed together to combine multiple block lists.
const DNS_BLOCKING_IPV4_BASE: Ipv4Addr = Ipv4Addr::new(100, 64, 0, 0);
const DNS_BLOCKING_IPV6_BASE: Ipv6Addr = Ipv6Addr::new(0xfc00, 0x100, 0x64, 0, 0, 0, 0, 0);
const DNS_AD_BLOCKING_IP_BIT: u8 = 1 << 0; // 0b00000001
const DNS_TRACKER_BLOCKING_IP_BIT: u8 = 1 << 1; // 0b00000010
const DNS_MALWARE_BLOCKING_IP_BIT: u8 = 1 << 2; // 0b00000100
const DNS_ADULT_BLOCKING_IP_BIT: u8 = 1 << 3; // 0b00001000
const DNS_GAMBLING_BLOCKING_IP_BIT: u8 = 1 << 4; // 0b00010000
const DNS_SOCIAL_MEDIA_BLOCKING_IP_BIT: u8 = 1 << 5; // 0b00100000

static DNS_BLOCKING_IPV6_NET: LazyLock<Ipv6Network> =
    LazyLock::new(|| Ipv6Network::new(DNS_BLOCKING_IPV6_BASE, 64).unwrap());

/// Return the resolvers as a vector of `IpAddr`s. Returns `None` when no special resolvers
/// are requested and the tunnel default gateway should be used.
pub fn addresses_from_options(options: &DnsOptions) -> DnsConfig {
    match options.state {
        DnsState::Default => {
            // Check if we should use a custom blocking DNS resolver.
            // And if so, compute the IP.
            let mut last_byte: u8 = 0;

            if options.default_options.block_ads {
                last_byte |= DNS_AD_BLOCKING_IP_BIT;
            }
            if options.default_options.block_trackers {
                last_byte |= DNS_TRACKER_BLOCKING_IP_BIT;
            }
            if options.default_options.block_malware {
                last_byte |= DNS_MALWARE_BLOCKING_IP_BIT;
            }
            if options.default_options.block_adult_content {
                last_byte |= DNS_ADULT_BLOCKING_IP_BIT;
            }
            if options.default_options.block_gambling {
                last_byte |= DNS_GAMBLING_BLOCKING_IP_BIT;
            }
            if options.default_options.block_social_media {
                last_byte |= DNS_SOCIAL_MEDIA_BLOCKING_IP_BIT;
            }

            if last_byte != 0 {
                let mut dns4_ip = DNS_BLOCKING_IPV4_BASE.octets();
                dns4_ip[dns4_ip.len() - 1] |= last_byte;

                #[cfg(target_os = "macos")]
                let dns6_ip = {
                    let mut dns6_ip = DNS_BLOCKING_IPV6_BASE.octets();
                    dns6_ip[dns6_ip.len() - 1] |= last_byte;
                    dns6_ip
                };

                DnsConfig::Override {
                    tunnel_config: vec![
                        IpAddr::V4(Ipv4Addr::from(dns4_ip)),
                        #[cfg(target_os = "macos")]
                        IpAddr::V6(Ipv6Addr::from(dns6_ip)),
                    ],
                    non_tunnel_config: vec![],
                }
            } else {
                DnsConfig::Default
            }
        }
        DnsState::Custom if options.custom_options.addresses.is_empty() => DnsConfig::Default,
        DnsState::Custom => {
            let (tunnel_config, non_tunnel_config) = options
                .custom_options
                .addresses
                .iter()
                .partition(|&addr| is_tunnel_interface_config(addr));
            DnsConfig::Override {
                tunnel_config,
                non_tunnel_config,
            }
        }
    }
}

/// Return whether a given address should be configured on the tunnel interface or non-tunnel
/// interface.
fn is_tunnel_interface_config(address: &IpAddr) -> bool {
    // Private IP ranges should not be tunneled, unless it is a content blocker
    !is_local_address(address) || is_v6_content_blocker(address)
}

fn is_v6_content_blocker(address: &IpAddr) -> bool {
    match address {
        IpAddr::V4(_) => false,
        IpAddr::V6(addr) => DNS_BLOCKING_IPV6_NET.contains(*addr),
    }
}

#[cfg(test)]
mod test {
    use crate::dns::addresses_from_options;
    use mullvad_types::settings::{CustomDnsOptions, DefaultDnsOptions, DnsOptions, DnsState};
    use talpid_core::dns::DnsConfig;

    #[test]
    fn test_default_dns() {
        let public_cfg = DnsOptions {
            state: DnsState::Default,
            custom_options: CustomDnsOptions::default(),
            default_options: DefaultDnsOptions::default(),
        };

        assert_eq!(addresses_from_options(&public_cfg), DnsConfig::Default);
    }

    #[test]
    fn test_content_blockers() {
        let public_cfg = DnsOptions {
            state: DnsState::Default,
            custom_options: CustomDnsOptions::default(),
            default_options: DefaultDnsOptions {
                block_ads: true,
                ..DefaultDnsOptions::default()
            },
        };

        #[cfg(not(target_os = "macos"))]
        let tunnel_config = vec!["100.64.0.1".parse().unwrap()];

        #[cfg(target_os = "macos")]
        let tunnel_config = vec![
            "100.64.0.1".parse().unwrap(),
            "fc00:100:64::1".parse().unwrap(),
        ];

        assert_eq!(
            addresses_from_options(&public_cfg),
            DnsConfig::Override {
                tunnel_config,
                non_tunnel_config: vec![],
            }
        );
    }

    // Public IPs should be tunneled, private IPs should not be, except gateway?
    #[test]
    fn test_custom_dns() {
        let public_ip = "1.2.3.4".parse().unwrap();
        let private_ip = "172.16.10.1".parse().unwrap();
        let public_cfg = DnsOptions {
            state: DnsState::Custom,
            custom_options: CustomDnsOptions {
                addresses: vec![public_ip, private_ip],
            },
            default_options: DefaultDnsOptions::default(),
        };

        assert_eq!(
            addresses_from_options(&public_cfg),
            DnsConfig::Override {
                tunnel_config: vec![public_ip],
                non_tunnel_config: vec![private_ip],
            }
        );
    }
}
