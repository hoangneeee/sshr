use crate::models::SshHost;
use anyhow::{Context, Result};
use std::process::{Command, Stdio};

pub fn connect_to_host(host: &SshHost) -> Result<()> {
    let port_str = host.port.unwrap_or(22).to_string();
    let connection_str = format!("{}@{}", host.user, host.host);

    // Tạm thời ghi log, sẽ không thấy khi SSH chiếm màn hình
    tracing::info!(
        "Attempting to connect: ssh {} -p {}",
        connection_str,
        port_str
    );

    let mut cmd = Command::new("ssh");
    cmd.arg(&connection_str).arg("-p").arg(&port_str);

    // Cho phép ssh kế thừa stdio để có phiên tương tác
    cmd.stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    // Thực thi lệnh
    // Quan trọng: chúng ta cần đợi lệnh ssh kết thúc.
    // `status()` sẽ chạy lệnh và đợi nó hoàn thành.
    let status = cmd
        .status()
        .with_context(|| format!("Failed to execute SSH command for {}", host.alias))?;

    if !status.success() {
        // Người dùng sẽ thấy lỗi trực tiếp từ ssh.
        // Trả về lỗi ở đây có thể không cần thiết nếu ssh xử lý hiển thị lỗi tốt.
        // Tuy nhiên, để an toàn, chúng ta có thể log hoặc trả về lỗi.
        tracing::error!("SSH command finished with a non-zero status: {}", status);
        // return Err(anyhow::anyhow!(
        //     "SSH command for {} failed with status: {}",
        //     host.alias,
        //     status
        // ));
    }

    Ok(())
}