//! WebSocket connection for extension live reload.

pub mod handlers;
pub mod models;

use crate::services::dev::extension::payload::store::ExtensionsPayloadStore;
use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

pub use handlers::{
    build_connected_payload, build_outgoing_dispatch, build_update_payload, format_log_output,
    handle_incoming_message, parse_log_message, should_upgrade_websocket,
};
pub use models::{EventType, IncomingMessage, LogPayload, WsClientMessage};

/// Create a broadcast channel used to fan out payload updates to WS clients.
pub fn setup_websocket_broadcast(
    _store: Arc<Mutex<ExtensionsPayloadStore>>,
    _manifest_version: String,
) -> broadcast::Sender<Vec<String>> {
    let (tx, _) = broadcast::channel(64);
    tx
}

/// Handle a single websocket client: send connected, then process messages + updates.
pub async fn handle_ws_socket(
    socket: WebSocket,
    store: Arc<Mutex<ExtensionsPayloadStore>>,
    update_tx: broadcast::Sender<Vec<String>>,
    manifest_version: String,
) {
    let (mut sink, mut stream) = socket.split();
    let mut update_rx = update_tx.subscribe();

    let connected = {
        let s = store.lock().unwrap();
        build_connected_payload(&s, &manifest_version)
    };
    if sink
        .send(Message::Text(connected.to_string()))
        .await
        .is_err()
    {
        return;
    }

    let mut ping_interval = tokio::time::interval(std::time::Duration::from_secs(10));

    loop {
        tokio::select! {
            _ = ping_interval.tick() => {
                if sink.send(Message::Ping(vec![])).await.is_err() {
                    break;
                }
            }
            msg = stream.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        let mut outbound: Vec<serde_json::Value> = Vec::new();
                        {
                            let mut s = store.lock().unwrap();
                            if let Some(out) = handle_incoming_message(&text, &mut s, &manifest_version) {
                                outbound.push(out);
                            }
                        }
                        for out in outbound {
                            if sink.send(Message::Text(out.to_string())).await.is_err() {
                                return;
                            }
                        }
                    }
                    Some(Ok(Message::Ping(p))) => {
                        let _ = sink.send(Message::Pong(p)).await;
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    _ => {}
                }
            }
            upd = update_rx.recv() => {
                match upd {
                    Ok(ids) if ids.first().map(|s| s.as_str()) == Some("__dispatch__") => {}
                    Ok(ids) => {
                        let payload = {
                            let s = store.lock().unwrap();
                            build_update_payload(&s, &manifest_version, &ids)
                        };
                        if sink.send(Message::Text(payload.to_string())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_log_array() {
        let msg = parse_log_message(r#"["hello", {"a":1}]"#);
        assert!(msg.contains("hello"));
        assert!(msg.contains("a"));
    }

    #[test]
    fn parse_log_fallback() {
        assert_eq!(parse_log_message("not-json"), "not-json");
    }
}
