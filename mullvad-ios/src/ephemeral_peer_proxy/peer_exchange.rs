use super::{ios_tcp_connection::*, EphemeralPeerParameters, PacketTunnelBridge};
use std::{ffi::CStr, ptr, sync::Mutex};
use talpid_tunnel_config_client::{request_ephemeral_peer_with, Error, RelayConfigService};
use talpid_types::net::wireguard::{PrivateKey, PublicKey};
use tokio::{runtime::Handle as TokioHandle, task::JoinHandle};
use tonic::transport::channel::Endpoint;
use tower::util::service_fn;

const GRPC_HOST_PTR: *const libc::c_char = {
    const BYTES: &[u8] = b"10.64.0.1:1337\0";
    BYTES.as_ptr().cast()
};

const GRPC_HOST_CSTR: &'static CStr = unsafe { CStr::from_ptr(GRPC_HOST_PTR) };

pub struct ExchangeCancelToken {
    inner: Mutex<CancelToken>,
}


impl ExchangeCancelToken {
    fn new(tokio_handle: TokioHandle, task: JoinHandle<()>) -> Self {
        let inner = CancelToken {
            tokio_handle,
            task: Some(task),
        };
        Self {
            inner: Mutex::new(inner),
        }
    }

    /// Blocks until the associated ephemeral peer exchange task is finished.
    pub fn cancel(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            if let Some(task) = inner.task.take() {
                task.abort();
                let _ = inner.tokio_handle.block_on(task);
            }
        }
    }
}

struct CancelToken {
    tokio_handle: TokioHandle,
    task: Option<JoinHandle<()>>,
}

pub struct EphemeralPeerExchange {
    pub_key: [u8; 32],
    ephemeral_key: [u8; 32],
    packet_tunnel: PacketTunnelBridge,
    peer_parameters: EphemeralPeerParameters,
}

impl EphemeralPeerExchange {
    pub fn new(
        pub_key: [u8; 32],
        ephemeral_key: [u8; 32],
        packet_tunnel: PacketTunnelBridge,
        peer_parameters: EphemeralPeerParameters,
    ) -> EphemeralPeerExchange {
        Self {
            pub_key,
            ephemeral_key,
            packet_tunnel,
            peer_parameters,
        }
    }

    pub fn run(self, tokio: TokioHandle) -> ExchangeCancelToken {
        let task = tokio.spawn(async move {
            self.run_service_inner().await;
        });

        ExchangeCancelToken::new(tokio, task)
    }

    /// Creates a `RelayConfigService` using the in-tunnel TCP Connection provided by the Packet
    /// Tunnel Provider
    async fn ios_tcp_client(tunnel_handle: i32, peer_parameters: EphemeralPeerParameters) -> Result<RelayConfigService, Error> {
        let endpoint = Endpoint::from_static("tcp://0.0.0.0:0");

        let tcp_provider = IosTcpProvider::new(tunnel_handle, peer_parameters);

        let conn = endpoint
            // it is assumend that the service function will only be called once.
            // Yet, by its signature, it is forced to be callable multiple times.
            // The tcp_provider appeases this constraint, maybe we should rewrite this back to
            // explicitly only allow a single invocation? It is due to this mismatch between how we
            // use it and what the interface expects that we are using a oneshot channel to
            // transfer the shutdown handle.
            .connect_with_connector(service_fn(move |_| {
                let provider = tcp_provider.clone();
                async move {
                    provider
                        .connect(&GRPC_HOST_CSTR)
                        .await
                        .map(hyper_util::rt::tokio::TokioIo::new)
                        .map_err(|_| Error::TcpConnectionOpen)
                }
            }))
            .await
            .map_err(Error::GrpcConnectError)?;

        Ok(RelayConfigService::new(conn))
    }

    fn report_failure(&self) {
        unsafe {
            swift_ephemeral_peer_ready(self.packet_tunnel.packet_tunnel, ptr::null(), ptr::null())
        };
    }

    async fn run_service_inner(self) {
        let async_provider = match Self::ios_tcp_client(
            self.packet_tunnel.tunnel_handle,
            self.peer_parameters,
        )
        .await
        {
            Ok(result) => result,
            Err(error) => {
                log::error!("Failed to create iOS TCP client: {error}");
                self.report_failure();
                return;
            }
        };
        // Use `self.ephemeral_key` as the new private key when no PQ but yes DAITA
        let ephemeral_pub_key = PrivateKey::from(self.ephemeral_key).public_key();

        tokio::select! {
            ephemeral_peer = request_ephemeral_peer_with(
                async_provider,
                PublicKey::from(self.pub_key),
                ephemeral_pub_key,
                self.peer_parameters.enable_post_quantum,
                self.peer_parameters.enable_daita,
            ) =>  {
                match ephemeral_peer {
                    Ok(peer) => {
                        match peer.psk {
                            Some(preshared_key) => unsafe {
                                let preshared_key_bytes = preshared_key.as_bytes();
                                swift_ephemeral_peer_ready(self.packet_tunnel.packet_tunnel,
                                    preshared_key_bytes.as_ptr(),
                                    self.ephemeral_key.as_ptr());
                            },
                            None => {
                                // Daita peer was requested, but without enabling post quantum keys
                                unsafe {
                                    swift_ephemeral_peer_ready(self.packet_tunnel.packet_tunnel,
                                        ptr::null(),
                                        self.ephemeral_key.as_ptr());
                                }
                            }
                        }
                    },
                    Err(error) => {
                        log::error!("Key exchange failed {}", error);
                        self.report_failure();
                    }
                }
            }

            _ = tokio::time::sleep(std::time::Duration::from_secs(self.peer_parameters.peer_exchange_timeout)) => {
                    self.report_failure();
            }
        }
    }
}
