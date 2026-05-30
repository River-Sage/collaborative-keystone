use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use chrono::{DateTime, Duration, Utc};
use tokio::sync::Mutex;

use crate::error::AppError;

const MAX_TRACKED_KEYS: usize = 10_000;

#[derive(Clone, Default)]
pub struct RateLimiter {
    attempts: Arc<Mutex<HashMap<String, VecDeque<DateTime<Utc>>>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn check(
        &self,
        key: impl Into<String>,
        max_attempts: usize,
        window: Duration,
    ) -> Result<(), AppError> {
        if max_attempts == 0 {
            return Err(AppError::Internal(
                "Invalid rate limit configuration.".to_string(),
            ));
        }

        let now = Utc::now();
        let cutoff = now - window;
        let mut attempts_by_key = self.attempts.lock().await;

        if attempts_by_key.len() > MAX_TRACKED_KEYS {
            attempts_by_key.retain(|_, attempts| {
                prune_attempts(attempts, cutoff);
                !attempts.is_empty()
            });
        }

        let attempts = attempts_by_key.entry(key.into()).or_default();
        prune_attempts(attempts, cutoff);

        if attempts.len() >= max_attempts {
            return Err(AppError::TooManyRequests(
                "Too many attempts. Please wait a few minutes and try again.".to_string(),
            ));
        }

        attempts.push_back(now);
        Ok(())
    }
}

fn prune_attempts(attempts: &mut VecDeque<DateTime<Utc>>, cutoff: DateTime<Utc>) {
    while matches!(attempts.front(), Some(attempt) if *attempt <= cutoff) {
        attempts.pop_front();
    }
}
