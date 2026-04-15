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

use core::future::Future;

mod condition;
mod future;
/// Assorted retry strategies including fixed interval and exponential back-off.
pub mod strategy;

pub use condition::Condition;
pub use future::{Retry, RetryIf};

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
