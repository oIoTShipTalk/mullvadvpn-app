//! UTM backend for running VMs

use crate::config::{self, Config, VmConfig};
use anyhow::{bail, Context, Result};
use std::{
    net::{IpAddr, Ipv4Addr},
    process::Stdio,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

use super::VmInstance;

pub struct UtmInstance {
    pty_path: String,
    ip_addr: IpAddr,
    machine: String, // VM name as seen by utmctl
    utm_task: Option<tokio::task::JoinHandle<()>>,
}

impl Drop for UtmInstance {
    fn drop(&mut self) {
        // Kill VM on drop
        let machine = self.machine.clone();
        let script = r#"tell application "UTM"
    stop vm by force
    end tell"#;

        tokio::task::spawn(async move {
            run_osascript(&machine, script)
                .await
                .expect("Failed to run osascript when dropping UTM VM instance");
        });
    }
}

#[async_trait::async_trait]
impl VmInstance for UtmInstance {
    fn get_pty(&self) -> &str {
        &self.pty_path
    }

    fn get_ip(&self) -> &IpAddr {
        &self.ip_addr
    }

    async fn wait(&mut self) {
        if let Some(utm_task) = self.utm_task.take() {
            let _ = utm_task.await;
        }
    }
}

pub async fn run(config: &Config, vm_config: &VmConfig) -> Result<UtmInstance> {
    super::network::macos::setup_test_network()
        .await
        .context("Failed to set up networking")?;

    match config.runtime_opts.display {
        config::Display::None => (),
        config::Display::Local => (),
        config::Display::Vnc => {
            log::error!("VNC is not supported on UTM backend");
        }
    }

    if !vm_config.disks.is_empty() {
        log::error!("Mounting disks is not yet supported")
    }

    let machine = vm_config.image_path.clone();
    start_disposable_vm(&machine)
        .await
        .context("Start UTM VM")?;

    let status = wait_for_status_condition(&vm_config.image_path, "started")
        .await
        .context("Wait for VM to start")?;
    log::debug!("VM status: {status}");

    log::debug!("Obtain serial port");
    let pty_path = get_pty_path(&vm_config.image_path)
        .await
        .context("Could not obtain serial port")?;
    log::debug!("Serial port: {pty_path}");

    log::debug!("Waiting for IP address");

    let ip_addr = get_guest_ip(&vm_config.image_path)
        .await
        .context("Failed to obtain guest IP")?;
    log::debug!("Guest IP: {ip_addr}");

    let image_path = vm_config.image_path.to_owned();
    let utm_task = tokio::spawn(async move {
        let status = wait_for_status_condition(&image_path, "not started").await;
        match status {
            Ok(status) => log::info!("UTM VM stopped: {status}"),
            Err(error) => {
                log::error!("UTM VM failed to stop: {error}");
            }
        }
    });

    // The tunnel must be configured after the virtual machine is up, or macOS refuses to assign an
    // IP. The reasons for this are poorly understood.
    crate::vm::network::macos::configure_tunnel().await?;

    Ok(UtmInstance {
        pty_path,
        ip_addr,
        utm_task: Some(utm_task),
        machine,
    })
}

async fn start_disposable_vm(machine: &str) -> Result<()> {
    let _ = run_osascript(machine, "start vm without saving").await?;
    Ok(())
}

async fn wait_for_status_condition(machine: &str, status_str: &str) -> Result<String> {
    run_osascript(
        machine,
        &format!(
            "repeat
    if status of vm is {status_str} then exit repeat
    delay 1
end repeat
get status of vm",
        ),
    )
    .await
}

async fn get_guest_ip(machine: &str) -> Result<IpAddr> {
    // TODO: Don't hardcode this. QEMU guest tools currently don't support Windows Arm.
    // Once the issue has been fixed: https://github.com/utmapp/UTM/issues/5134
    /*
    use std::time::Duration;

    const OBTAIN_IP_TIMEOUT: Duration = Duration::from_secs(60);

    let ip_addr = tokio::time::timeout(
        OBTAIN_IP_TIMEOUT,
        run_osascript(machine, "get item 1 of (query ip of vm)"),
    )
    .await
    .context("Timed out waiting for IP address")?
    .context("Could not obtain IP address")?
    .parse()
    .context("Invalid guest IP address")
    */

    let _ = machine;
    const GUEST_IP: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 64, 2));
    Ok(GUEST_IP)
}

async fn get_pty_path(machine: &str) -> Result<String> {
    run_osascript(machine, "get address of first serial port of vm").await
}

async fn run_osascript(machine: &str, script: &str) -> Result<String> {
    let mut cmd = Command::new("/usr/bin/osascript");
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn().context("Failed to run osascript")?;
    let mut stdin = child.stdin.take().unwrap();

    stdin
        .write_all(
            format!(
                r#"tell application "UTM"
    set vm to virtual machine named "{machine}"
    {script}
end tell"#
            )
            .as_bytes(),
        )
        .await
        .context("Failed to write osascript")?;

    drop(stdin);

    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();

    let mut buffer = vec![];
    stdout
        .read_to_end(&mut buffer)
        .await
        .context("Failed to read stdout")?;

    let mut errbuffer = vec![];
    stderr
        .read_to_end(&mut errbuffer)
        .await
        .context("Failed to read stderr")?;

    let status = child.wait().await.context("Failed to wait on osascript")?;

    if !status.success() {
        bail!(
            "osascript failed: stderr: {}",
            String::from_utf8(errbuffer)?
        );
    }

    String::from_utf8(buffer).context("osascript returned invalid UTF-8")
}
