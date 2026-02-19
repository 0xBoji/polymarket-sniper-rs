use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use anyhow::Result;
use tracing::{info, error, warn, debug, trace};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;

const CLOB_WS_URL: &str = "wss://ws-subscriptions-clob.polymarket.com/ws/market";

#[derive(Debug, Clone, Serialize)]
pub struct Subscription {
    pub assets_ids: Vec<String>,
    #[serde(rename = "type")]
    pub msg_type: String,
}





// NEW STRUCTS DEFINITION
#[derive(Debug, Deserialize)]
pub struct WsMessage {
    pub market: String,
    pub price_changes: Vec<WsPriceChange>,
    pub timestamp: String,
    pub event_type: String,
}

#[derive(Debug, Deserialize)]
pub struct WsBookSnapshot {
    pub asset_id: String,
    pub bids: Vec<PriceLevel>,
    pub asks: Vec<PriceLevel>,
    pub timestamp: String,
    pub hash: String,
}

#[derive(Debug, Deserialize)]
pub struct WsPriceChange {
    pub asset_id: String,
    pub price: String,
    pub size: String,
    pub side: String,
    pub hash: String,
    pub best_bid: String,
    pub best_ask: String,
}

#[derive(Debug, Deserialize)]
pub struct OrderbookUpdate {
    pub asset_id: String,
    pub bids: Vec<PriceLevel>, // Kept for compatibility with sniper.rs
    pub asks: Vec<PriceLevel>,
    pub timestamp: String,
    pub hash: String,
}

#[derive(Debug, Deserialize)]
pub struct PriceLevel {
    pub price: String,
    pub size: String,
}

pub struct ClobWebSocket {
    // We might need to send subscriptions dynamically
    subscribe_tx: mpsc::UnboundedSender<Vec<String>>,
}

impl ClobWebSocket {
    pub async fn new(
        update_tx: mpsc::Sender<OrderbookUpdate>
    ) -> Result<Self> {
        let (subscribe_tx, mut subscribe_rx) = mpsc::unbounded_channel::<Vec<String>>();

        tokio::spawn(async move {
            loop {
                info!("🔌 Connecting to CLOB WebSocket: {}", CLOB_WS_URL);
                match connect_async(CLOB_WS_URL).await {
                    Ok((ws_stream, _)) => {
                        info!("✅ CLOB WebSocket Connected!");
                        let (mut write, mut read) = ws_stream.split();

                        // Keep registration of new subs
                        // In a real robust implem, we should re-subscribe to everything on reconnect.
                        // For now, we listen for new requests.
                        
                        let mut pending_subs: Vec<String> = Vec::new();
                        let mut invalid_operation_logged = false;
                        let mut unknown_object_logged = false;
                        let mut flush_interval = tokio::time::interval(std::time::Duration::from_millis(200));

                        loop {
                            tokio::select! {
                                Some(msg) = read.next() => {
                                    match msg {
                                        Ok(Message::Text(text)) => {
                                            let json: Value = match serde_json::from_str(&text) {
                                                Ok(v) => v,
                                                Err(e) => {
                                                    if !text.contains("check_ka") {
                                                        if text.trim().eq_ignore_ascii_case("INVALID OPERATION") {
                                                            if !invalid_operation_logged {
                                                                debug!("ℹ️ Ignoring repeated WS non-JSON control message: {}", text.trim());
                                                                invalid_operation_logged = true;
                                                            }
                                                        } else {
                                                            trace!("ℹ️ Ignored non-JSON WS msg: {} | {}", e, text);
                                                        }
                                                    }
                                                    continue;
                                                }
                                            };

                                            // 1) Array snapshots
                                            if json.is_array() {
                                                if let Ok(snapshots) = serde_json::from_value::<Vec<WsBookSnapshot>>(json) {
                                                    for snap in snapshots {
                                                        let update = OrderbookUpdate {
                                                            asset_id: snap.asset_id,
                                                            bids: snap.bids,
                                                            asks: snap.asks,
                                                            timestamp: snap.timestamp,
                                                            hash: snap.hash,
                                                        };
                                                        if let Err(e) = update_tx.send(update).await {
                                                            error!("❌ Failed to send snapshot update to agent: {}", e);
                                                        }
                                                    }
                                                    continue;
                                                }
                                                debug!("ℹ️ Ignored WS array msg (unknown shape)");
                                                continue;
                                            }

                                            // 2) Object events (book / last_trade_price / price_changes)
                                            if let Some(obj) = json.as_object() {
                                                let event_type = obj
                                                    .get("event_type")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or_default();

                                                if event_type == "last_trade_price" {
                                                    // We do not use last trade ticks for orderbook-based strategy.
                                                    continue;
                                                }

                                                if event_type == "book" || (obj.contains_key("bids") && obj.contains_key("asks")) {
                                                    if let Ok(snap) = serde_json::from_value::<WsBookSnapshot>(Value::Object(obj.clone())) {
                                                        let update = OrderbookUpdate {
                                                            asset_id: snap.asset_id,
                                                            bids: snap.bids,
                                                            asks: snap.asks,
                                                            timestamp: snap.timestamp,
                                                            hash: snap.hash,
                                                        };
                                                        if let Err(e) = update_tx.send(update).await {
                                                            error!("❌ Failed to send book update to agent: {}", e);
                                                        }
                                                        continue;
                                                    }
                                                }

                                                if obj.contains_key("price_changes") {
                                                    if let Ok(msg) = serde_json::from_value::<WsMessage>(Value::Object(obj.clone())) {
                                                        for change in msg.price_changes {
                                                            let update = OrderbookUpdate {
                                                                asset_id: change.asset_id,
                                                                bids: vec![PriceLevel { price: change.best_bid, size: "0".to_string() }],
                                                                asks: vec![PriceLevel { price: change.best_ask, size: "0".to_string() }],
                                                                timestamp: msg.timestamp.clone(),
                                                                hash: change.hash,
                                                            };
                                                            if let Err(e) = update_tx.send(update).await {
                                                                error!("❌ Failed to send update to agent: {}", e);
                                                            }
                                                        }
                                                        continue;
                                                    }
                                                }
                                            }

                                            if !text.contains("check_ka") && !unknown_object_logged {
                                                debug!("ℹ️ Ignoring unsupported WS object payload shape (logging once per connection)");
                                                unknown_object_logged = true;
                                            }
                                        }

// ... (Ping/Close handlers remain same) ...


                                        Ok(Message::Ping(ping)) => {
                                             if let Err(_) = write.send(Message::Pong(ping)).await {
                                                 break;
                                             }
                                        }
                                        Ok(Message::Close(_)) => {
                                            warn!("🔌 CLOB WebSocket closed");
                                            break;
                                        }
                                        Err(e) => {
                                            error!("❌ CLOB WebSocket Error: {}", e);
                                            break;
                                        }
                                        _ => {}
                                    }
                                }
                                Some(mut assets) = subscribe_rx.recv() => {
                                    pending_subs.append(&mut assets);
                                }
                                _ = flush_interval.tick() => {
                                    if !pending_subs.is_empty() {
                                        let mut batch: Vec<String> = pending_subs.drain(..).collect();
                                        batch.sort();
                                        batch.dedup();
                                        
                                        // Chunking to avoid too large frames (e.g. 50 assets per msg)
                                        for chunk in batch.chunks(50) {
                                            let sub = Subscription {
                                                assets_ids: chunk.to_vec(),
                                                msg_type: "market".to_string(),
                                            };
                                            let json = serde_json::to_string(&sub).unwrap_or_default();
                                            debug!("📤 Sending Sub: {}", json); // Downgrade to debug
                                            if let Err(e) = write.send(Message::Text(json)).await {
                                                error!("❌ Failed to send batch subscription: {}", e);
                                            }
                                        }
                                    }
                                }
                                else => break,
                            }
                        }
                    }
                    Err(e) => {
                        error!("❌ Connection failed: {}", e);
                    }
                }
                
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                info!("🔄 Reconnecting CLOB WebSocket...");
            }
        });

        Ok(Self { subscribe_tx })
    }

    pub fn subscribe(&self, asset_ids: Vec<String>) {
        if let Err(e) = self.subscribe_tx.send(asset_ids) {
            error!("❌ Failed to queue subscription: {}", e);
        }
    }
}
