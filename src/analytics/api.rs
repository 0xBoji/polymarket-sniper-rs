use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use std::sync::{Arc, Mutex};
use tower_http::cors::{CorsLayer, Any};
use tower_http::services::ServeDir;

use super::pnl::{PnLTracker, Position, Trade, PnLStats};

pub type SharedPnLTracker = Arc<Mutex<PnLTracker>>;

pub async fn run_server(tracker: SharedPnLTracker) {
    println!("DEBUG: [API] run_server called!");
    tracing::info!("🌐 Starting dashboard server...");
    
    let app = Router::new()
        .route("/api/positions", get(get_positions))
        .route("/api/trades", get(get_trades))
        .route("/api/pnl", get(get_pnl))
        .route("/api/stats", get(get_stats))
        .fallback_service(ServeDir::new("dashboard"))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any))
        .with_state(tracker);

    println!("DEBUG: [API] Binding to 0.0.0.0:3002...");
    match tokio::net::TcpListener::bind("0.0.0.0:3002").await {
        Ok(listener) => {
            tracing::info!("🌐 Dashboard server running on http://localhost:3002");
            println!("DEBUG: [API] ✅ Bound to port 3002! Starting serve loop...");
            
            if let Err(e) = axum::serve(listener, app).await {
                tracing::error!("❌ Dashboard server error: {}", e);
                println!("DEBUG: [API] ❌ Serve error: {}", e);
            }
        }
        Err(e) => {
            tracing::error!("❌ Failed to bind dashboard server to port 3002: {}", e);
            println!("DEBUG: [API] ❌ Failed to bind: {}", e);
        }
    }
}

async fn get_positions(
    State(tracker): State<SharedPnLTracker>,
) -> Result<Json<Vec<Position>>, StatusCode> {
    let tracker = tracker.lock().unwrap();
    let positions: Vec<Position> = tracker.positions.values().cloned().collect();
    Ok(Json(positions))
}

async fn get_trades(
    State(tracker): State<SharedPnLTracker>,
) -> Result<Json<Vec<Trade>>, StatusCode> {
    let tracker = tracker.lock().unwrap();
    Ok(Json(tracker.trades.clone()))
}

async fn get_pnl(
    State(tracker): State<SharedPnLTracker>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let tracker = tracker.lock().unwrap();
    Ok(Json(serde_json::json!({
        "total_pnl": tracker.calculate_total_pnl(),
        "unrealized_pnl": tracker.calculate_unrealized_pnl(),
        "realized_pnl": tracker.calculate_realized_pnl(),
        "portfolio_value": tracker.portfolio_value(),
    })))
}

async fn get_stats(
    State(tracker): State<SharedPnLTracker>,
) -> Result<Json<PnLStats>, StatusCode> {
    let tracker = tracker.lock().unwrap();
    Ok(Json(tracker.get_stats()))
}
