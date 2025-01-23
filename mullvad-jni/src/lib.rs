#![cfg(target_os = "android")]

use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Once};

use jnix::jni::objects::{JClass, JObject};
use jnix::jni::JNIEnv;
use jnix::{FromJava, JnixEnv};
use tokio::runtime::Runtime;

use mullvad_daemon::runtime::new_multi_thread;
use talpid_types::android::AndroidContext;

mod api;
mod classes;
mod daemon;
mod problem_report;

const LOG_FILENAME: &str = "daemon.log";

/// Mullvad daemon instance. It must be initialized and destroyed by `MullvadDaemon.initialize` and
/// `MullvadDaemon.shutdown`, respectively.
static DAEMON_CONTEXT: Mutex<Option<(daemon::DaemonContext, Runtime)>> = Mutex::new(None);

static LOAD_CLASSES: Once = Once::new();

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to create global reference to Java object")]
    CreateGlobalReference(#[source] jnix::jni::errors::Error),

    #[error("Failed to get Java VM instance")]
    GetJvmInstance(#[source] jnix::jni::errors::Error),

    #[error(transparent)]
    Daemon(#[from] daemon::Error),

    #[error("Failed to init Tokio runtime")]
    InitTokio(#[source] io::Error),
}

/// Throw a Java exception and return if `result` is an error
macro_rules! ok_or_throw {
    ($env:expr, $result:expr) => {{
        match $result {
            Ok(val) => val,
            Err(err) => {
                let env = $env;
                env.throw(err.to_string())
                    .expect("Failed to throw exception");
                return;
            }
        }
    }};
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

    let env = JnixEnv::from(env);

    LOAD_CLASSES.call_once(|| env.preload_classes(classes::CLASSES.iter().cloned()));

    let runtime = ok_or_throw!(&env, new_multi_thread().build().map_err(Error::InitTokio));

    let rpc_socket = pathbuf_from_java(&env, rpc_socket_path);
    let files_dir = pathbuf_from_java(&env, files_directory);
    let cache_dir = pathbuf_from_java(&env, cache_directory);
    let api_endpoint = api::api_endpoint_from_java(&env, api_endpoint);

    let android_context = ok_or_throw!(&env, create_android_context(&env, vpn_service));

    let daemon = ok_or_throw!(
        &env,
        runtime.block_on(daemon::DaemonContext::start(
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

fn create_android_context(
    env: &JnixEnv<'_>,
    vpn_service: JObject<'_>,
) -> Result<AndroidContext, Error> {
    Ok(AndroidContext {
        jvm: Arc::new(env.get_java_vm().map_err(Error::GetJvmInstance)?),
        vpn_service: env
            .new_global_ref(vpn_service)
            .map_err(Error::CreateGlobalReference)?,
    })
}

fn pathbuf_from_java(env: &JnixEnv<'_>, path: JObject<'_>) -> PathBuf {
    PathBuf::from(String::from_java(env, path))
}
