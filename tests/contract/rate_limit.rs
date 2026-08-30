use std::time::Duration;

use breeze_icici::rate_limit::{RateLimitPolicy, RequestClass};
use breeze_icici::testing::{RateDecision, RateLimiterModel};

#[test]
fn documented_default_policy_contains_all_three_independent_limits() {
    let policy = RateLimitPolicy::documented_defaults();
    assert_eq!(policy.rest_per_minute(), 100);
    assert_eq!(policy.rest_per_day(), 5_000);
    assert_eq!(policy.order_mutations_per_second(), 10);
}

#[test]
fn custom_rate_limit_policy_requires_positive_limits() {
    let policy = RateLimitPolicy::new(20, 1_000, 5).unwrap();
    assert_eq!(policy.rest_per_minute(), 20);
    assert_eq!(policy.rest_per_day(), 1_000);
    assert_eq!(policy.order_mutations_per_second(), 5);
    assert!(RateLimitPolicy::new(0, 1_000, 5).is_err());
}

#[test]
fn one_hundred_and_first_rest_call_waits_for_the_minute_window() {
    let mut limiter = RateLimiterModel::new(RateLimitPolicy::documented_defaults());
    for _ in 0..100 {
        assert_eq!(
            limiter.try_acquire_at(RequestClass::Read, Duration::ZERO),
            RateDecision::Allow
        );
    }

    assert_eq!(
        limiter.try_acquire_at(RequestClass::Read, Duration::ZERO),
        RateDecision::Wait(Duration::from_secs(60))
    );
    assert_eq!(
        limiter.try_acquire_at(RequestClass::Read, Duration::from_secs(60)),
        RateDecision::Allow
    );
}

#[test]
fn combined_order_mutations_share_the_ten_per_second_gate() {
    let mut limiter = RateLimiterModel::new(RateLimitPolicy::documented_defaults());
    let classes = [
        RequestClass::PlaceOrder,
        RequestClass::ModifyOrder,
        RequestClass::CancelOrder,
        RequestClass::SquareOff,
        RequestClass::GttMutation,
    ];

    for index in 0..10 {
        assert_eq!(
            limiter.try_acquire_at(classes[index % classes.len()], Duration::ZERO),
            RateDecision::Allow
        );
    }
    assert_eq!(
        limiter.try_acquire_at(RequestClass::PlaceOrder, Duration::ZERO),
        RateDecision::Wait(Duration::from_secs(1))
    );
    assert_eq!(
        limiter.try_acquire_at(RequestClass::CancelOrder, Duration::from_secs(1)),
        RateDecision::Allow
    );
}

#[test]
fn mutation_calls_also_consume_the_general_rest_budget() {
    let mut limiter = RateLimiterModel::new(RateLimitPolicy::documented_defaults());
    for second in 0..10 {
        for _ in 0..10 {
            assert_eq!(
                limiter.try_acquire_at(RequestClass::PlaceOrder, Duration::from_secs(second),),
                RateDecision::Allow
            );
        }
    }

    assert!(matches!(
        limiter.try_acquire_at(RequestClass::Read, Duration::from_secs(9)),
        RateDecision::Wait(_)
    ));
}

#[test]
fn daily_limit_is_independent_of_short_windows() {
    let mut limiter = RateLimiterModel::new(RateLimitPolicy::documented_defaults());
    for minute in 0..50 {
        for _ in 0..100 {
            assert_eq!(
                limiter.try_acquire_at(RequestClass::Read, Duration::from_secs(minute * 60)),
                RateDecision::Allow
            );
        }
    }

    assert!(matches!(
        limiter.try_acquire_at(RequestClass::Read, Duration::from_secs(50 * 60)),
        RateDecision::Wait(wait) if wait > Duration::from_secs(60)
    ));
}

#[test]
fn local_validation_failure_does_not_consume_network_quota() {
    let mut limiter = RateLimiterModel::new(RateLimitPolicy::documented_defaults());
    limiter.record_validation_failure(RequestClass::PlaceOrder);
    assert_eq!(limiter.rest_calls_recorded(), 0);
    assert_eq!(limiter.order_mutations_recorded(), 0);
}
