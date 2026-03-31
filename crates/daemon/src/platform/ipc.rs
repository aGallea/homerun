/// Platform-aware IPC primitives for the daemon server.
///
/// On Unix, the daemon listens on a Unix domain socket (`daemon.sock`).
/// On Windows, it listens on a Windows named pipe (`\\.\pipe\homerun-daemon`).
#[cfg(windows)]
pub const PIPE_NAME: &str = r"\\.\pipe\homerun-daemon";

/// Check whether a daemon is already reachable on the platform IPC endpoint.
#[cfg(windows)]
pub async fn is_daemon_reachable(pipe_name: &str) -> bool {
    tokio::net::windows::named_pipe::ClientOptions::new()
        .open(pipe_name)
        .is_ok()
}

#[cfg(unix)]
pub async fn is_daemon_reachable(socket_path: &std::path::Path) -> bool {
    tokio::net::UnixStream::connect(socket_path).await.is_ok()
}

// ---------------------------------------------------------------------------
// Windows: NamedPipeListener that implements axum's `Listener` trait
// ---------------------------------------------------------------------------

#[cfg(windows)]
pub mod named_pipe {
    use std::io;
    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

    /// A listener that accepts connections on a Windows named pipe.
    ///
    /// Each call to [`accept_client`] waits for a client to connect, then creates a
    /// fresh pipe instance so the next client can connect immediately.
    pub struct NamedPipeListener {
        pipe_name: String,
        current: NamedPipeServer,
    }

    impl NamedPipeListener {
        /// Create a new listener bound to `pipe_name`.
        ///
        /// `first_pipe_instance(true)` ensures we fail fast if another daemon
        /// already owns this pipe.
        pub fn bind(pipe_name: &str) -> io::Result<Self> {
            let server = ServerOptions::new()
                .first_pipe_instance(true)
                .create(pipe_name)?;
            Ok(Self {
                pipe_name: pipe_name.to_string(),
                current: server,
            })
        }

        /// Wait for a client to connect, then return the connected pipe.
        async fn accept_client(&mut self) -> io::Result<NamedPipeServer> {
            // Wait for a client on the current instance.
            self.current.connect().await?;
            // Create a replacement instance for the next caller.
            let new_server = ServerOptions::new().create(&self.pipe_name)?;
            let connected = std::mem::replace(&mut self.current, new_server);
            Ok(connected)
        }

        /// Return the pipe name this listener is bound to.
        pub fn pipe_name(&self) -> &str {
            &self.pipe_name
        }
    }

    // -- Implement axum 0.8 `Listener` so we can pass this to `axum::serve` --

    impl axum::serve::Listener for NamedPipeListener {
        type Io = NamedPipeServer;
        type Addr = String;

        async fn accept(&mut self) -> (Self::Io, Self::Addr) {
            // axum's Listener::accept must not return errors; retry on
            // transient failures (back off briefly to avoid busy-loop).
            loop {
                match self.accept_client().await {
                    Ok(server) => return (server, self.pipe_name.clone()),
                    Err(e) => {
                        tracing::error!("Named-pipe accept error: {e}");
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                }
            }
        }

        fn local_addr(&self) -> io::Result<Self::Addr> {
            Ok(self.pipe_name.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    #[test]
    fn test_pipe_name_format() {
        assert!(super::PIPE_NAME.starts_with(r"\\.\pipe\"));
        assert!(super::PIPE_NAME.len() > r"\\.\pipe\".len());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn test_is_daemon_reachable_returns_false_when_no_daemon() {
        // A random pipe name that definitely isn't running
        let reachable =
            super::is_daemon_reachable(r"\\.\pipe\homerun-daemon-test-nonexistent").await;
        assert!(!reachable);
    }
}
