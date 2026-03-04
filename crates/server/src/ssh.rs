//! Native Rust SSH client using russh.
//!
//! Replaces ssh/sshpass subprocess calls with pure Rust implementation.
//! Supports password and private key authentication, known_hosts verification.

use russh::client::{self, Config};
use russh::keys::key::PrivateKeyWithHashAlg;
use russh::keys::{known_hosts, load_secret_key};
use russh::ChannelMsg;
use std::sync::Arc;
use std::time::Duration;

/// Output from a remote SSH command.
#[derive(Debug, Clone)]
pub struct SshCommandOutput {
    pub stdout: String,
    pub stderr: String,
    #[allow(dead_code)]
    pub exit_status: u32,
}

/// SSH authentication material.
pub enum SshAuth {
    PrivateKey { key_content: String },
    Password { password: String },
}

/// Prepared context for SSH execution.
pub struct SshContext {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub known_hosts_content: String,
    pub auth: SshAuth,
    pub fallback_password: Option<String>,
}

/// Handler for russh client - verifies server key against known_hosts.
struct SshClientHandler {
    host: String,
    port: u16,
    known_hosts_path: std::path::PathBuf,
}

impl client::Handler for SshClientHandler {
    type Error = russh::Error;

    fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKey,
    ) -> impl std::future::Future<Output = Result<bool, Self::Error>> + Send {
        let path = self.known_hosts_path.clone();
        let host = self.host.clone();
        let port = self.port;
        async move {
            known_hosts::check_known_hosts_path(&host, port, server_public_key, path)
                .map_err(|_e| russh::Error::KeyChanged { line: 0 })
        }
    }
}

/// Run a command over SSH using russh.
pub async fn run_ssh_command(
    context: &SshContext,
    remote_command: &str,
    timeout_duration: Duration,
) -> Result<SshCommandOutput, String> {
    let result =
        run_ssh_command_inner(context, remote_command, timeout_duration, &context.auth).await;

    // Fallback: if private key failed with permission denied, retry with password
    if let Err(ref err) = result {
        if err.to_lowercase().contains("permission denied")
            || err.to_lowercase().contains("authentication failed")
        {
            if let Some(ref password) = context.fallback_password {
                let password_auth = SshAuth::Password {
                    password: password.clone(),
                };
                if let Ok(out) =
                    run_ssh_command_inner(context, remote_command, timeout_duration, &password_auth)
                        .await
                {
                    return Ok(out);
                }
            }
        }
    }

    result
}

async fn run_ssh_command_inner(
    context: &SshContext,
    remote_command: &str,
    timeout_duration: Duration,
    auth: &SshAuth,
) -> Result<SshCommandOutput, String> {
    let temp_dir = std::env::temp_dir().join(format!("acpms-ssh-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).map_err(|e| format!("Failed to create temp dir: {}", e))?;

    let known_hosts_path = temp_dir.join("known_hosts");
    std::fs::write(&known_hosts_path, &context.known_hosts_content)
        .map_err(|e| format!("Failed to write known_hosts: {}", e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&known_hosts_path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("Failed to set known_hosts permissions: {}", e))?;
    }

    let config = Config {
        inactivity_timeout: Some(Duration::from_secs(60)),
        ..Default::default()
    };
    let config = Arc::new(config);

    let handler = SshClientHandler {
        host: context.host.clone(),
        port: context.port,
        known_hosts_path: known_hosts_path.clone(),
    };

    let addrs = (context.host.as_str(), context.port);
    let mut session = client::connect(config, addrs, handler)
        .await
        .map_err(|e| format!("SSH connection failed: {}", e))?;

    // Authenticate
    match auth {
        SshAuth::PrivateKey { key_content } => {
            let key_path = temp_dir.join("id_deploy");
            std::fs::write(&key_path, key_content)
                .map_err(|e| format!("Failed to write key file: {}", e))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))
                    .map_err(|e| format!("Failed to set key permissions: {}", e))?;
            }

            let key_pair = load_secret_key(key_path, None)
                .map_err(|e| format!("Failed to load private key: {}", e))?;

            let hash_alg = session
                .best_supported_rsa_hash()
                .await
                .ok()
                .flatten()
                .flatten();
            let auth_res = session
                .authenticate_publickey(
                    &context.username,
                    PrivateKeyWithHashAlg::new(Arc::new(key_pair), hash_alg),
                )
                .await
                .map_err(|e| format!("Public key auth failed: {}", e))?;

            if !auth_res.success() {
                return Err("Authentication (publickey) failed".to_string());
            }
        }
        SshAuth::Password { password } => {
            let auth_res = session
                .authenticate_password(&context.username, password)
                .await
                .map_err(|e| format!("Password auth failed: {}", e))?;

            if !auth_res.success() {
                return Err("Authentication (password) failed".to_string());
            }
        }
    }

    // Open channel and exec
    let mut channel = session
        .channel_open_session()
        .await
        .map_err(|e| format!("Failed to open channel: {}", e))?;

    channel
        .exec(true, remote_command.as_bytes())
        .await
        .map_err(|e| format!("Failed to exec command: {}", e))?;

    let (mut stdout, mut stderr, mut exit_status) = (Vec::new(), Vec::new(), None);

    let run_future = async {
        while let Some(msg) = channel.wait().await {
            match msg {
                ChannelMsg::Data { data } => {
                    stdout.extend_from_slice(data.as_ref());
                }
                ChannelMsg::ExtendedData { data, ext } if ext == 1 => {
                    stderr.extend_from_slice(data.as_ref());
                }
                ChannelMsg::ExitStatus { exit_status: code } => {
                    exit_status = Some(code);
                }
                ChannelMsg::Close => break,
                ChannelMsg::Eof => {}
                _ => {}
            }
        }
        (stdout, stderr, exit_status)
    };

    let result = tokio::time::timeout(timeout_duration, run_future).await;

    let _ = session
        .disconnect(russh::Disconnect::ByApplication, "", "English")
        .await;

    let (stdout, stderr, exit_status) = result.map_err(|_| "SSH command timed out".to_string())?;

    let stdout_str = String::from_utf8_lossy(&stdout).to_string();
    let stderr_str = String::from_utf8_lossy(&stderr).to_string();

    let code = exit_status.unwrap_or(255);
    if code != 0 {
        let diagnostics = if !stderr_str.trim().is_empty() {
            sanitize_output(&stderr_str)
        } else {
            sanitize_output(&stdout_str)
        };
        return Err(format!(
            "SSH command failed (exit {}): {}",
            code, diagnostics
        ));
    }

    Ok(SshCommandOutput {
        stdout: stdout_str,
        stderr: stderr_str,
        exit_status: code,
    })
}

fn sanitize_output(output: &str) -> String {
    let normalized = output.replace(['\n', '\r'], " ");
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return "no output".to_string();
    }
    const MAX_LEN: usize = 240;
    if trimmed.len() > MAX_LEN {
        format!("{}...", &trimmed[..MAX_LEN])
    } else {
        trimmed.to_string()
    }
}
