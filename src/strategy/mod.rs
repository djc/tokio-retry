mod fixed_interval;
mod jitter;

pub use self::fixed_interval::FixedInterval;
pub use self::jitter::jitter;

use std::iter::Iterator;
use std::time::Duration;
use std::u64::MAX as U64_MAX;

/// A retry strategy driven by exponential back-off.
///
/// The power corresponds to the number of past attempts.
#[derive(Debug, Clone)]
pub struct ExponentialBackoff {
    current: u64,
    base: u64,
    factor: u64,
    max_delay: Option<Duration>,
}

impl ExponentialBackoff {
    /// Constructs a new exponential back-off strategy,
    /// given a base duration in milliseconds.
    ///
    /// The resulting duration is calculated by taking the base to the `n`-th power,
    /// where `n` denotes the number of past attempts.
    pub fn from_millis(base: u64) -> ExponentialBackoff {
        ExponentialBackoff {
            current: base,
            base: base,
            factor: 1u64,
            max_delay: None,
        }
    }

    /// A multiplicative factor that will be applied to the retry delay.
    ///
    /// For example, using a factor of `1000` will make each delay in units of seconds.
    ///
    /// Default factor is `1`.
    pub fn factor(mut self, factor: u64) -> ExponentialBackoff {
        self.factor = factor;
        self
    }

    /// Apply a maximum delay. No retry delay will be longer than this `Duration`.
    pub fn max_delay(mut self, duration: Duration) -> ExponentialBackoff {
        self.max_delay = Some(duration);
        self
    }
}

impl Iterator for ExponentialBackoff {
    type Item = Duration;

    fn next(&mut self) -> Option<Duration> {
        // set delay duration by applying factor
        let duration = if let Some(duration) = self.current.checked_mul(self.factor) {
            Duration::from_millis(duration)
        } else {
            Duration::from_millis(U64_MAX)
        };

        // check if we reached max delay
        if let Some(ref max_delay) = self.max_delay {
            if duration > *max_delay {
                return Some(*max_delay);
            }
        }

        if let Some(next) = self.current.checked_mul(self.base) {
            self.current = next;
        } else {
            self.current = U64_MAX;
        }

        Some(duration)
    }
}

/// A retry strategy driven by the fibonacci series.
///
/// Each retry uses a delay which is the sum of the two previous delays.
///
/// Depending on the problem at hand, a fibonacci retry strategy might
/// perform better and lead to better throughput than the `ExponentialBackoff`
/// strategy.
///
/// See ["A Performance Comparison of Different Backoff Algorithms under Different Rebroadcast Probabilities for MANETs."](https://www.researchgate.net/profile/Saher-Manaseer/publication/255672213_A_Performance_Comparison_of_Different_Backoff_Algorithms_under_Different_Rebroadcast_Probabilities_for_MANET's/links/542d40220cf29bbc126d2378/A-Performance-Comparison-of-Different-Backoff-Algorithms-under-Different-Rebroadcast-Probabilities-for-MANETs.pdf)
/// for more details.
#[derive(Debug, Clone)]
pub struct FibonacciBackoff {
    curr: u64,
    next: u64,
    factor: u64,
    max_delay: Option<Duration>,
}

impl FibonacciBackoff {
    /// Constructs a new fibonacci back-off strategy,
    /// given a base duration in milliseconds.
    pub fn from_millis(millis: u64) -> FibonacciBackoff {
        FibonacciBackoff {
            curr: millis,
            next: millis,
            factor: 1u64,
            max_delay: None,
        }
    }

    /// A multiplicative factor that will be applied to the retry delay.
    ///
    /// For example, using a factor of `1000` will make each delay in units of seconds.
    ///
    /// Default factor is `1`.
    pub fn factor(mut self, factor: u64) -> FibonacciBackoff {
        self.factor = factor;
        self
    }

    /// Apply a maximum delay. No retry delay will be longer than this `Duration`.
    pub fn max_delay(mut self, duration: Duration) -> FibonacciBackoff {
        self.max_delay = Some(duration);
        self
    }
}

impl Iterator for FibonacciBackoff {
    type Item = Duration;

    fn next(&mut self) -> Option<Duration> {
        // set delay duration by applying factor
        let duration = if let Some(duration) = self.curr.checked_mul(self.factor) {
            Duration::from_millis(duration)
        } else {
            Duration::from_millis(U64_MAX)
        };

        // check if we reached max delay
        if let Some(ref max_delay) = self.max_delay {
            if duration > *max_delay {
                return Some(*max_delay);
            }
        }

        if let Some(next_next) = self.curr.checked_add(self.next) {
            self.curr = self.next;
            self.next = next_next;
        } else {
            self.curr = self.next;
            self.next = U64_MAX;
        }

        Some(duration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exp_returns_some_exponential_base_10() {
        let mut s = ExponentialBackoff::from_millis(10);

        assert_eq!(s.next(), Some(Duration::from_millis(10)));
        assert_eq!(s.next(), Some(Duration::from_millis(100)));
        assert_eq!(s.next(), Some(Duration::from_millis(1000)));
    }

    #[test]
    fn exp_returns_some_exponential_base_2() {
        let mut s = ExponentialBackoff::from_millis(2);

        assert_eq!(s.next(), Some(Duration::from_millis(2)));
        assert_eq!(s.next(), Some(Duration::from_millis(4)));
        assert_eq!(s.next(), Some(Duration::from_millis(8)));
    }

    #[test]
    fn exp_saturates_at_maximum_value() {
        let mut s = ExponentialBackoff::from_millis(U64_MAX - 1);

        assert_eq!(s.next(), Some(Duration::from_millis(U64_MAX - 1)));
        assert_eq!(s.next(), Some(Duration::from_millis(U64_MAX)));
        assert_eq!(s.next(), Some(Duration::from_millis(U64_MAX)));
    }

    #[test]
    fn exp_can_use_factor_to_get_seconds() {
        let factor = 1000;
        let mut s = ExponentialBackoff::from_millis(2).factor(factor);

        assert_eq!(s.next(), Some(Duration::from_secs(2)));
        assert_eq!(s.next(), Some(Duration::from_secs(4)));
        assert_eq!(s.next(), Some(Duration::from_secs(8)));
    }

    #[test]
    fn exp_stops_increasing_at_max_delay() {
        let mut s = ExponentialBackoff::from_millis(2).max_delay(Duration::from_millis(4));

        assert_eq!(s.next(), Some(Duration::from_millis(2)));
        assert_eq!(s.next(), Some(Duration::from_millis(4)));
        assert_eq!(s.next(), Some(Duration::from_millis(4)));
    }

    #[test]
    fn exp_returns_max_when_max_less_than_base() {
        let mut s = ExponentialBackoff::from_millis(20).max_delay(Duration::from_millis(10));

        assert_eq!(s.next(), Some(Duration::from_millis(10)));
        assert_eq!(s.next(), Some(Duration::from_millis(10)));
    }

    #[test]
    fn exp_returns_the_fibonacci_series_starting_at_10() {
        let mut iter = FibonacciBackoff::from_millis(10);
        assert_eq!(iter.next(), Some(Duration::from_millis(10)));
        assert_eq!(iter.next(), Some(Duration::from_millis(10)));
        assert_eq!(iter.next(), Some(Duration::from_millis(20)));
        assert_eq!(iter.next(), Some(Duration::from_millis(30)));
        assert_eq!(iter.next(), Some(Duration::from_millis(50)));
        assert_eq!(iter.next(), Some(Duration::from_millis(80)));
    }

    #[test]
    fn fib_saturates_at_maximum_value() {
        let mut iter = FibonacciBackoff::from_millis(U64_MAX);
        assert_eq!(iter.next(), Some(Duration::from_millis(U64_MAX)));
        assert_eq!(iter.next(), Some(Duration::from_millis(U64_MAX)));
    }

    #[test]
    fn fib_stops_increasing_at_max_delay() {
        let mut iter = FibonacciBackoff::from_millis(10).max_delay(Duration::from_millis(50));
        assert_eq!(iter.next(), Some(Duration::from_millis(10)));
        assert_eq!(iter.next(), Some(Duration::from_millis(10)));
        assert_eq!(iter.next(), Some(Duration::from_millis(20)));
        assert_eq!(iter.next(), Some(Duration::from_millis(30)));
        assert_eq!(iter.next(), Some(Duration::from_millis(50)));
        assert_eq!(iter.next(), Some(Duration::from_millis(50)));
    }

    #[test]
    fn fib_returns_max_when_max_less_than_base() {
        let mut iter = FibonacciBackoff::from_millis(20).max_delay(Duration::from_millis(10));

        assert_eq!(iter.next(), Some(Duration::from_millis(10)));
        assert_eq!(iter.next(), Some(Duration::from_millis(10)));
    }

    #[test]
    fn fib_can_use_factor_to_get_seconds() {
        let factor = 1000;
        let mut s = FibonacciBackoff::from_millis(1).factor(factor);

        assert_eq!(s.next(), Some(Duration::from_secs(1)));
        assert_eq!(s.next(), Some(Duration::from_secs(1)));
        assert_eq!(s.next(), Some(Duration::from_secs(2)));
    }
}
