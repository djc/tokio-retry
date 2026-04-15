# tokio-retry

Extensible, asynchronous retry behaviours for the ecosystem of [tokio](https://tokio.rs/) libraries.

[![Documentation](https://docs.rs/tokio-retry/badge.svg)](https://docs.rs/tokio-retry/)
[![Crates.io](https://img.shields.io/crates/v/tokio-retry.svg)](https://crates.io/crates/tokio-retry)
[![Build status](https://github.com/djc/tokio-retry/workflows/CI/badge.svg)](https://github.com/djc/tokio-retry/actions?query=workflow%3ACI)

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
tokio-retry = "0.3"
```

## Examples

```rust
use tokio_retry::Retry;
use tokio_retry::strategy::{ExponentialBackoff, jitter};

async fn action() -> Result<u64, ()> {
    // do some real-world stuff here...
    Err(())
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    let retry_strategy = ExponentialBackoff::from_millis(10)
        .map(jitter) // add jitter to delays
        .take(3);    // limit to 3 retries

    let result = Retry::spawn(retry_strategy, action).await?;

    Ok(())
}
```
