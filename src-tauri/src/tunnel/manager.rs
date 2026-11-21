use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use russh::ChannelMsg;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex, OwnedMutexGuard};
use tokio::task::{JoinHandle, JoinSet};

use crate::ssh::client::{HeadlessConnection, SharedHandle};
use crate::ssh::forwarding::serve_local;
use crate::ssh::sshconf::forward::End;
use crate::state::{TunnelConfig, TunnelStatus, TunnelType};

#[derive(Debug, Error)]
pub enum TunnelError {
    #[error("Tunnel not found: {0}")]
    NotFound(String),
    #[error("This tunnel is already starting")]
    Starting,
    #[error("This tunnel is already running")]
    Running,
    #[error("Port already in use: {0}")]
    PortInUse(u16),
    #[error("IO error: {0}")]
    IoError(String),
}

struct Tunnel {
    config: TunnelConfig,
    // Held across authentication, which can wait for the user. Other tunnels
    // can still be listed or controlled, and cancellation releases the guard.
    starting: Arc<Mutex<()>>,
    active: Option<ActiveTunnel>,
}

struct ActiveTunnel {
    stop: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

impl Tunnel {
    fn running(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| !active.task.is_finished())
    }

    fn view(&self) -> TunnelConfig {
        let mut config = self.config.clone();
        config.status = if self.running() {
            TunnelStatus::Running
        } else if self.starting.try_lock().is_err() {
            TunnelStatus::Starting
        } else {
            TunnelStatus::Stopped
        };
        config
    }
}

/// Tunnel definitions and their independently owned SSH connections.
#[derive(Default)]
pub struct TunnelManager {
    tunnels: HashMap<String, Tunnel>,
}

impl TunnelManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create(
        &mut self,
        tunnel_type: TunnelType,
        local_port: u16,
        remote_host: String,
        remote_port: u16,
        session_id: String,
    ) -> TunnelConfig {
        let config = TunnelConfig {
            id: uuid::Uuid::new_v4().to_string(),
            tunnel_type,
            local_port,
            remote_host,
            remote_port,
            session_id,
            status: TunnelStatus::Stopped,
        };
        self.tunnels.insert(
            config.id.clone(),
            Tunnel {
                config: config.clone(),
                starting: Arc::new(Mutex::new(())),
                active: None,
            },
        );
        config
    }

    pub fn reserve_start(
        &self,
        id: &str,
    ) -> Result<(TunnelConfig, OwnedMutexGuard<()>), TunnelError> {
        let tunnel = self
            .tunnels
            .get(id)
            .ok_or_else(|| TunnelError::NotFound(id.to_owned()))?;
        let guard = tunnel
            .starting
            .clone()
            .try_lock_owned()
            .map_err(|_| TunnelError::Starting)?;
        if tunnel.running() {
            return Err(TunnelError::Running);
        }
        Ok((tunnel.config.clone(), guard))
    }

    pub async fn bind(port: u16) -> Result<TcpListener, TunnelError> {
        TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
            .await
            .map_err(|err| {
                if err.kind() == std::io::ErrorKind::AddrInUse {
                    TunnelError::PortInUse(port)
                } else {
                    TunnelError::IoError(format!("Failed to bind 127.0.0.1:{port}: {err}"))
                }
            })
    }

    pub(crate) fn start(
        &mut self,
        id: &str,
        listener: TcpListener,
        ssh: HeadlessConnection,
    ) -> Result<(), TunnelError> {
        let tunnel = self
            .tunnels
            .get_mut(id)
            .ok_or_else(|| TunnelError::NotFound(id.to_owned()))?;
        let target = match tunnel.config.tunnel_type {
            TunnelType::Local => Some(End::Tcp {
                host: Some(tunnel.config.remote_host.clone()),
                port: tunnel.config.remote_port,
            }),
            TunnelType::Dynamic => None,
        };
        let (stop, mut shutdown) = oneshot::channel();
        let task = tokio::spawn(async move {
            let mut relays = JoinSet::new();
            let mut check_connection = tokio::time::interval(Duration::from_secs(1));
            loop {
                tokio::select! {
                    _ = &mut shutdown => break,
                    _ = check_connection.tick() => {
                        if ssh.handle.try_lock().is_ok_and(|handle| handle.is_closed()) {
                            break;
                        }
                    }
                    Some(_) = relays.join_next(), if !relays.is_empty() => {}
                    result = listener.accept() => match result {
                        Ok((socket, peer)) => {
                            relays.spawn(serve_local(ssh.handle.clone(), socket, peer, target.clone(), None));
                        }
                        Err(err) => {
                            tracing::warn!("Tunnel listener: {err}");
                            tokio::time::sleep(Duration::from_millis(200)).await;
                        }
                    }
                }
            }
            drop(listener);
            relays.shutdown().await;
        });
        tunnel.active = Some(ActiveTunnel { stop, task });
        Ok(())
    }

    /// Drop the listener, its relays and its SSH connection before returning.
    pub async fn stop(&mut self, id: &str, remove: bool) -> Result<bool, TunnelError> {
        let tunnel = self
            .tunnels
            .get_mut(id)
            .ok_or_else(|| TunnelError::NotFound(id.to_owned()))?;
        if tunnel.starting.try_lock().is_err() {
            return Err(TunnelError::Starting);
        }
        let was_running = tunnel.running();
        if let Some(active) = tunnel.active.take() {
            let _ = active.stop.send(());
            let _ = active.task.await;
        }
        if remove {
            self.tunnels.remove(id);
        }
        Ok(was_running)
    }

    pub fn list(&self) -> Vec<TunnelConfig> {
        let mut configs: Vec<_> = self.tunnels.values().map(Tunnel::view).collect();
        configs.sort_unstable_by(|a, b| {
            a.local_port
                .cmp(&b.local_port)
                .then_with(|| a.id.cmp(&b.id))
        });
        configs
    }
}

/// Relay data bidirectionally between a local TCP stream and an SSH direct-tcpip channel.
///
/// Uses a single task with `tokio::select!` since russh::Channel is not Clone.
/// The TCP stream is split into read/write halves; the SSH channel is driven
/// from a single owner using select over both directions.
pub(crate) async fn relay_connection(
    tcp_stream: tokio::net::TcpStream,
    handle: &SharedHandle,
    remote_host: &str,
    remote_port: u16,
) -> Result<(), TunnelError> {
    // Open a direct-tcpip channel through the SSH connection
    let mut channel = {
        let guard = handle.lock().await;
        guard
            .channel_open_direct_tcpip(remote_host, remote_port as u32, "127.0.0.1", 0)
            .await
            .map_err(|e| {
                TunnelError::IoError(format!("Failed to open direct-tcpip channel: {}", e))
            })?
    };

    let (mut tcp_read, mut tcp_write) = tcp_stream.into_split();

    let mut buf = vec![0u8; 8192];
    let mut tcp_done = false;
    let mut ssh_done = false;

    // Relay loop: use select to handle both directions in a single task
    while !tcp_done || !ssh_done {
        tokio::select! {
            // Local TCP -> SSH channel (only if TCP is still readable)
            result = tcp_read.read(&mut buf), if !tcp_done => {
                match result {
                    Ok(0) => {
                        tcp_done = true;
                        let _ = channel.eof().await;
                    }
                    Ok(n) => {
                        if let Err(e) = channel.data(&buf[..n]).await {
                            tracing::debug!("Tunnel relay: write to SSH failed: {}", e);
                            tcp_done = true;
                        }
                    }
                    Err(e) => {
                        tracing::debug!("Tunnel relay: TCP read error: {}", e);
                        tcp_done = true;
                        let _ = channel.eof().await;
                    }
                }
            }
            // SSH channel -> local TCP (only if SSH channel is still open)
            msg = channel.wait(), if !ssh_done => {
                match msg {
                    Some(ChannelMsg::Data { ref data }) => {
                        if let Err(e) = tcp_write.write_all(data).await {
                            tracing::debug!("Tunnel relay: TCP write error: {}", e);
                            ssh_done = true;
                        }
                    }
                    Some(ChannelMsg::Eof) | None => {
                        ssh_done = true;
                        let _ = tcp_write.shutdown().await;
                    }
                    _ => {}
                }
            }
        }
    }

    tracing::debug!("Tunnel relay: connection closed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_pending_start_blocks_duplicate_starts_and_removal_until_released() {
        let mut manager = TunnelManager::new();
        let config = manager.create(
            TunnelType::Dynamic,
            1080,
            String::new(),
            0,
            "ssh-session".into(),
        );
        let (selected, attempt) = manager.reserve_start(&config.id).unwrap();
        assert_eq!(selected.session_id, "ssh-session");
        assert_eq!(manager.list()[0].status, TunnelStatus::Starting);
        assert!(matches!(
            manager.reserve_start(&config.id),
            Err(TunnelError::Starting)
        ));
        assert!(matches!(
            manager.stop(&config.id, true).await,
            Err(TunnelError::Starting)
        ));

        drop(attempt);
        assert_eq!(manager.list()[0].status, TunnelStatus::Stopped);
        assert!(manager.reserve_start(&config.id).is_ok());
        assert!(!manager.stop(&config.id, true).await.unwrap());
        assert!(manager.list().is_empty());
    }
}
