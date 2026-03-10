use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use thiserror::Error;
use tokio::time::Instant;

#[derive(Default)]
pub(crate) struct StatsTable {
    ingress_traffic: u128,
    egress: u128,
    concurrency: u16,
}

impl StatsTable {
    pub(crate) const fn total_traffic(&self) -> u128 {
        self.ingress_traffic + self.egress
    }
}

enum LimitValue<T> {
    Unrestricted,
    Restricted(T),
}
pub(crate) struct Limits {
    concurrency: LimitValue<u16>,
    traffic: LimitValue<u128>,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            concurrency: LimitValue::Unrestricted,
            traffic: LimitValue::Unrestricted,
        }
    }
}

impl<T> From<Option<T>> for LimitValue<T>{
    fn from(value: Option<T>) -> Self {
        match value {
            None => LimitValue::Unrestricted,
            Some(v) => LimitValue::Restricted(v)
        }
    }
}

impl Limits {
    pub(crate) fn new(concurrency: Option<u16>, traffic: Option<u128>) -> Self {
        Limits {
            concurrency: LimitValue::from(concurrency),
            traffic: LimitValue::from(traffic),
        }
    }
    #[allow(dead_code)]
    pub(crate) const fn with_low_concurrency() -> Self {
        Self {
            concurrency: LimitValue::Restricted(2),
            traffic: LimitValue::Unrestricted,
        }
    }

    #[allow(dead_code)]
    pub(crate) const fn with_low_traffic() -> Self {
        Self {
            concurrency: LimitValue::Unrestricted,
            traffic: LimitValue::Restricted(10_000),
        }
    }

    #[allow(dead_code)]
    pub(crate) const fn with_low_limits() -> Self {
        Self {
            concurrency: LimitValue::Restricted(2),
            traffic: LimitValue::Restricted(10_000),
        }
    }
}
pub(crate) struct Limiter {
    limits: Limits,
}
impl Limiter {
    pub(crate) const fn new(limits: Limits) -> Self {
        Self { limits }
    }
    pub(crate) const fn is_limit_exceed(&self, stats: &StatsTable) -> Result<(), LimitError> {
        if self.is_concurrency_limit_exceed(stats.concurrency) {
            return Err(LimitError::ConcurrencyLimitExceed(stats.concurrency));
        }
        if self.is_traffic_limit_exceed(stats.total_traffic()) {
            return Err(LimitError::TrafficLimitExceed(stats.total_traffic()));
        }
        Ok(())
    }

    const fn is_traffic_limit_exceed(&self, total_traffic: u128) -> bool {
        match self.limits.traffic {
            LimitValue::Unrestricted => false,
            LimitValue::Restricted(value) => value < total_traffic,
        }
    }
    const fn is_concurrency_limit_exceed(&self, concurrency: u16) -> bool {
        match self.limits.concurrency {
            LimitValue::Unrestricted => false,
            LimitValue::Restricted(value) => value < concurrency,
        }
    }
}

pub(crate) struct UserContext {
    limiter: Limiter,
    stats_table: StatsTable,
    last_update_at: Instant,
}
impl UserContext {
    pub(crate) fn new(limits: Limits) -> Self {
        Self {
            limiter: Limiter::new(limits),
            stats_table: StatsTable::default(),
            last_update_at: Instant::now(),
        }
    }
    pub(crate) fn add_ingress_traffic(&mut self, traffic_value: u128) {
        self.stats_table.ingress_traffic += traffic_value;
        self.last_update_at = Instant::now();
    }
    pub(crate) fn add_egress_traffic(&mut self, traffic_value: u128) {
        self.stats_table.egress += traffic_value;
        self.last_update_at = Instant::now();
    }

    pub(crate) fn inc_concurrency(&mut self) {
        self.stats_table.concurrency += 1;
        self.last_update_at = Instant::now();
    }
    pub(crate) fn dec_concurrency(&mut self) {
        self.stats_table.concurrency = self.stats_table.concurrency.saturating_sub(1);
        self.last_update_at = Instant::now();
    }
}
pub(crate) struct Registry {
    inner: HashMap<String, UserContext>,
}

#[derive(Error, Debug)]
pub(crate) enum LimitError {
    #[error("Concurrency limit exceed")]
    ConcurrencyLimitExceed(u16),
    #[error("Traffic limit exceed")]
    TrafficLimitExceed(u128),
    #[error("User not found")]
    UserNotFound,
}

impl Registry {
    pub(crate) fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }
    pub(crate) fn create_user(&mut self, user: &str, limits: Limits) {
        self.inner
            .entry(user.to_string())
            .or_insert_with(|| UserContext::new(limits));
    }

    pub(crate) fn add_ingress_traffic(&mut self, user: &str, traffic_value: u128) {
        self.inner
            .entry(user.to_string())
            .and_modify(|ctx| ctx.add_ingress_traffic(traffic_value));
    }
    pub(crate) fn add_egress_traffic(&mut self, user: &str, traffic_value: u128) {
        self.inner
            .entry(user.to_string())
            .and_modify(|ctx| ctx.add_egress_traffic(traffic_value));
    }

    pub(crate) fn inc_concurrency(&mut self, user: &str) {
        self.inner
            .entry(user.to_string())
            .and_modify(UserContext::inc_concurrency);
    }
    pub(crate) fn dec_concurrency(&mut self, user: &str) {
        self.inner
            .entry(user.to_string())
            .and_modify(UserContext::dec_concurrency);
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub(crate) fn check_limits(&self, user: &str) -> Result<(), LimitError> {
        let ctx = self.inner.get(user).ok_or(LimitError::UserNotFound)?;
        ctx.limiter.is_limit_exceed(&ctx.stats_table)
    }
}

impl Display for Registry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (user, ctx) in &self.inner {
            writeln!(
                f,
                "User `{}` stats. ingress: {}, egress: {}",
                user, ctx.stats_table.ingress_traffic, ctx.stats_table.egress
            )?;
        }
        Ok(())
    }
}

impl Debug for Registry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits_with_concurrency(max: u16) -> Limits {
        Limits {
            concurrency: LimitValue::Restricted(max),
            traffic: LimitValue::Unrestricted,
        }
    }

    fn limits_with_traffic(max: u128) -> Limits {
        Limits {
            concurrency: LimitValue::Unrestricted,
            traffic: LimitValue::Restricted(max),
        }
    }

    #[test]
    fn limiter_allows_when_under_concurrency_limit() {
        let limiter = Limiter::new(limits_with_concurrency(2));
        let stats = StatsTable {
            concurrency: 1,
            ..Default::default()
        };

        assert!(limiter.is_limit_exceed(&stats).is_ok());
    }

    #[test]
    fn limiter_denies_when_concurrency_limit_exceeded() {
        let limiter = Limiter::new(limits_with_concurrency(2));
        let stats = StatsTable {
            concurrency: 3,
            ..Default::default()
        };

        let result = limiter.is_limit_exceed(&stats);
        assert!(matches!(result, Err(LimitError::ConcurrencyLimitExceed(3))));
    }

    #[test]
    fn limiter_allows_when_under_traffic_limit() {
        let limiter = Limiter::new(limits_with_traffic(10_000));
        let stats = StatsTable {
            ingress_traffic: 5_000,
            egress: 4_000,
            ..Default::default()
        };

        assert!(limiter.is_limit_exceed(&stats).is_ok());
    }

    #[test]
    fn limiter_denies_when_traffic_limit_exceeded() {
        let limiter = Limiter::new(limits_with_traffic(10_000));
        let stats = StatsTable {
            ingress_traffic: 6_000,
            egress: 5_000,
            ..Default::default()
        };

        let result = limiter.is_limit_exceed(&stats);
        assert!(matches!(
            result,
            Err(LimitError::TrafficLimitExceed(11_000))
        ));
    }

    #[test]
    fn limiter_allows_unrestricted() {
        let limiter = Limiter::new(Limits::default());
        let stats = StatsTable {
            concurrency: 100,
            ingress_traffic: 1_000_000,
            egress: 1_000_000,
        };

        assert!(limiter.is_limit_exceed(&stats).is_ok());
    }

    #[test]
    fn concurrency_checked_before_traffic() {
        let limits = Limits {
            concurrency: LimitValue::Restricted(1),
            traffic: LimitValue::Restricted(100),
        };
        let limiter = Limiter::new(limits);
        let stats = StatsTable {
            concurrency: 5,
            ingress_traffic: 500,
            egress: 500,
        };

        let result = limiter.is_limit_exceed(&stats);
        assert!(matches!(result, Err(LimitError::ConcurrencyLimitExceed(_))));
    }

    #[test]
    fn users_statistic_create_user_does_not_overwrite() {
        let mut stats = Registry::new();

        stats.create_user("alice", limits_with_traffic(1000));
        stats.add_ingress_traffic("alice", 500);

        stats.create_user("alice", limits_with_traffic(2000));

        let result = stats.check_limits("alice");
        assert!(result.is_ok());

        stats.add_ingress_traffic("alice", 600);
        let result = stats.check_limits("alice");
        assert!(matches!(result, Err(LimitError::TrafficLimitExceed(1100))));
    }

    #[test]
    fn users_statistic_concurrency_inc_dec() {
        let mut stats = Registry::new();
        stats.create_user("bob", limits_with_concurrency(2));

        stats.inc_concurrency("bob");
        stats.inc_concurrency("bob");
        assert!(stats.check_limits("bob").is_ok());

        stats.inc_concurrency("bob");
        assert!(matches!(
            stats.check_limits("bob"),
            Err(LimitError::ConcurrencyLimitExceed(3))
        ));

        stats.dec_concurrency("bob");
        assert!(stats.check_limits("bob").is_ok());
    }
}
