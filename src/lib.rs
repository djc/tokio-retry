//! This library provides extensible asynchronous retry behaviours
//! for use with the ecosystem of [`tokio`](https://tokio.rs/) libraries.
//!
//! # Installation
//!
//! Add this to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! tokio-retry = "0.3"
//! ```
//!
//! # Example
//!
//! ```rust,no_run
//! # extern crate tokio;
//! # extern crate tokio_retry;
//! #
//! use tokio_retry::Retry;
//! use tokio_retry::strategy::{ExponentialBackoff, jitter};
//!
//! async fn action() -> Result<u64, ()> {
//!     // do some real-world stuff here...
//!     Err(())
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), ()> {
//! let retry_strategy = ExponentialBackoff::from_millis(10)
//!     .map(jitter) // add jitter to delays
//!     .take(3);    // limit to 3 retries
//!
//! let result = Retry::spawn(retry_strategy, action).await?;
//! # Ok(())
//! # }
//! ```

#![no_std]

use core::future::Future;
use core::iter::{IntoIterator, Iterator};
use core::pin::Pin;
use core::task::{Context, Poll};

use pin_project_lite::pin_project;
use tokio::time::{sleep_until, Duration, Instant, Sleep};

/// Assorted retry strategies including fixed interval and exponential back-off.
pub mod strategy;

pin_project! {
    #[project = RetryStateProj]
    enum RetryState<A>
    where
        A: Action,
    {
        Running {
            #[pin]
            future: A::Future,
        },
        Sleeping {
            #[pin]
            future: Sleep,
        },
    }
}

impl<A: Action> RetryState<A> {
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> RetryFuturePoll<A> {
        match self.project() {
            RetryStateProj::Running { future } => RetryFuturePoll::Running(future.poll(cx)),
            RetryStateProj::Sleeping { future } => RetryFuturePoll::Sleeping(future.poll(cx)),
        }
    }
}

enum RetryFuturePoll<A>
where
    A: Action,
{
    Running(Poll<Result<A::Item, A::Error>>),
    Sleeping(Poll<()>),
}

pin_project! {
    /// Future that drives multiple attempts at an action via a retry strategy.
    pub struct Retry<I, A>
    where
        I: Iterator<Item = Duration>,
        A: Action,
    {
        #[pin]
        retry_if: RetryIf<I, A, fn(&A::Error) -> bool>,
    }
}

impl<I, A> Retry<I, A>
where
    I: Iterator<Item = Duration>,
    A: Action,
{
    pub fn spawn<T: IntoIterator<IntoIter = I, Item = Duration>>(strategy: T, action: A) -> Self {
        Self {
            retry_if: RetryIf::spawn(strategy, action, (|_| true) as fn(&A::Error) -> bool),
        }
    }
}

impl<I, A> Future for Retry<I, A>
where
    I: Iterator<Item = Duration>,
    A: Action,
{
    type Output = Result<A::Item, A::Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        this.retry_if.poll(cx)
    }
}

pin_project! {
    /// Future that drives multiple attempts at an action via a retry strategy. Retries are only attempted if
    /// the `Error` returned by the future satisfies a given condition.
    pub struct RetryIf<I, A, C>
    where
        I: Iterator<Item = Duration>,
        A: Action,
        C: Condition<A::Error>,
    {
        strategy: I,
        #[pin]
        state: RetryState<A>,
        action: A,
        condition: C,
    }
}

impl<I, A, C> RetryIf<I, A, C>
where
    I: Iterator<Item = Duration>,
    A: Action,
    C: Condition<A::Error>,
{
    pub fn spawn<T: IntoIterator<IntoIter = I, Item = Duration>>(
        strategy: T,
        mut action: A,
        condition: C,
    ) -> Self {
        Self {
            strategy: strategy.into_iter(),
            state: RetryState::Running {
                future: action.run(),
            },
            action,
            condition,
        }
    }

    fn attempt(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<A::Item, A::Error>> {
        let future = {
            let this = self.as_mut().project();
            this.action.run()
        };
        self.as_mut()
            .project()
            .state
            .set(RetryState::Running { future });
        self.poll(cx)
    }

    #[allow(clippy::type_complexity)]
    fn retry(
        mut self: Pin<&mut Self>,
        err: A::Error,
        cx: &mut Context<'_>,
    ) -> Result<Poll<Result<A::Item, A::Error>>, A::Error> {
        match self.as_mut().project().strategy.next() {
            None => Err(err),
            Some(duration) => {
                let deadline = Instant::now() + duration;
                let future = sleep_until(deadline);
                self.as_mut()
                    .project()
                    .state
                    .set(RetryState::Sleeping { future });
                Ok(self.poll(cx))
            }
        }
    }
}

impl<I, A, C> Future for RetryIf<I, A, C>
where
    I: Iterator<Item = Duration>,
    A: Action,
    C: Condition<A::Error>,
{
    type Output = Result<A::Item, A::Error>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.as_mut().project().state.poll(cx) {
            RetryFuturePoll::Running(poll_result) => match poll_result {
                Poll::Ready(Ok(ok)) => Poll::Ready(Ok(ok)),
                Poll::Pending => Poll::Pending,
                Poll::Ready(Err(err)) => {
                    if self.as_mut().project().condition.should_retry(&err) {
                        match self.retry(err, cx) {
                            Ok(poll) => poll,
                            Err(err) => Poll::Ready(Err(err)),
                        }
                    } else {
                        Poll::Ready(Err(err))
                    }
                }
            },
            RetryFuturePoll::Sleeping(poll_result) => match poll_result {
                Poll::Pending => Poll::Pending,
                Poll::Ready(_) => self.attempt(cx),
            },
        }
    }
}

/// An action can be run multiple times and produces a future.
pub trait Action {
    /// The future that this action produces.
    type Future: Future<Output = Result<Self::Item, Self::Error>>;
    /// The item that the future may resolve with.
    type Item;
    /// The error that the future may resolve with.
    type Error;

    fn run(&mut self) -> Self::Future;
}

impl<R, E, T: Future<Output = Result<R, E>>, F: FnMut() -> T> Action for F {
    type Item = R;
    type Error = E;
    type Future = T;

    fn run(&mut self) -> Self::Future {
        self()
    }
}

/// Specifies under which conditions a retry is attempted.
pub trait Condition<E> {
    fn should_retry(&mut self, error: &E) -> bool;
}

impl<E, F: FnMut(&E) -> bool> Condition<E> for F {
    fn should_retry(&mut self, error: &E) -> bool {
        self(error)
    }
}
