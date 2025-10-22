//! Budget manager interfaces (Phase 3 target; stubs with counters).

#![deny(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BudgetConfig {
    pub max_tokens: Option<u64>,
    pub max_cost_micros: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetState {
    Within,
    Warning80,
    Warning90,
    Exceeded,
}

#[derive(Debug, Clone, Default)]
pub struct Counters {
    pub tokens: Arc<AtomicU64>,
    pub cost_micros: Arc<AtomicU64>,
}

impl Counters {
    pub fn add_tokens(&self, n: u64) {
        let _ = self.tokens.fetch_add(n, Ordering::Relaxed);
    }
    pub fn add_cost_micros(&self, n: u64) {
        let _ = self.cost_micros.fetch_add(n, Ordering::Relaxed);
    }
    pub fn snapshot(&self) -> (u64, u64) {
        (self.tokens.load(Ordering::Relaxed), self.cost_micros.load(Ordering::Relaxed))
    }
}

#[derive(Debug, Clone)]
pub struct Manager {
    cfg: BudgetConfig,
    counters: Counters,
}

impl Manager {
    pub fn new(cfg: BudgetConfig) -> Self {
        Self { cfg, counters: Counters::default() }
    }
    pub fn counters(&self) -> Counters {
        self.counters.clone()
    }
    pub fn within_limits(&self) -> bool {
        let (t, c) = self.counters.snapshot();
        self.cfg.max_tokens.map(|m| t <= m).unwrap_or(true)
            && self.cfg.max_cost_micros.map(|m| c <= m).unwrap_or(true)
    }

    pub fn add_usage(&self, tokens: u64, cost_micros: u64) {
        if tokens > 0 {
            self.counters.add_tokens(tokens);
        }
        if cost_micros > 0 {
            self.counters.add_cost_micros(cost_micros);
        }
    }

    pub fn status(&self) -> BudgetState {
        let (t, c) = self.counters.snapshot();
        let token_ratio = self
            .cfg
            .max_tokens
            .map(|m| if m > 0 { (t as f64) / (m as f64) } else { 0.0 })
            .unwrap_or(0.0);
        let cost_ratio = self
            .cfg
            .max_cost_micros
            .map(|m| if m > 0 { (c as f64) / (m as f64) } else { 0.0 })
            .unwrap_or(0.0);
        let r = token_ratio.max(cost_ratio);
        if r > 1.0 {
            BudgetState::Exceeded
        } else if r >= 0.90 {
            BudgetState::Warning90
        } else if r >= 0.80 {
            BudgetState::Warning80
        } else {
            BudgetState::Within
        }
    }
}
