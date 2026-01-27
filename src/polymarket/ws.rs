use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use anyhow::Result;
use tracing::{info, error, warn};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

const CLOB_WS_URL: &str = "wss://ws-subscriptions-clob.polymarket.com/ws/market";

#[derive(Debug, Clone, Serialize)]
pub struct Subscription {
    pub assets_ids: Vec<String>,
    #[serde(rename = "type")]
    pub msg_type: String,
}

#[derive(Debug, Deserialize)]
pub struct OrderbookUpdate {
    pub asset_id: String,
    pub bids: Vec<PriceLevel>,
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
                        
                        loop {
                            tokio::select! {
                                Some(msg) = read.next() => {
                                    match msg {
                                        Ok(Message::Text(text)) => {
                                            // info!("📩 WS Msg: {}", text); // Debug
                                            if let Ok(updates) = serde_json::from_str::<Vec<OrderbookUpdate>>(&text) {
                                                for update in updates {
                                                     if let Err(e) = update_tx.send(update).await {
                                                         error!("❌ Failed to send update to agent: {}", e);
                                                     }
                                                }
                                            } else if text.contains("event") && text.contains("param") {
                                                // Ignore system messages
                                            } else {
                                                // warn!("⚠️ Unknown WS message format: {}", text);
                                            }
                                        }
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
                                Some(assets) = subscribe_rx.recv() => {
                                    if !assets.is_empty() {
                                        let sub = Subscription {
                                            assets_ids: assets.clone(),
                                            msg_type: "market".to_string(),
                                        };
                                        let json = serde_json::to_string(&sub).unwrap_or_default();
                                        info!("Binds Subscribing to {} assets", assets.len());
                                        if let Err(e) = write.send(Message::Text(json)).await {
                                            error!("❌ Failed to send subscription: {}", e);
                                            // Break to reconnect
                                            break;
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
