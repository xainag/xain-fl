//! # IOTA trust anchor integration
//!
//! To fight against AI misuse the coordinator signs the hash of the decrypted aggregated global
//! model and publish it to the IOTA Tangle. The hash is calculated from the `bincode` encoded
//! model. It is the same encoding that is sent to the user via the API.
//!
//! ```ignore
//! // global_model: Model
//! let global_model_hash = hex::encode(bincode::serialize(&global_model).unwrap());
//! ```
//!
//! ## Implementation details
//!
//! IOTA trust anchor integration is based on IOTA
//! [Streams](https://docs.iota.org/docs/channels/1.3/overview).

pub(self) mod client;
pub(self) mod store;
pub(self) mod utils;

pub use client::{IotaClient, IotaClientError};
