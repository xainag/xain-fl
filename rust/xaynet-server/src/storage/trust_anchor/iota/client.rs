//! # IOTA trust anchor integration
//!
//! To fight against AI misuse the coordinator signs the hash of the decrypted aggregated global
//! model and publishes it to the IOTA Tangle. The hash is calculated from the `bincode` encoded
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
//!
//! ### General workflow
//!
//! ```ignore
//! // create a channel by sending an announcement message.
//! let announcement_msg_id = author.send_announce().unwrap();
//!
//! // send first message / link it to the announcement message
//! let (msg_id, seq) = author.send_signed_packet(&announcement_msg_id, &public_payload, &Bytes::new());
//!
//! // send second message / link it to the first message
//! // here we can use `gen_next_msg_ids` to get the current msg ID
//! let current_msg_id = author.gen_next_msg_ids(false).last().unwrap().1.link;
//! let _ = author.send_signed_packet(&current_msg_id, &public_payload, &Bytes::new());
//!
//! // now we only repeat the previous step to send the next message
//! ```
//!
//! Currently there is no way to check if the announcement message has already been sent.
//! It is not possible to just create the announcement message ID which we could use
//! to check if the message exists in the tangle. We only have the `send_announce` method
//! to choose from. However, this method always creates and sends an announcement message
//! in one step. That means that in the end there could be multiple announcement messages with
//! the same id. Obviously that's not ok but I don't how we could fix it on our side.
//!
//! ### Side note
//!
//! There is also the method `gen_next_msg_ids` which returns the current and the next message ID.
//! Interestingly, this does not work until we send the first signed message which was linked
//! to the announcement message. (see general workflow)
//!
//! To reduce the risk of having multiple messages with the same id we make some assumptions:
//!
//! **Sequence Number**
//!
//! Since `gen_next_msg_ids` returns the current message ID + the sequence number we can
//! check whether at least one message has already been sent.
//!
//! To be more precise:
//! If the sequence number is greater than one, we know that at least one signed message
//! has already been sent. If the sequence number is one, we know that no signed message has
//! been sent yet, however, we do not know whether the announcement message has already
//! been sent or not.
//!
//! **`AuthorStore` returns state**
//!
//! If the `AuthorStore` returns some state we know that we have already sent the
//! announcement message because we always save the state after sending the
//! announcement message.
//! However, we can never be sure that the status returned by the `AuthorStore` is the latest state,
//! as the coordinator between the sending and saving methods could have crashed.
//!
//! ### Caveat
//!
//! With these assumptions we can reduce the risk of having multiple messages with the same ID,
//! but we cannot prevent it!

use std::{
    convert::TryFrom,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use iota_streams::{
    app::transport::tangle::client::Client,
    app_channels::api::tangle::{Address, Author},
    ddml::types::Bytes,
};
use redis::RedisError;
use serde::{Deserialize, Serialize};
use tracing::warn;
use xaynet_core::mask::Model;

use super::store::AuthorStore;
use crate::{
    settings::IotaSettings,
    storage::traits::{StorageResult, TrustAnchor},
};

#[derive(thiserror::Error, Debug)]
pub enum IotaClientError {
    #[error("creating address failed: {0}")]
    CreateAddress(anyhow::Error),
    #[error("creating channel failed: {0}")]
    CreateChannel(anyhow::Error),
    #[error("sending message failed: {0}")]
    SendMessage(anyhow::Error),
    #[error("importing author state failed: {0}")]
    ImportState(anyhow::Error),
    #[error("exporting author state failed: {0}")]
    ExportState(anyhow::Error),
    #[error("initializing author state failed: {0}")]
    InitAuthorStore(RedisError),
    #[error("saving author state failed: {0}")]
    SaveState(RedisError),
    #[error("fetching author state failed: {0}")]
    FetchState(RedisError),
    #[error("internal error: {0}")]
    Internal(&'static str),
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct AuthorState {
    /// The serialized and encrypted state of the author.
    pub state: Vec<u8>,
    /// The address (appinst, msgid) of the announcement message.
    // Cannot use `Address` here because `Address` is not `Serialize` / `Deserialize`.
    // Implementing `#[serde(remote = )]` is not worth it since the fields
    // of `Address` are also not `Serialize` / `Deserialize`
    pub announcement_message: (String, String),
}

impl AuthorState {
    pub fn new(state: Vec<u8>, announcement_message: &Address) -> Self {
        Self {
            state,
            announcement_message: (
                announcement_message.appinst.to_string(),
                announcement_message.msgid.to_string(),
            ),
        }
    }
}

#[derive(Clone)]
pub struct IotaClient {
    author: Arc<Mutex<Author<Client>>>,
    announcement_message: Address,
    state_pwd: String,
    store: AuthorStore,
}

impl IotaClient {
    pub async fn new(settings: IotaSettings) -> Result<Self, IotaClientError> {
        let state_pwd = settings.author_state_pwd.clone();
        let mut store = AuthorStore::new(settings.store.url.clone())
            .await
            .map_err(IotaClientError::InitAuthorStore)?;

        let (author, announcement_message) = if let Some(author_state) = store
            .author_state()
            .await
            .map_err(IotaClientError::FetchState)?
        {
            let author = Author::import(&author_state.state, &state_pwd, Client::from(&settings))
                .map_err(IotaClientError::ImportState)?;
            let announcement_message =
                Address::try_from(&author_state).map_err(IotaClientError::CreateAddress)?;
            (author, announcement_message)
        } else {
            let mut author = Author::from(&settings);
            let announcement_message =
                Self::init_channel(&mut author, &mut store, &state_pwd).await?;
            (author, announcement_message)
        };

        Ok(Self {
            author: Arc::new(Mutex::new(author)),
            announcement_message,
            state_pwd,
            store,
        })
    }

    async fn init_channel(
        author: &mut Author<Client>,
        store: &mut AuthorStore,
        state_pwd: &str,
    ) -> Result<Address, IotaClientError> {
        let announcement_message = author
            .send_announce()
            .map_err(IotaClientError::CreateChannel)?;
        let state = author
            .export(&state_pwd)
            .map_err(IotaClientError::ExportState)?;
        store
            .set_author_state(&AuthorState::new(state, &announcement_message))
            .await
            .map_err(IotaClientError::SaveState)?;
        Ok(announcement_message)
    }

    fn signed_and_send_message(&mut self, payload: &str) -> Result<Vec<u8>, IotaClientError> {
        let public_payload = Bytes(payload.as_bytes().to_vec());

        let mut author = self.author.lock().unwrap();

        let current_address = author
            .gen_next_msg_ids(false)
            .last()
            .ok_or(IotaClientError::Internal("failed to get the current link"))?
            .1
            .clone();

        let current_link = if current_address.seq_no > 1 {
            &current_address.link
        } else {
            &self.announcement_message
        };

        author
            .send_signed_packet(current_link, &public_payload, &Bytes::new())
            .map_err(IotaClientError::SendMessage)?;

        author
            .export(&self.state_pwd)
            .map_err(IotaClientError::ExportState)
    }
}

#[async_trait]
impl TrustAnchor for IotaClient {
    async fn publish_proof(&mut self, global_model: &Model) -> StorageResult<()> {
        let global_model_encoded =
            bincode::serialize(global_model).map_err(|err| anyhow::anyhow!(err))?;
        let global_model_hash = hex::encode(global_model_encoded);
        let author_state = self
            .signed_and_send_message(&global_model_hash)
            .map_err(|err| anyhow::anyhow!(err))?;

        // we don't want that the round fails if the author status could not be saved
        let _ = self
            .store
            .set_author_state(&AuthorState::new(author_state, &self.announcement_message))
            .await
            .map_err(|err| warn!("failed to save author state: {}", err));
        Ok(())
    }

    async fn is_ready(&mut self) -> StorageResult<()> {
        Ok(())
    }
}
