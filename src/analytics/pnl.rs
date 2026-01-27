use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use chrono::{DateTime, Utc};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub id: String,
    pub market_id: String,
    pub market_question: String,
    pub side: String, // "YES" or "NO"
    pub size: f64,
    pub entry_price: f64,
    pub current_price: f64,
    pub entry_time: DateTime<Utc>,
}

impl Position {
    pub fn unrealized_pnl(&self) -> f64 {
        let price_change = if self.side == "YES" {
            self.current_price - self.entry_price
        } else {
            self.entry_price - self.current_price
        };
        self.size * price_change
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub id: String,
    pub market_id: String,
    pub market_question: String,
    pub side: String,
    pub size: f64,
    pub entry_price: f64,
    pub exit_price: Option<f64>,
    pub entry_time: DateTime<Utc>,
    pub exit_time: Option<DateTime<Utc>>,
    pub realized_pnl: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSnapshot {
    pub timestamp: DateTime<Utc>,
    pub total_value: f64,
    pub cash: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PnLStats {
    pub total_pnl: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub portfolio_value: f64,
    pub num_positions: usize,
    pub num_trades: usize,
    pub win_rate: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
}

pub struct PnLTracker {
    pub positions: HashMap<String, Position>,
    pub trades: Vec<Trade>,
    pub snapshots: Vec<PortfolioSnapshot>,
    pub initial_capital: f64,
    pub cash: f64,
}

impl PnLTracker {
    pub fn new(initial_capital: f64) -> Self {
        Self {
            positions: HashMap::new(),
            trades: Vec::new(),
            snapshots: Vec::new(),
            initial_capital,
            cash: initial_capital,
        }
    }

    pub fn add_position(&mut self, position: Position) {
        // Deduct cash for position
        self.cash -= position.size;
        self.positions.insert(position.id.clone(), position);
    }

    pub fn update_market_price(&mut self, market_id: &str, yes_price: f64, no_price: f64) {
        for position in self.positions.values_mut() {
            if position.market_id == market_id {
                position.current_price = if position.side == "YES" {
                    yes_price
                } else {
                    no_price
                };
            }
        }
    }

    pub fn close_position(&mut self, position_id: &str) -> Option<f64> {
        if let Some(position) = self.positions.remove(position_id) {
            let realized_pnl = position.unrealized_pnl();
            
            // Return cash + PnL
            self.cash += position.size + realized_pnl;
            
            // Record trade
            let trade = Trade {
                id: position.id.clone(),
                market_id: position.market_id.clone(),
                market_question: position.market_question.clone(),
                side: position.side.clone(),
                size: position.size,
                entry_price: position.entry_price,
                exit_price: Some(position.current_price),
                entry_time: position.entry_time,
                exit_time: Some(Utc::now()),
                realized_pnl: Some(realized_pnl),
            };
            self.trades.push(trade);
            
            Some(realized_pnl)
        } else {
            None
        }
    }

    pub fn calculate_unrealized_pnl(&self) -> f64 {
        self.positions.values().map(|p| p.unrealized_pnl()).sum()
    }

    pub fn calculate_realized_pnl(&self) -> f64 {
        self.trades
            .iter()
            .filter_map(|t| t.realized_pnl)
            .sum()
    }

    pub fn calculate_total_pnl(&self) -> f64 {
        self.calculate_unrealized_pnl() + self.calculate_realized_pnl()
    }

    pub fn portfolio_value(&self) -> f64 {
        self.cash + self.positions.values().map(|p| p.size + p.unrealized_pnl()).sum::<f64>()
    }

    pub fn take_snapshot(&mut self) {
        let snapshot = PortfolioSnapshot {
            timestamp: Utc::now(),
            total_value: self.portfolio_value(),
            cash: self.cash,
            unrealized_pnl: self.calculate_unrealized_pnl(),
            realized_pnl: self.calculate_realized_pnl(),
        };
        self.snapshots.push(snapshot);
    }

    pub fn get_stats(&self) -> PnLStats {
        let total_pnl = self.calculate_total_pnl();
        let unrealized_pnl = self.calculate_unrealized_pnl();
        let realized_pnl = self.calculate_realized_pnl();
        let portfolio_value = self.portfolio_value();
        
        // Win rate
        let winning_trades = self.trades.iter()
            .filter(|t| t.realized_pnl.unwrap_or(0.0) > 0.0)
            .count();
        let win_rate = if self.trades.is_empty() {
            0.0
        } else {
            winning_trades as f64 / self.trades.len() as f64
        };

        // Sharpe ratio (simplified)
        let returns: Vec<f64> = self.snapshots.windows(2)
            .map(|w| (w[1].total_value - w[0].total_value) / w[0].total_value)
            .collect();
        
        let sharpe_ratio = if returns.len() > 1 {
            let mean = returns.iter().sum::<f64>() / returns.len() as f64;
            let variance = returns.iter()
                .map(|r| (r - mean).powi(2))
                .sum::<f64>() / returns.len() as f64;
            let std_dev = variance.sqrt();
            if std_dev > 0.0 {
                mean / std_dev * (252.0_f64).sqrt() // Annualized
            } else {
                0.0
            }
        } else {
            0.0
        };

        // Max drawdown
        let mut peak = self.initial_capital;
        let mut max_dd = 0.0;
        for snapshot in &self.snapshots {
            if snapshot.total_value > peak {
                peak = snapshot.total_value;
            }
            let dd = (peak - snapshot.total_value) / peak;
            if dd > max_dd {
                max_dd = dd;
            }
        }

        PnLStats {
            total_pnl,
            unrealized_pnl,
            realized_pnl,
            portfolio_value,
            num_positions: self.positions.len(),
            num_trades: self.trades.len(),
            win_rate,
            sharpe_ratio,
            max_drawdown: max_dd,
        }
    }
}
