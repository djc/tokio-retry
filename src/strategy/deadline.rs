use std::time::Duration;
use std::time::Instant;

/// Wraps a strategy, applying deadline, after which strategy will
/// stop retrying.
pub trait Deadline: Iterator<Item = Duration> {
    /// Applies a deadline for a strategy. In `max_duration` from now,
    /// the strategy will stop retrying.
    fn deadline(self, max_duration: Duration) -> DeadlineIterator<Self>
    where
        Self: Sized,
    {
        DeadlineIterator {
            iter: self,
            start: Instant::now(),
            max_duration,
        }
    }
}

impl<I> Deadline for I where I: Iterator<Item = Duration> {}

/// A strategy wrapper with applied deadline,
/// created by [`Deadline::deadline`] function.
#[derive(Debug)]
pub struct DeadlineIterator<I> {
    iter: I,
    start: Instant,
    max_duration: Duration,
}

impl<I: Iterator<Item = Duration>> Iterator for DeadlineIterator<I> {
    type Item = Duration;

    fn next(&mut self) -> Option<Self::Item> {
        if self.start.elapsed() > self.max_duration {
            None
        } else {
            self.iter.next()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::strategy::FixedInterval;

    #[tokio::test]
    async fn returns_none_after_deadline_passes() {
        let mut s = FixedInterval::from_millis(10).deadline(Duration::from_millis(50));
        assert_eq!(s.next(), Some(Duration::from_millis(10)));
        tokio::time::sleep(Duration::from_millis(15)).await;
        assert_eq!(s.next(), Some(Duration::from_millis(10)));
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(s.next(), None);
    }
}
