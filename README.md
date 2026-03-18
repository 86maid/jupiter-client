# jupiter-client

[![Crates.io](https://img.shields.io/crates/v/jupiter-client.svg)](https://crates.io/crates/jupiter-client)
[![Docs.rs](https://docs.rs/jupiter-client/badge.svg)](https://docs.rs/jupiter-client)

A type-safe, asynchronous Rust client for the Jupiter Aggregator API on Solana. Facilitates seamless token swaps, route finding, and quote generation with full error handling.

## Installation

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
jupiter-client = "1.0.1"
```

Or use `cargo add`:

```shell
cargo add jupiter-client
```

## Quick Start

```rust
use jupiter_client::*;
use jupiter_client::solana_sdk::pubkey;

#[tokio::main]
async fn main(){
    let client = JupiterClient::new_with_apikey(
        "https://api.jup.ag/swap/v1",
        "6fec26c0-9178-4d63-abe2-e29f8a10107f",
    )
    .unwrap();

    let quote = client
        .quote(&QuoteRequest {
            input_mint: pubkey!("So11111111111111111111111111111111111111112"),
            output_mint: pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"),
            amount: 1_000_000_000,
            ..Default::default()
        })
        .await
        .unwrap();

    println!("{:#?}", quote);
}
```