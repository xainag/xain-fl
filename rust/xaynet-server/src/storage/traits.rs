//! Storage API.

use async_trait::async_trait;
use derive_more::Deref;
use displaydoc::Display;
use mockall::*;
use num_enum::TryFromPrimitive;
use thiserror::Error;

use crate::state_machine::coordinator::CoordinatorState;
use xaynet_core::{
    common::RoundSeed,
    crypto::ByteObject,
    mask::{MaskObject, Model},
    LocalSeedDict,
    SeedDict,
    SumDict,
    SumParticipantEphemeralPublicKey,
    SumParticipantPublicKey,
    UpdateParticipantPublicKey,
};

/// The error type for storage operations that are not directly related to application domain.
/// These include, for example IO errors like broken pipe, file not found, out-of-memory, etc.
pub type StorageError = anyhow::Error;

/// The result of the storage operation.
pub type StorageResult<T> = Result<T, StorageError>;

#[async_trait]
/// An abstract coordinator storage.
pub trait CoordinatorStorage
where
    Self: Clone + Send + Sync + 'static,
{
    /// Sets a [`CoordinatorState`].
    ///
    /// # Behavior
    ///
    /// - If no state has been set yet, set the state and return `StorageResult::Ok(())`.
    /// - If a state already exists, override the state and return `StorageResult::Ok(())`.
    async fn set_coordinator_state(&mut self, state: &CoordinatorState) -> StorageResult<()>;

    /// Returns a [`CoordinatorState`].
    ///
    /// # Behavior
    ///
    /// - If no state has been set yet, return `StorageResult::Ok(Option::None)`.
    /// - If a state exists, return `StorageResult::Ok(Some(CoordinatorState))`.
    async fn coordinator_state(&mut self) -> StorageResult<Option<CoordinatorState>>;

    /// Adds a sum participant entry to the [`SumDict`].
    ///
    /// # Behavior
    ///
    /// - If a sum participant has been successfully added, return `StorageResult::Ok(SumPartAdd)`
    ///   containing a `Result::Ok(())`.
    /// - If the participant could not be added due to a PET protocol error, return
    ///   the corresponding `StorageResult::Ok(SumPartAdd)` containing a
    ///   `Result::Err(SumPartAddError)`.
    async fn add_sum_participant(
        &mut self,
        pk: &SumParticipantPublicKey,
        ephm_pk: &SumParticipantEphemeralPublicKey,
    ) -> StorageResult<SumPartAdd>;

    /// Returns the [`SumDict`].
    ///
    /// # Behavior
    ///
    /// - If the sum dict does not exist, return `StorageResult::Ok(Option::None)`.
    /// - If the sum dict exists, return `StorageResult::Ok(Option::Some(SumDict))`.
    async fn sum_dict(&mut self) -> StorageResult<Option<SumDict>>;

    /// Adds a local [`LocalSeedDict`] of the given [`UpdateParticipantPublicKey`] to the [`SeedDict`].
    ///
    /// # Behavior
    ///
    /// - If the local seed dict has been successfully added, return
    ///   `StorageResult::Ok(LocalSeedDictAdd)` containing a `Result::Ok(())`.
    /// - If the local seed dict could not be added due to a PET protocol error, return
    ///   the corresponding `StorageResult::Ok(LocalSeedDictAdd)` containing a
    ///   `Result::Err(LocalSeedDictAddError)`.
    async fn add_local_seed_dict(
        &mut self,
        update_pk: &UpdateParticipantPublicKey,
        local_seed_dict: &LocalSeedDict,
    ) -> StorageResult<LocalSeedDictAdd>;

    /// Returns the [`SeedDict`].
    ///
    /// # Behavior
    ///
    /// - If the seed dict does not exist, return `StorageResult::Ok(Option::None)`.
    /// - If the seed dict exists, return `StorageResult::Ok(Option::Some(SeedDict))`.
    async fn seed_dict(&mut self) -> StorageResult<Option<SeedDict>>;

    /// Increments the mask score with the given [`MaskObject`]b by one.
    ///
    /// # Behavior
    ///
    /// - If the mask score has been successfully incremented, return
    ///   `StorageResult::Ok(MaskScoreIncr)` containing a `Result::Ok(())`.
    /// - If the mask score could not be incremented due to a PET protocol error,
    ///   return the corresponding `Result::Ok(MaskScoreIncr)` containing a
    ///   `Result::Err(MaskScoreIncrError)`.
    async fn incr_mask_score(
        &mut self,
        pk: &SumParticipantPublicKey,
        mask: &MaskObject,
    ) -> StorageResult<MaskScoreIncr>;

    /// Returns the two masks with the highest score.
    ///
    /// # Behavior
    ///
    /// - If no masks exist, return `Result::Ok(Option::None)`.
    /// - If only one mask exists, return this mask
    ///   `StorageResult::Ok(Option::Some(Vec<(MaskObject, u64)>))`.
    /// - If two masks exist with the same score, return both
    ///   `StorageResult::Ok(Option::Some(Vec<(MaskObject, u64)>))`.
    /// - If two masks exist with the different score, return
    ///   both in descending order `StorageResult::Ok(Option::Some(Vec<(MaskObject, u64)>))`.
    async fn best_masks(&mut self) -> StorageResult<Option<Vec<(MaskObject, u64)>>>;

    /// Returns the number of unique masks.
    async fn number_of_unique_masks(&mut self) -> StorageResult<u64>;

    /// Deletes all coordinator data. This includes the coordinator
    /// state as well as the [`SumDict`], [`SeedDict`] and `mask` dictionary.
    async fn delete_coordinator_data(&mut self) -> StorageResult<()>;

    /// Deletes the [`SumDict`], [`SeedDict`] and `mask` dictionary.
    async fn delete_dicts(&mut self) -> StorageResult<()>;

    /// Sets the latest global model id.
    ///
    /// # Behavior
    ///
    /// - If no global model id has been set yet, set the new id and return `StorageResult::Ok(())`.
    /// - If the global model id already exists, override with the new id and
    ///   return `StorageResult::Ok(())`.
    async fn set_latest_global_model_id(&mut self, id: &str) -> StorageResult<()>;

    /// Returns the latest global model id.
    ///
    /// # Behavior
    ///
    /// - If the global model id does not exist, return `StorageResult::Ok(None)`.
    /// - If the global model id exists, return `StorageResult::Ok(Some(String)))`.
    async fn latest_global_model_id(&mut self) -> StorageResult<Option<String>>;

    /// Checks if the [`CoordinatorStorage`] is ready to process requests.
    ///
    /// # Behavior
    ///
    /// If the [`CoordinatorStorage`] is ready to process requests, return `StorageResult::Ok(())`.
    /// If the [`CoordinatorStorage`] cannot process requests because of a connection error,
    /// for example, return `StorageResult::Err(error)`.
    async fn is_ready(&mut self) -> StorageResult<()>;
}

#[async_trait]
/// An abstract model storage.
pub trait ModelStorage
where
    Self: Clone + Send + Sync + 'static,
{
    /// Sets a global model.
    ///
    /// # Behavior
    ///
    /// - If the global model already exists (has the same model id), return
    ///   `StorageResult::Err(StorageError))`.
    /// - If the global model does not exist, set the model and return `StorageResult::Ok(String)`
    async fn set_global_model(
        &mut self,
        round_id: u64,
        round_seed: &RoundSeed,
        global_model: &Model,
    ) -> StorageResult<String>;

    /// Returns a global model.
    ///
    /// # Behavior
    ///
    /// - If the global model does not exist, return `StorageResult::Ok(Option::None)`.
    /// - If the global model exists, return `StorageResult::Ok(Option::Some(Model))`.
    async fn global_model(&mut self, id: &str) -> StorageResult<Option<Model>>;

    /// Creates a unique global model id by using the round id and the round seed in which
    /// the global model was created.
    ///
    /// The format of the default implementation is `roundid_roundseed`,
    /// where the [`RoundSeed`] is encoded in hexadecimal.
    fn create_global_model_id(round_id: u64, round_seed: &RoundSeed) -> String {
        let round_seed = hex::encode(round_seed.as_slice());
        format!("{}_{}", round_id, round_seed)
    }

    /// Checks if the [`ModelStorage`] is ready to process requests.
    ///
    /// # Behavior
    ///
    /// If the [`ModelStorage`] is ready to process requests, return `StorageResult::Ok(())`.
    /// If the [`ModelStorage`] cannot process requests because of a connection error,
    /// for example, return `StorageResult::Err(error)`.
    async fn is_ready(&mut self) -> StorageResult<()>;
}

#[async_trait]
/// An abstract trust anchor provider.
pub trait TrustAnchor
where
    Self: Clone + Send + Sync + 'static,
{
    /// Publishes a proof of the global model.
    ///
    /// # Behavior
    ///
    /// Return `StorageResult::Ok(())` if the proof was published successfully,
    /// otherwise return `StorageResult::Err(error)`.
    async fn publish_proof(&mut self, global_model: &Model) -> StorageResult<()>;

    /// Checks if the [`TrustAnchor`] is ready to process requests.
    ///
    /// # Behavior
    ///
    /// If the [`TrustAnchor`] is ready to process requests, return `StorageResult::Ok(())`.
    /// If the [`TrustAnchor`] cannot process requests because of a connection error,
    /// for example, return `StorageResult::Err(error)`.
    async fn is_ready(&mut self) -> StorageResult<()>;
}

#[async_trait]
pub trait Storage: CoordinatorStorage + ModelStorage + TrustAnchor {
    /// Checks if the [`CoordinatorStorage`], [`ModelStorage`] and  [`TrustAnchor`]
    /// are ready to process requests.
    ///
    /// # Behavior
    ///
    /// If all inner services are ready to process requests,
    /// return `StorageResult::Ok(())`.
    /// If any inner service cannot process requests because of a connection error,
    /// for example, return `StorageResult::Err(error)`.
    async fn is_ready(&mut self) -> StorageResult<()>;
}

/// A wrapper that contains the result of the "add sum participant" operation.
#[derive(Deref)]
pub struct SumPartAdd(pub(crate) Result<(), SumPartAddError>);

impl SumPartAdd {
    /// Unwraps this wrapper, returning the underlying result.
    pub fn into_inner(self) -> Result<(), SumPartAddError> {
        self.0
    }
}

/// Error that can occur when adding a sum participant to the [`SumDict`].
#[derive(Display, Error, Debug, TryFromPrimitive)]
#[repr(i64)]
pub enum SumPartAddError {
    /// sum participant already exists
    AlreadyExists = 0,
}

/// A wrapper that contains the result of the "add local seed dict" operation.
#[derive(Deref)]
pub struct LocalSeedDictAdd(pub(crate) Result<(), LocalSeedDictAddError>);

impl LocalSeedDictAdd {
    /// Unwraps this wrapper, returning the underlying result.
    pub fn into_inner(self) -> Result<(), LocalSeedDictAddError> {
        self.0
    }
}

/// Error that can occur when adding a local seed dict to the [`SeedDict`].
#[derive(Display, Error, Debug, TryFromPrimitive)]
#[repr(i64)]
pub enum LocalSeedDictAddError {
    /// the length of the local seed dict and the length of sum dict are not equal
    LengthMisMatch = -1,
    /// local dict contains an unknown sum participant
    UnknownSumParticipant = -2,
    /// update participant already submitted an update
    UpdatePkAlreadySubmitted = -3,
    /// update participant already exists in the inner update seed dict
    UpdatePkAlreadyExistsInUpdateSeedDict = -4,
}

/// A wrapper that contains the result of the "increment mask score" operation.
#[derive(Deref)]
pub struct MaskScoreIncr(pub(crate) Result<(), MaskScoreIncrError>);

impl MaskScoreIncr {
    /// Unwraps this wrapper, returning the underlying result.
    pub fn into_inner(self) -> Result<(), MaskScoreIncrError> {
        self.0
    }
}

/// Error that can occur when incrementing a mask score.
#[derive(Display, Error, Debug, TryFromPrimitive)]
#[repr(i64)]
pub enum MaskScoreIncrError {
    /// unknown sum participant
    UnknownSumPk = -1,
    /// sum participant submitted a mask already
    MaskAlreadySubmitted = -2,
}

mock! {
    pub CoordinatorStore {}

    #[async_trait]
    impl CoordinatorStorage for CoordinatorStore {
        async fn set_coordinator_state(&mut self, state: &CoordinatorState) -> StorageResult<()>;
        async fn coordinator_state(&mut self) -> StorageResult<Option<CoordinatorState>>;
        async fn add_sum_participant(
            &mut self,
            pk: &SumParticipantPublicKey,
            ephm_pk: &SumParticipantEphemeralPublicKey,
        ) -> StorageResult<SumPartAdd>;
        async fn sum_dict(&mut self) -> StorageResult<Option<SumDict>>;
        async fn add_local_seed_dict(
            &mut self,
            update_pk: &UpdateParticipantPublicKey,
            local_seed_dict: &LocalSeedDict,
        ) -> StorageResult<LocalSeedDictAdd>;
        async fn seed_dict(&mut self) -> StorageResult<Option<SeedDict>>;
        async fn incr_mask_score(
            &mut self,
            pk: &SumParticipantPublicKey,
            mask: &MaskObject,
        ) -> StorageResult<MaskScoreIncr>;
        async fn best_masks(&mut self) -> StorageResult<Option<Vec<(MaskObject, u64)>>>;
        async fn number_of_unique_masks(&mut self) -> StorageResult<u64>;
        async fn delete_coordinator_data(&mut self) -> StorageResult<()>;
        async fn delete_dicts(&mut self) -> StorageResult<()>;
        async fn set_latest_global_model_id(&mut self, id: &str) -> StorageResult<()>;
        async fn latest_global_model_id(&mut self) -> StorageResult<Option<String>>;
        async fn is_ready(&mut self) -> StorageResult<()>;
    }

    impl Clone for CoordinatorStore {
        fn clone(&self) -> Self;
    }
}

mock! {
    pub ModelStore {}

    #[async_trait]
    impl ModelStorage for ModelStore {
        async fn set_global_model(
            &mut self,
            round_id: u64,
            round_seed: &RoundSeed,
            global_model: &Model,
        ) -> StorageResult<String>;
        async fn global_model(&mut self, id: &str) -> StorageResult<Option<Model>>;
        async fn is_ready(&mut self) -> StorageResult<()>;
    }

    impl Clone for ModelStore {
        fn clone(&self) -> Self;
    }

}

mock! {
    pub TrustAnchor {}

    #[async_trait]
    impl TrustAnchor for TAnchor {
        async fn publish_proof(&mut self, global_model: &Model) -> StorageResult<()>;
        async fn is_ready(&mut self) -> StorageResult<()>;
    }

    impl Clone for TAnchor {
        fn clone(&self) -> Self;
    }
}
