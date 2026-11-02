//! HTTP layer.
//!
//! One [`reqwest`] client with rustls, shared by everything Kaeri fetches: AniList and its image
//! CDN, the trackers, the plugin stores, the mapping datasets, plugin `fetch` calls and the
//! streams the loopback proxy relays. It honours the system's proxy settings.

pub mod client;
pub mod fetch;
pub mod loopback;
pub mod ratelimit;

pub use client::{HttpClient, NetError, trusted_roots};
pub use fetch::{Conditional, ConditionalResponse};
pub use ratelimit::RateLimiter;

pub type Result<T> = std::result::Result<T, NetError>;
