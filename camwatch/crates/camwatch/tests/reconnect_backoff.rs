use std::time::Duration;

use camwatch::stream::{ReconnectBackoff, ReconnectBackoffError};

#[test]
fn increases_delay_after_consecutive_failures() {
    let mut backoff = ReconnectBackoff::new(Duration::from_secs(1), Duration::from_secs(30))
        .expect("valid reconnect delays");

    assert_eq!(backoff.next_delay(), Duration::from_secs(1));
    assert_eq!(backoff.next_delay(), Duration::from_secs(2));
    assert_eq!(backoff.next_delay(), Duration::from_secs(4));
    assert_eq!(backoff.next_delay(), Duration::from_secs(8));
}

#[test]
fn caps_delay_at_the_configured_maximum() {
    let mut backoff = ReconnectBackoff::new(Duration::from_secs(3), Duration::from_secs(10))
        .expect("valid reconnect delays");

    assert_eq!(backoff.next_delay(), Duration::from_secs(3));
    assert_eq!(backoff.next_delay(), Duration::from_secs(6));
    assert_eq!(backoff.next_delay(), Duration::from_secs(10));
    assert_eq!(backoff.next_delay(), Duration::from_secs(10));
}

#[test]
fn resets_delay_after_a_successful_reconnection() {
    let mut backoff = ReconnectBackoff::new(Duration::from_secs(1), Duration::from_secs(30))
        .expect("valid reconnect delays");

    backoff.next_delay();
    backoff.next_delay();
    backoff.reset();

    assert_eq!(backoff.next_delay(), Duration::from_secs(1));
}

#[test]
fn rejects_an_invalid_delay_range() {
    assert_eq!(
        ReconnectBackoff::new(Duration::ZERO, Duration::from_secs(1)),
        Err(ReconnectBackoffError::InitialDelayIsZero)
    );
    assert_eq!(
        ReconnectBackoff::new(Duration::from_secs(2), Duration::from_secs(1)),
        Err(ReconnectBackoffError::MaximumDelayTooShort)
    );
}
