use std::collections::VecDeque;
use std::time::Duration;

use crate::error::ValidationError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RateLimitPolicy {
    rest_per_minute: usize,
    rest_per_day: usize,
    order_mutations_per_second: usize,
}

impl RateLimitPolicy {
    pub fn new(
        rest_per_minute: usize,
        rest_per_day: usize,
        order_mutations_per_second: usize,
    ) -> Result<Self, ValidationError> {
        if rest_per_minute == 0 || rest_per_day == 0 || order_mutations_per_second == 0 {
            return Err(ValidationError::new(
                "rate-limit policy values must all be positive",
            ));
        }
        Ok(Self {
            rest_per_minute,
            rest_per_day,
            order_mutations_per_second,
        })
    }

    pub fn documented_defaults() -> Self {
        Self {
            rest_per_minute: 100,
            rest_per_day: 5_000,
            order_mutations_per_second: 10,
        }
    }

    pub fn rest_per_minute(self) -> usize {
        self.rest_per_minute
    }
    pub fn rest_per_day(self) -> usize {
        self.rest_per_day
    }
    pub fn order_mutations_per_second(self) -> usize {
        self.order_mutations_per_second
    }
}

impl Default for RateLimitPolicy {
    fn default() -> Self {
        Self::documented_defaults()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RequestClass {
    Read,
    SetFunds,
    PlaceOrder,
    ModifyOrder,
    CancelOrder,
    SquareOff,
    GttMutation,
}

impl RequestClass {
    pub(crate) fn is_order_mutation(self) -> bool {
        matches!(
            self,
            Self::PlaceOrder
                | Self::ModifyOrder
                | Self::CancelOrder
                | Self::SquareOff
                | Self::GttMutation
        )
    }

    pub(crate) fn is_mutation(self) -> bool {
        !matches!(self, Self::Read)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RateDecision {
    Allow,
    Wait(Duration),
}

#[derive(Debug)]
pub struct RateLimiterModel {
    policy: RateLimitPolicy,
    minute: VecDeque<Duration>,
    day: VecDeque<Duration>,
    mutations: VecDeque<Duration>,
}

impl RateLimiterModel {
    pub fn new(policy: RateLimitPolicy) -> Self {
        Self {
            policy,
            minute: VecDeque::new(),
            day: VecDeque::new(),
            mutations: VecDeque::new(),
        }
    }

    pub fn try_acquire_at(&mut self, class: RequestClass, now: Duration) -> RateDecision {
        prune(&mut self.minute, now, Duration::from_secs(60));
        prune(&mut self.day, now, Duration::from_secs(86_400));
        prune(&mut self.mutations, now, Duration::from_secs(1));

        let mut wait = Duration::ZERO;
        if self.minute.len() >= self.policy.rest_per_minute {
            wait = wait.max(until(self.minute[0], now, Duration::from_secs(60)));
        }
        if self.day.len() >= self.policy.rest_per_day {
            wait = wait.max(until(self.day[0], now, Duration::from_secs(86_400)));
        }
        if class.is_order_mutation()
            && self.mutations.len() >= self.policy.order_mutations_per_second
        {
            wait = wait.max(until(self.mutations[0], now, Duration::from_secs(1)));
        }
        if !wait.is_zero() {
            return RateDecision::Wait(wait);
        }

        self.minute.push_back(now);
        self.day.push_back(now);
        if class.is_order_mutation() {
            self.mutations.push_back(now);
        }
        RateDecision::Allow
    }

    pub fn record_validation_failure(&mut self, _class: RequestClass) {}
    pub fn rest_calls_recorded(&self) -> usize {
        self.day.len()
    }
    pub fn order_mutations_recorded(&self) -> usize {
        self.mutations.len()
    }
}

fn prune(values: &mut VecDeque<Duration>, now: Duration, window: Duration) {
    while values
        .front()
        .is_some_and(|then| now.saturating_sub(*then) >= window)
    {
        values.pop_front();
    }
}

fn until(then: Duration, now: Duration, window: Duration) -> Duration {
    then.saturating_add(window).saturating_sub(now)
}
