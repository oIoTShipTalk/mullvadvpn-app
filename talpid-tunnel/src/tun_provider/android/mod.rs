mod ipnetwork_sub;

use self::ipnetwork_sub::IpNetworkSub;
use super::TunConfig;
use ipnetwork::IpNetwork;
use jnix::{
    jni::{
        objects::{GlobalRef, JValue},
        signature::{JavaType, Primitive},
        JavaVM,
    },
    FromJava, IntoJava, JnixEnv,
};
use std::{
    net::IpAddr,
    os::unix::io::{AsRawFd, RawFd},
    sync::Arc,
};
use talpid_types::net::{ALLOWED_LAN_MULTICAST_NETS, ALLOWED_LAN_NETS};
use talpid_types::{android::AndroidContext, ErrorExt};

/// Errors that occur while setting up VpnService tunnel.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to attach Java VM to tunnel thread")]
    AttachJvmToThread(#[source] jnix::jni::errors::Error),

    #[error("Failed to allow socket to bypass tunnel")]
    Bypass,

    #[error("Failed to call Java method TalpidVpnService.{0}")]
    CallMethod(&'static str, #[source] jnix::jni::errors::Error),

    #[error("Failed to create Java VM handle clone")]
    CloneJavaVm(#[source] jnix::jni::errors::Error),

    #[error("Failed to find TalpidVpnService.{0} method")]
    FindMethod(&'static str, #[source] jnix::jni::errors::Error),

    #[error("Attempt to configure the tunnel with an invalid DNS server address(es): {0:?}")]
    InvalidDnsServers(Vec<IpAddr>),

    #[error("Received an invalid result from TalpidVpnService.{0}: {1}")]
    InvalidMethodResult(&'static str, String),

    #[error("Failed to create tunnel device")]
    TunnelDeviceError,

    #[error("Routes timed out")]
    RoutesTimedOut,

    #[error("Profile for VPN has not been setup")]
    NotPrepared,

    #[error("Another legacy VPN profile is used as always on")]
    OtherLegacyAlwaysOnVpn,

    #[error("Another VPN app is used as always on")]
    OtherAlwaysOnApp { app_name: String },
}

/// Factory of tunnel devices on Android.
pub struct AndroidTunProvider {
    jvm: Arc<JavaVM>,
    class: GlobalRef,
    object: GlobalRef,
    config: TunConfig,
}

impl AndroidTunProvider {
    /// Create a new AndroidTunProvider interfacing with Android's VpnService.
    pub fn new(context: AndroidContext, config: TunConfig) -> Self {
        let env = JnixEnv::from(
            context
                .jvm
                .attach_current_thread_as_daemon()
                .expect("Failed to attach thread to Java VM"),
        );
        let talpid_vpn_service_class = env.get_class("net/mullvad/talpid/TalpidVpnService");

        AndroidTunProvider {
            jvm: context.jvm,
            class: talpid_vpn_service_class,
            object: context.vpn_service,
            config,
        }
    }

    /// Get the current tunnel config. Note that the tunnel must be recreated for any changes to
    /// take effect.
    pub fn config_mut(&mut self) -> &mut TunConfig {
        &mut self.config
    }

    /// Open a tunnel with the current configuration.
    pub fn open_tun(&mut self) -> Result<VpnServiceTun, Error> {
        self.open_tun_inner("openTun")
    }

    /// Open a tunnel with the current configuration.
    /// Force recreation even if the tunnel config hasn't changed.
    pub fn open_tun_forced(&mut self) -> Result<VpnServiceTun, Error> {
        self.open_tun_inner("openTunForced")
    }

    /// Open a tunnel with the current configuration.
    fn open_tun_inner(&mut self, get_tun_func_name: &'static str) -> Result<VpnServiceTun, Error> {
        let tun_fd = self.open_tun_fd(get_tun_func_name)?;

        let jvm = unsafe { JavaVM::from_raw(self.jvm.get_java_vm_pointer()) }
            .map_err(Error::CloneJavaVm)?;

        Ok(VpnServiceTun {
            tunnel: tun_fd,
            jvm,
            class: self.class.clone(),
            object: self.object.clone(),
        })
    }

    fn open_tun_fd(&self, get_tun_func_name: &'static str) -> Result<RawFd, Error> {
        let config = VpnServiceConfig::new(self.config.clone());

        let env = self.env()?;
        let java_config = config.into_java(&env);

        let result = self.call_method(
            get_tun_func_name,
            "(Lnet/mullvad/talpid/model/TunConfig;)Lnet/mullvad/talpid/model/CreateTunResult;",
            JavaType::Object("net/mullvad/talpid/model/CreateTunResult".to_owned()),
            &[JValue::Object(java_config.as_obj())],
        )?;

        match result {
            JValue::Object(result) => CreateTunResult::from_java(&env, result).into(),
            value => Err(Error::InvalidMethodResult(
                get_tun_func_name,
                format!("{:?}", value),
            )),
        }
    }

    /// Close currently active tunnel device.
    pub fn close_tun(&mut self) {
        let result = self.call_method("closeTun", "()V", JavaType::Primitive(Primitive::Void), &[]);

        let error = match result {
            Ok(JValue::Void) => None,
            Ok(value) => Some(Error::InvalidMethodResult(
                "closeTun",
                format!("{:?}", value),
            )),
            Err(error) => Some(error),
        };

        if let Some(error) = error {
            log::error!(
                "{}",
                error.display_chain_with_msg("Failed to close the tunnel")
            );
        }
    }

    /// Allow a socket to bypass the tunnel.
    pub fn bypass(&mut self, socket: RawFd) -> Result<(), Error> {
        let env = JnixEnv::from(
            self.jvm
                .attach_current_thread_as_daemon()
                .map_err(Error::AttachJvmToThread)?,
        );
        let create_tun_method = env
            .get_method_id(&self.class, "bypass", "(I)Z")
            .map_err(|cause| Error::FindMethod("bypass", cause))?;

        let result = env
            .call_method_unchecked(
                self.object.as_obj(),
                create_tun_method,
                JavaType::Primitive(Primitive::Boolean),
                &[JValue::Int(socket)],
            )
            .map_err(|cause| Error::CallMethod("bypass", cause))?;

        match result {
            JValue::Bool(0) => Err(Error::Bypass),
            JValue::Bool(_) => Ok(()),
            value => Err(Error::InvalidMethodResult("bypass", format!("{:?}", value))),
        }
    }

    fn call_method(
        &self,
        name: &'static str,
        signature: &str,
        return_type: JavaType,
        parameters: &[JValue<'_>],
    ) -> Result<JValue<'_>, Error> {
        let env = JnixEnv::from(
            self.jvm
                .attach_current_thread_as_daemon()
                .map_err(Error::AttachJvmToThread)?,
        );
        let method_id = env
            .get_method_id(&self.class, name, signature)
            .map_err(|cause| Error::FindMethod(name, cause))?;

        env.call_method_unchecked(self.object.as_obj(), method_id, return_type, parameters)
            .map_err(|cause| Error::CallMethod(name, cause))
    }

    fn env(&self) -> Result<JnixEnv<'_>, Error> {
        let jni_env = self
            .jvm
            .attach_current_thread_as_daemon()
            .map_err(Error::AttachJvmToThread)?;

        Ok(JnixEnv::from(jni_env))
    }
}

/// Configuration to use for VpnService
#[derive(Clone, Debug, Eq, PartialEq, IntoJava)]
#[jnix(class_name = "net.mullvad.talpid.model.TunConfig")]
struct VpnServiceConfig {
    /// IP addresses for the tunnel interface.
    pub addresses: Vec<IpAddr>,

    /// IP addresses for the DNS servers to use.
    pub dns_servers: Vec<IpAddr>,

    /// Routes to configure for the tunnel.
    pub routes: Vec<InetNetwork>,

    /// App packages that should be excluded from the tunnel.
    pub excluded_packages: Vec<String>,

    /// Maximum Transmission Unit in the tunnel.
    #[jnix(map = "|mtu| mtu as i32")]
    pub mtu: u16,
}

impl VpnServiceConfig {
    pub fn new(tun_config: TunConfig) -> VpnServiceConfig {
        let dns_servers = Self::resolve_dns_servers(&tun_config);
        let routes = Self::resolve_routes(&tun_config);

        VpnServiceConfig {
            addresses: tun_config.addresses,
            dns_servers,
            routes,
            excluded_packages: tun_config.excluded_packages,
            mtu: tun_config.mtu,
        }
    }

    /// Return a list of custom DNS servers. If not specified, gateway addresses are used for DNS.
    /// Note that `Some(vec![])` is different from `None`. `Some(vec![])` disables DNS.
    fn resolve_dns_servers(config: &TunConfig) -> Vec<IpAddr> {
        config
            .dns_servers
            .clone()
            .unwrap_or_else(|| config.gateways())
    }

    /// Potentially subtract LAN nets from the VPN service routes, excepting gateways.
    /// This prevents LAN traffic from going in the tunnel.
    fn resolve_routes(config: &TunConfig) -> Vec<InetNetwork> {
        if !config.allow_lan {
            return config
                .routes
                .iter()
                .cloned()
                .map(InetNetwork::from)
                .collect();
        }

        let required_ipv4_routes = vec![IpNetwork::from(IpAddr::from(config.ipv4_gateway))];
        let required_ipv6_routes = config
            .ipv6_gateway
            .map(|addr| IpNetwork::from(IpAddr::from(addr)))
            .into_iter()
            .collect::<Vec<IpNetwork>>();

        let (original_lan_ipv4_networks, original_lan_ipv6_networks) = Self::allowed_lan_networks()
            .cloned()
            .partition::<Vec<_>, _>(|network| network.is_ipv4());

        let lan_ipv4_networks = original_lan_ipv4_networks
            .into_iter()
            .flat_map(|network| network.sub_all(required_ipv4_routes.clone()))
            .collect::<Vec<_>>();

        let lan_ipv6_networks = original_lan_ipv6_networks
            .into_iter()
            .flat_map(|network| network.sub_all(required_ipv6_routes.clone()))
            .collect::<Vec<_>>();

        config
            .routes
            .iter()
            .flat_map(|&route| {
                if route.is_ipv4() {
                    route.sub_all(lan_ipv4_networks.clone())
                } else {
                    route.sub_all(lan_ipv6_networks.clone())
                }
            })
            .map(InetNetwork::from)
            .collect()
    }

    fn allowed_lan_networks() -> impl Iterator<Item = &'static IpNetwork> {
        ALLOWED_LAN_NETS
            .iter()
            .chain(ALLOWED_LAN_MULTICAST_NETS.iter())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, IntoJava)]
#[jnix(package = "net.mullvad.talpid.model")]
struct InetNetwork {
    address: IpAddr,
    prefix: i16,
}

impl From<IpNetwork> for InetNetwork {
    fn from(ip_network: IpNetwork) -> Self {
        InetNetwork {
            address: ip_network.ip(),
            prefix: ip_network.prefix() as i16,
        }
    }
}

/// Handle to a tunnel device on Android.
pub struct VpnServiceTun {
    tunnel: RawFd,
    jvm: JavaVM,
    class: GlobalRef,
    object: GlobalRef,
}

impl VpnServiceTun {
    /// Retrieve the tunnel interface name.
    pub fn interface_name(&self) -> &str {
        "tun"
    }

    /// Allow a socket to bypass the tunnel.
    pub fn bypass(&mut self, socket: RawFd) -> Result<(), Error> {
        let env = JnixEnv::from(
            self.jvm
                .attach_current_thread_as_daemon()
                .map_err(Error::AttachJvmToThread)?,
        );
        let create_tun_method = env
            .get_method_id(&self.class, "bypass", "(I)Z")
            .map_err(|cause| Error::FindMethod("bypass", cause))?;

        let result = env
            .call_method_unchecked(
                self.object.as_obj(),
                create_tun_method,
                JavaType::Primitive(Primitive::Boolean),
                &[JValue::Int(socket)],
            )
            .map_err(|cause| Error::CallMethod("bypass", cause))?;

        if !bool::from_java(&env, result) {
            return Err(Error::Bypass);
        }
        Ok(())
    }
}

impl AsRawFd for VpnServiceTun {
    fn as_raw_fd(&self) -> RawFd {
        self.tunnel
    }
}

#[derive(FromJava)]
#[jnix(package = "net.mullvad.talpid.model")]
enum CreateTunResult {
    Success { tun_fd: i32 },
    InvalidDnsServers { addresses: Vec<IpAddr> },
    EstablishError,
    OtherLegacyAlwaysOnVpn,
    OtherAlwaysOnApp { app_name: String },
    NotPrepared,
}

impl From<CreateTunResult> for Result<RawFd, Error> {
    fn from(result: CreateTunResult) -> Self {
        match result {
            CreateTunResult::Success { tun_fd } => Ok(tun_fd),
            CreateTunResult::InvalidDnsServers { addresses } => {
                Err(Error::InvalidDnsServers(addresses))
            }
            CreateTunResult::EstablishError => Err(Error::TunnelDeviceError),
            CreateTunResult::OtherLegacyAlwaysOnVpn => Err(Error::OtherLegacyAlwaysOnVpn),
            CreateTunResult::OtherAlwaysOnApp { app_name } => {
                Err(Error::OtherAlwaysOnApp { app_name })
            }
            CreateTunResult::NotPrepared => Err(Error::NotPrepared),
        }
    }
}
