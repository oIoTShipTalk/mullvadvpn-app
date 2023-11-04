pub mod client;
pub mod types;

#[cfg(unix)]
use std::{env, os::unix::fs::PermissionsExt};
use std::{future::Future, io, path::Path};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
#[cfg(windows)]
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
#[cfg(unix)]
use tokio::{
    fs,
    net::{UnixListener, UnixStream},
};
#[cfg(unix)]
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::{server::Connected, Endpoint, Server, Uri};
use tower::service_fn;

pub use tonic::{async_trait, transport::Channel, Code, Request, Response, Status};

pub type ManagementServiceClient =
    types::management_service_client::ManagementServiceClient<Channel>;
pub use types::management_service_server::{ManagementService, ManagementServiceServer};

#[cfg(unix)]
use std::sync::LazyLock;
#[cfg(unix)]
static MULLVAD_MANAGEMENT_SOCKET_GROUP: LazyLock<Option<String>> =
    LazyLock::new(|| env::var("MULLVAD_MANAGEMENT_SOCKET_GROUP").ok());

pub const CUSTOM_LIST_LIST_NOT_FOUND_DETAILS: &[u8] = b"custom_list_list_not_found";
pub const CUSTOM_LIST_LIST_EXISTS_DETAILS: &[u8] = b"custom_list_list_exists";
pub const CUSTOM_LIST_LIST_NAME_TOO_LONG_DETAILS: &[u8] = b"custom_list_list_name_too_long";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Management RPC server or client error")]
    GrpcTransportError(#[source] tonic::transport::Error),

    #[error("Failed to open IPC pipe/socket")]
    StartServerError(#[source] io::Error),

    #[error("Failed to initialize pipe/socket security attributes")]
    SecurityAttributes(#[source] io::Error),

    #[error("Unable to set permissions for IPC endpoint")]
    PermissionsError(#[source] io::Error),

    #[cfg(unix)]
    #[error("Group not found")]
    NoGidError,

    #[cfg(unix)]
    #[error("Failed to obtain group ID")]
    ObtainGidError(#[source] nix::Error),

    #[cfg(unix)]
    #[error("Failed to set group ID")]
    SetGidError(#[source] nix::Error),

    #[error("gRPC call returned error")]
    Rpc(#[source] tonic::Status),

    #[error("Failed to parse gRPC response")]
    InvalidResponse(#[source] types::FromProtobufTypeError),

    #[error("Duration is too large")]
    DurationTooLarge,

    #[error("Unexpected non-UTF8 string")]
    PathMustBeUtf8,

    #[error("Missing daemon event")]
    MissingDaemonEvent,

    #[error("This voucher code is invalid")]
    InvalidVoucher,

    #[error("This voucher code has already been used")]
    UsedVoucher,

    #[error("There are too many devices on the account. One must be revoked to log in")]
    TooManyDevices,

    #[error("You are already logged in. Log out to create a new account")]
    AlreadyLoggedIn,

    #[error("The account does not exist")]
    InvalidAccount,

    #[error("There is no such device")]
    DeviceNotFound,

    #[error("Location data is unavailable")]
    NoLocationData,

    #[error("A custom list with that name already exists")]
    CustomListExists,

    #[error("A custom list with that name does not exist")]
    CustomListListNotFound,

    #[error("Location already exists in the custom list")]
    LocationExistsInCustomList,

    #[error("Location was not found in the custom list")]
    LocationNotFoundInCustomlist,

    #[error("Could not retrieve API access methods from settings")]
    ApiAccessMethodSettingsNotFound,

    #[error("An access method with that id does not exist")]
    ApiAccessMethodNotFound,
}

#[cfg(not(target_os = "android"))]
#[deprecated(note = "Prefer MullvadProxyClient")]
pub async fn new_rpc_client() -> Result<ManagementServiceClient, Error> {
    use futures::TryFutureExt;

    // The URI will be ignored
    Endpoint::from_static("lttp://[::]:50051")
        .connect_with_connector(service_fn(move |_: Uri| {
            UnixStream::connect(mullvad_paths::get_rpc_socket_path())
                .map_ok(hyper_util::rt::tokio::TokioIo::new)
        }))
        .await
        .map(ManagementServiceClient::new)
        .map_err(Error::GrpcTransportError)
}

#[cfg(not(target_os = "android"))]
pub use client::MullvadProxyClient;

pub type ServerJoinHandle = tokio::task::JoinHandle<()>;

pub async fn spawn_rpc_server<T: ManagementService, F: Future<Output = ()> + Send + 'static>(
    service: T,
    abort_rx: F,
    socket_path: impl AsRef<std::path::Path>,
) -> std::result::Result<ServerJoinHandle, Error> {
    let clients = server_transport(socket_path.as_ref()).await?;

    Ok(tokio::spawn(async move {
        if let Err(execution_error) = Server::builder()
            .add_service(ManagementServiceServer::new(service))
            .serve_with_incoming_shutdown(clients, abort_rx)
            .await
            .map_err(Error::GrpcTransportError)
        {
            log::error!("Management server panic: {execution_error}");
        }
        log::trace!("gRPC server is shutting down");
    }))
}

#[cfg(unix)]
async fn server_transport(socket_path: &Path) -> Result<UnixListenerStream, Error> {
    let clients =
        UnixListenerStream::new(UnixListener::bind(socket_path).map_err(Error::StartServerError)?);

    let mode = if let Some(group_name) = &*MULLVAD_MANAGEMENT_SOCKET_GROUP {
        let group = nix::unistd::Group::from_name(group_name)
            .map_err(Error::ObtainGidError)?
            .ok_or(Error::NoGidError)?;
        nix::unistd::chown(socket_path, None, Some(group.gid)).map_err(Error::SetGidError)?;
        0o760
    } else {
        0o766
    };
    fs::set_permissions(socket_path, PermissionsExt::from_mode(mode))
        .await
        .map_err(Error::PermissionsError)?;

    Ok(clients)
}

#[cfg(windows)]
async fn server_transport(socket_path: &Path) -> Result<NamedPipeServer, Error> {
    // FIXME: allow everyone access
    ServerOptions::new()
        .reject_remote_clients(true)
        .first_pipe_instance(true)
        .access_inbound(true)
        .access_outbound(true)
        .create(socket_path)
        .map_err(Error::StartServerError)
}
