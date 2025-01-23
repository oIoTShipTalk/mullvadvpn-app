use std::path::{Path, PathBuf};

use tokio::task::JoinHandle;

use mullvad_api::ApiEndpoint;
use mullvad_daemon::{
    cleanup_old_rpc_socket, exception_logging, logging, version, Daemon, DaemonCommandChannel,
    DaemonCommandSender, DaemonConfig,
};
use talpid_types::{android::AndroidContext, ErrorExt};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to initialize logging: {0}")]
    InitializeLogging(String),

    #[error("Failed to initialize the mullvad daemon")]
    InitializeDaemon(#[source] mullvad_daemon::Error),
}

#[derive(Debug)]
pub struct DaemonContext {
    daemon_command_tx: DaemonCommandSender,
    running_daemon: JoinHandle<()>,
}

impl DaemonContext {
    pub async fn start(
        android_context: AndroidContext,
        rpc_socket: PathBuf,
        files_dir: PathBuf,
        cache_dir: PathBuf,
        api_endpoint: Option<ApiEndpoint>,
    ) -> Result<DaemonContext, Error> {
        start_logging(&files_dir).map_err(Error::InitializeLogging)?;
        version::log_version();

        #[cfg(not(feature = "api-override"))]
        if api_endpoint.is_some() {
            log::warn!("api_endpoint will be ignored since 'api-override' is not enabled");
        }

        let endpoint = api_endpoint.unwrap_or(ApiEndpoint::from_env_vars());
        let daemon_command_channel = DaemonCommandChannel::new();
        let daemon_command_tx = daemon_command_channel.sender();

        let daemon_config = DaemonConfig {
            rpc_socket_path: rpc_socket,
            log_dir: Some(files_dir.clone()),
            resource_dir: files_dir.clone(),
            settings_dir: files_dir,
            cache_dir,
            android_context,
            endpoint,
        };

        let running_daemon =
            Self::spawn_daemon_thread(daemon_config, daemon_command_channel).await?;

        Ok(DaemonContext {
            daemon_command_tx,
            running_daemon,
        })
    }

    pub async fn stop(self) {
        _ = self.daemon_command_tx.shutdown();
        _ = self.running_daemon.await;
    }

    async fn spawn_daemon_thread(
        daemon_config: DaemonConfig,
        daemon_command_channel: DaemonCommandChannel,
    ) -> Result<JoinHandle<()>, Error> {
        cleanup_old_rpc_socket(&daemon_config.rpc_socket_path).await;

        let daemon = Daemon::start(daemon_config, daemon_command_channel)
            .await
            .map_err(Error::InitializeDaemon)?;

        let running_daemon = tokio::spawn(async move {
            match daemon.run().await {
                Ok(()) => log::info!("Mullvad daemon has stopped"),
                Err(error) => log::error!(
                    "{}",
                    error.display_chain_with_msg("Mullvad daemon exited with an error")
                ),
            }
        });

        Ok(running_daemon)
    }
}

fn start_logging(log_dir: &Path) -> Result<(), String> {
    use std::sync::OnceLock;
    static LOGGER_RESULT: OnceLock<Result<(), String>> = OnceLock::new();
    LOGGER_RESULT
        .get_or_init(|| start_logging_inner(log_dir).map_err(|e| e.display_chain()))
        .to_owned()
}

fn start_logging_inner(log_dir: &Path) -> Result<(), logging::Error> {
    use crate::LOG_FILENAME;
    let log_file = log_dir.join(LOG_FILENAME);

    logging::init_logger(log::LevelFilter::Debug, Some(&log_file), true)?;
    exception_logging::enable();
    log_panics::init();

    Ok(())
}
