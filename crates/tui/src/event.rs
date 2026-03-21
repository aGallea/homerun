use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event as CrosstermEvent, KeyEvent};
use tokio::sync::mpsc;

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
pub fn start_event_loop(tick_rate: Duration) -> Result<mpsc::UnboundedReceiver<AppEvent>> {
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

    Ok(rx)
}
