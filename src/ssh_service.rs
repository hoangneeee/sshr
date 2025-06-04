use crate::models::SshHost;
use anyhow::{Context, Result};
use std::process::{Command, Stdio};

pub fn connect_to_host(host: &SshHost) -> Result<()> {
    let port_str = host.port.unwrap_or(22).to_string();
    let connection_str = format!("{}@{}", host.user, host.host);

    tracing::info!(
        "Attempting to connect: ssh {} -p {}",
        connection_str,
        port_str
    );

    let mut cmd = Command::new("ssh");
    let timeout = 60 * 5; // seconds
    cmd.arg(&connection_str).arg("-p").arg(&port_str).arg("-o").arg(format!("ConnectTimeout={}", timeout));

    // Allow ssh to inherit stdio to have an interactive session
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    // Execute the command
    // Important: we need to wait for the ssh command to finish.
    // `status()` will run the command and wait for it to complete.
    let status = cmd
        .status()
        .with_context(|| format!("Failed to execute SSH command for {}", host.alias))?;

    if !status.success() {
        // The user will see the error directly from ssh.
        // Returning the error here may not be necessary if ssh handles error display well.
        // However, for safety, we can log or return the error.
        tracing::error!("SSH command finished with a non-zero status: {}", status);
        // return Err(anyhow::anyhow!(
        //     "SSH command for {} failed with status: {}",
        //     host.alias,
        //     status
        // ));
    }

    Ok(())
}