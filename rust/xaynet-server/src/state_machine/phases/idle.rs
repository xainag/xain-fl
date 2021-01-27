use async_trait::async_trait;
use sodiumoxide::crypto::hash::sha256;
use thiserror::Error;
use tracing::{debug, info};

use crate::{
    metric,
    metrics::Measurement,
    state_machine::{
        events::DictionaryUpdate,
        phases::{Phase, PhaseName, PhaseState, Shared, Sum},
        PhaseStateError,
        StateMachine,
    },
    storage::{Storage, StorageError},
};
use xaynet_core::{
    common::RoundSeed,
    crypto::{ByteObject, EncryptKeyPair, SigningKeySeed},
};

/// Error that occurs during the idle phase.
#[derive(Error, Debug)]
pub enum IdleStateError {
    #[error("setting the coordinator state failed: {0}")]
    SetCoordinatorState(StorageError),
    #[error("deleting the dictionaries failed: {0}")]
    DeleteDictionaries(StorageError),
}

/// Idle state
#[derive(Debug)]
pub struct Idle;

#[async_trait]
impl<S> Phase<S> for PhaseState<Idle, S>
where
    S: Storage,
{
    const NAME: PhaseName = PhaseName::Idle;

    async fn run(&mut self) -> Result<(), PhaseStateError> {
        info!("updating the keys");
        self.gen_round_keypair();

        info!("updating round thresholds");
        self.update_round_thresholds();

        info!("updating round seeds");
        self.update_round_seed();

        self.shared
            .store
            .set_coordinator_state(&self.shared.state)
            .await
            .map_err(IdleStateError::SetCoordinatorState)?;

        let events = &mut self.shared.events;

        info!("broadcasting new keys");
        events.broadcast_keys(self.shared.state.keys.clone());

        info!("broadcasting invalidation of sum dictionary from previous round");
        events.broadcast_sum_dict(DictionaryUpdate::Invalidate);

        info!("broadcasting invalidation of seed dictionary from previous round");
        events.broadcast_seed_dict(DictionaryUpdate::Invalidate);

        self.shared
            .store
            .delete_dicts()
            .await
            .map_err(IdleStateError::DeleteDictionaries)?;

        info!("broadcasting new round parameters");
        events.broadcast_params(self.shared.state.round_params.clone());

        metric!(Measurement::RoundTotalNumber, self.shared.state.round_id);
        metric!(
            Measurement::RoundParamSum,
            self.shared.state.round_params.sum,
            ("round_id", self.shared.state.round_id),
            ("phase", Self::NAME as u8)
        );
        metric!(
            Measurement::RoundParamUpdate,
            self.shared.state.round_params.update,
            ("round_id", self.shared.state.round_id),
            ("phase", Self::NAME as u8)
        );

        Ok(())
    }

    fn next(self) -> Option<StateMachine<S>> {
        Some(PhaseState::<Sum, _>::new(self.shared).into())
    }
}

impl<S> PhaseState<Idle, S>
where
    S: Storage,
{
    /// Creates a new idle state.
    pub fn new(mut shared: Shared<S>) -> Self {
        // Since some events are emitted very early, the round id must
        // be correct when the idle phase starts. Therefore, we update
        // it here, when instantiating the idle PhaseState.
        shared.set_round_id(shared.round_id() + 1);
        debug!("new round ID = {}", shared.round_id());
        Self {
            private: Idle,
            shared,
        }
    }

    fn update_round_thresholds(&mut self) {}

    /// Updates the seed round parameter.
    fn update_round_seed(&mut self) {
        // Safe unwrap: `sk` and `seed` have same number of bytes
        let (_, sk) =
            SigningKeySeed::from_slice_unchecked(self.shared.state.keys.secret.as_slice())
                .derive_signing_key_pair();
        let signature = sk.sign_detached(
            &[
                self.shared.state.round_params.seed.as_slice(),
                &self.shared.state.round_params.sum.to_le_bytes(),
                &self.shared.state.round_params.update.to_le_bytes(),
            ]
            .concat(),
        );
        // Safe unwrap: the length of the hash is 32 bytes
        self.shared.state.round_params.seed =
            RoundSeed::from_slice_unchecked(sha256::hash(signature.as_slice()).as_ref());
    }

    /// Generates fresh round credentials.
    fn gen_round_keypair(&mut self) {
        self.shared.state.keys = EncryptKeyPair::generate();
        self.shared.state.round_params.pk = self.shared.state.keys.public;
    }
}

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::*;
    use crate::{
        state_machine::{
            events::Event,
            tests::{builder::StateMachineBuilder, utils},
        },
        storage::{tests::init_store, CoordinatorStorage},
    };

    #[tokio::test]
    #[serial]
    async fn integration_round_id_is_updated_when_idle_phase_runs() {
        let store = init_store().await;
        let coordinator_state = utils::coordinator_state();
        let (shared, _, event_subscriber) = utils::init_shared(coordinator_state, store);

        let keys = event_subscriber.keys_listener();
        let id = keys.get_latest().round_id;
        assert_eq!(id, 0);

        let mut idle_phase = PhaseState::<Idle, _>::new(shared);
        idle_phase.run().await.unwrap();

        let id = keys.get_latest().round_id;
        assert_eq!(id, 1);
    }

    #[tokio::test]
    #[serial]
    async fn integration_store_fails() {
        use crate::{
            state_machine::tests::utils::enable_logging,
            storage::{
                traits::{MockCoordinatorStore, MockModelStore},
                Store,
            },
        };
        use anyhow::anyhow;

        enable_logging();

        let mut cs = MockCoordinatorStore::new();
        cs.expect_set_coordinator_state()
            .return_once(move |_| Err(anyhow!("Missing attribute")));

        let store = Store::new(cs, MockModelStore::new());

        let (state_machine, _request_tx, _) =
            StateMachineBuilder::new(store).with_round_id(2).build();
        assert!(state_machine.is_idle());

        let state_machine = state_machine.next().await.unwrap();
        assert!(state_machine.is_error());
    }

    #[tokio::test]
    #[serial]
    async fn integration_idle_to_sum() {
        let mut store = init_store().await;
        let (state_machine, _request_tx, events) = StateMachineBuilder::new(store.clone())
            .with_round_id(2)
            .build();
        assert!(state_machine.is_idle());

        let initial_round_params = events.params_listener().get_latest().event;
        let initial_keys = events.keys_listener().get_latest().event;
        let initial_seed = initial_round_params.seed.clone();

        let state_machine = state_machine.next().await.unwrap();
        assert!(state_machine.is_sum());

        let PhaseState { shared, .. } = state_machine.into_sum_phase_state();

        let sum_dict = store.sum_dict().await.unwrap();
        assert!(sum_dict.is_none());

        let new_round_params = shared.state.round_params.clone();
        let new_keys = shared.state.keys.clone();

        // Make sure the seed and keys have updated
        assert_ne!(initial_seed, new_round_params.seed);
        assert_ne!(initial_keys, new_keys);

        fn expected_event<T>(event: T) -> Event<T> {
            Event { round_id: 2, event }
        }

        // Check all the events that should be emitted during the idle
        // phase
        assert_eq!(
            events.phase_listener().get_latest(),
            expected_event(PhaseName::Idle)
        );

        assert_eq!(
            events.keys_listener().get_latest(),
            expected_event(new_keys),
        );

        assert_eq!(
            events.params_listener().get_latest(),
            expected_event(new_round_params)
        );

        assert_eq!(
            events.sum_dict_listener().get_latest(),
            expected_event(DictionaryUpdate::Invalidate)
        );

        assert_eq!(
            events.seed_dict_listener().get_latest(),
            expected_event(DictionaryUpdate::Invalidate)
        );
    }
}
