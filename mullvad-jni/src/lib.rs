#![cfg(target_os = "android")]

use std::io;
use std::sync::Mutex;

use jnix::jni::objects::{JClass, JObject};
use jnix::jni::JNIEnv;
use tokio::runtime::Runtime;

use mullvad_daemon::runtime::new_multi_thread;

mod jvm;
mod mullvad;

const LOG_FILENAME: &str = "daemon.log";

/// Mullvad daemon instance. It must be initialized and destroyed by `MullvadDaemon.initialize` and
/// `MullvadDaemon.shutdown`, respectively.
static DAEMON_CONTEXT: Mutex<Option<(mullvad::daemon::DaemonContext, Runtime)>> = Mutex::new(None);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Jvm(#[from] jvm::Error),

    #[error(transparent)]
    Daemon(#[from] mullvad::daemon::Error),

    #[error("Failed to init Tokio runtime")]
    InitTokio(#[source] io::Error),
}

/// Spawn Mullvad daemon. There can only be a single instance, which must be shut down using
/// `MullvadDaemon.shutdown`. On success, nothing is returned. On error, an exception is thrown.
#[no_mangle]
pub extern "system" fn Java_net_mullvad_mullvadvpn_service_MullvadDaemon_initialize(
    env: JNIEnv<'_>,
    _class: JClass<'_>,
    vpn_service: JObject<'_>,
    rpc_socket_path: JObject<'_>,
    files_directory: JObject<'_>,
    cache_directory: JObject<'_>,
    api_endpoint: JObject<'_>,
) {
    let mut ctx = DAEMON_CONTEXT.lock().unwrap();
    assert!(ctx.is_none(), "multiple calls to MullvadDaemon.initialize");

    let jvm = jvm::Jvm::new(env);
    let runtime = jvm::ok_or_throw!(&env, new_multi_thread().build().map_err(Error::InitTokio));

    let rpc_socket = jvm.pathbuf_from_java(rpc_socket_path);
    let files_dir = jvm.pathbuf_from_java(files_directory);
    let cache_dir = jvm.pathbuf_from_java(cache_directory);
    let api_endpoint = jvm.api_endpoint_from_java(api_endpoint);

    let android_context = jvm::ok_or_throw!(&env, jvm.create_android_context(vpn_service));

    let daemon = jvm::ok_or_throw!(
        &env,
        runtime.block_on(mullvad::daemon::DaemonContext::start(
            android_context,
            rpc_socket,
            files_dir,
            cache_dir,
            api_endpoint,
        ))
    );

    *ctx = Some((daemon, runtime));
}

/// Shut down Mullvad daemon that was initialized using `MullvadDaemon.initialize`.
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_net_mullvad_mullvadvpn_service_MullvadDaemon_shutdown(
    _: JNIEnv<'_>,
    _class: JClass<'_>,
) {
    if let Some((daemon, runtime)) = DAEMON_CONTEXT.lock().unwrap().take() {
        // Dropping the tokio runtime will block if there are any tasks in flight.
        // That is, until all async tasks yield *and* all blocking threads have stopped.
        runtime.block_on(daemon.stop());
    }
}
