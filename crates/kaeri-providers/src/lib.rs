//! Provider registry, health tracking and failover.
//!
//! Sources are the edge of Kaeri that other people run: they change, move or go away. This crate
//! turns that from a crash into a visible, recoverable state. The chain is walked in preference
//! order, every failure is carried rather than discarded, and the Providers screen can always say
//! which source broke and why.

pub mod demo;
pub mod health;
pub mod mock;
pub mod proxy;
pub mod registry;
pub mod resolve;
pub mod torrent;

pub use health::{HealthTracker, ProviderHealth};
pub use mock::MockProvider;
pub use proxy::{ProxyState, TorrentRoute};
pub use registry::{Attempt, ProviderRegistry};
pub use resolve::{
    Answer, Candidate, Contribution, Gathered, Resolution, Surveyed, confirm, first_answer,
    gather, resolve, survey,
};
