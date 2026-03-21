use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent};
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message as WsMessage;

/// Events the TUI reacts to.
pub enum AppEvent {
    /// A key press from the terminal.
    Key(KeyEvent),
    /// Periodic tick for polling daemon state.
    Tick,
    /// A runner event from the daemon WebSocket.
    DaemonEvent(String),
}

/// Spawns a background task that sends AppEvents into a channel.
/// Returns the receiver and the sender so callers can attach WebSocket forwarders.
pub fn start_event_loop(
    tick_rate: Duration,
) -> Result<(
    mpsc::UnboundedSender<AppEvent>,
    mpsc::UnboundedReceiver<AppEvent>,
)> {
    let (tx, rx) = mpsc::unbounded_channel();

    let key_tx = tx.clone();
    // Crossterm event polling (blocking, so run in a blocking task)
    tokio::task::spawn_blocking(move || loop {
        if event::poll(tick_rate).unwrap_or(false) {
            if let Ok(CrosstermEvent::Key(key)) = event::read() {
                if key_tx.send(AppEvent::Key(key)).is_err() {
                    break;
                }
            }
        }
    });

    let tick_tx = tx.clone();
    // Tick timer
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tick_rate);
        loop {
            interval.tick().await;
            if tick_tx.send(AppEvent::Tick).is_err() {
                break;
            }
        }
    });

    Ok((tx, rx))
}

/// Start WebSocket event forwarding (optional — if daemon is reachable).
pub fn start_ws_forwarding(
    tx: mpsc::UnboundedSender<AppEvent>,
    mut ws_read: futures::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<tokio::net::UnixStream>,
    >,
) {
    tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_read.next().await {
            if let WsMessage::Text(text) = msg {
                if tx.send(AppEvent::DaemonEvent(text.to_string())).is_err() {
                    break;
                }
            }
        }
    });
}
