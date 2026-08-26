use std::time::Duration;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconnectBackoff {
    initial_delay: Duration,
    maximum_delay: Duration,
    consecutive_failures: u32,
}

impl ReconnectBackoff {
    pub fn new(
        initial_delay: Duration,
        maximum_delay: Duration,
    ) -> Result<Self, ReconnectBackoffError> {
        if initial_delay.is_zero() {
            return Err(ReconnectBackoffError::InitialDelayIsZero);
        }

        if maximum_delay < initial_delay {
            return Err(ReconnectBackoffError::MaximumDelayTooShort);
        }

        Ok(Self {
            initial_delay,
            maximum_delay,
            consecutive_failures: 0,
        })
    }

    pub fn next_delay(&mut self) -> Duration {
        let multiplier = 1_u32
            .checked_shl(self.consecutive_failures)
            .unwrap_or(u32::MAX);
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);

        self.initial_delay
            .saturating_mul(multiplier)
            .min(self.maximum_delay)
    }

    pub fn reset(&mut self) {
        self.consecutive_failures = 0;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReconnectBackoffError {
    InitialDelayIsZero,
    MaximumDelayTooShort,
}
