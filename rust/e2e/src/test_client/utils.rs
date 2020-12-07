use async_trait::async_trait;
use futures::future::BoxFuture;
use std::sync::Arc;
use tokio::sync::mpsc;
use xaynet_core::{
    common::RoundParameters,
    crypto::{ByteObject, Signature, SigningKeyPair},
    mask::Model,
    ParticipantSecretKey,
};
use xaynet_sdk::{
    client::Client as ApiClient,
    settings::PetSettings,
    ModelStore,
    Notify,
    StateMachine,
    TransitionOutcome,
};

#[derive(Ord, PartialOrd, Eq, PartialEq)]
pub enum ClientType {
    Awaiting,
    Sum,
    Update,
}

pub fn generate_client(r#type: &ClientType, round_params: &RoundParameters) -> SigningKeyPair {
    loop {
        let (client_type, key_pair) = new_client(&round_params);
        if client_type == *r#type {
            break key_pair;
        }
    }
}

fn new_client(round_params: &RoundParameters) -> (ClientType, SigningKeyPair) {
    let key_pair = SigningKeyPair::generate();
    let role = determine_role(
        key_pair.secret.clone(),
        round_params.seed.as_slice(),
        round_params.sum,
        round_params.update,
    );
    (role, key_pair)
}

pub fn determine_role(
    secret_key: ParticipantSecretKey,
    round_seed: &[u8],
    round_sum: f64,
    round_update: f64,
) -> ClientType {
    let (sum_signature, update_signature) = compute_signatures(secret_key, round_seed);
    if sum_signature.is_eligible(round_sum) {
        ClientType::Sum
    } else if update_signature.is_eligible(round_update) {
        ClientType::Update
    } else {
        ClientType::Awaiting
    }
}

/// Compute the sum and update signatures for the given round seed.
fn compute_signatures(
    secret_key: ParticipantSecretKey,
    round_seed: &[u8],
) -> (Signature, Signature) {
    (
        secret_key.sign_detached(&[round_seed, b"sum"].concat()),
        secret_key.sign_detached(&[round_seed, b"update"].concat()),
    )
}

pub fn default_sum_client(
    key_pair: SigningKeyPair,
    api_client: ApiClient<reqwest::Client>,
    model_store: LocalModel,
) -> BoxFuture<'static, BoxFuture<'static, (StateMachine, mpsc::Receiver<Event>)>> {
    let (event_tx, mut event_rx) = mpsc::channel::<Event>(2);

    let mut sum_client = StateMachine::new(
        PetSettings::new(key_pair),
        api_client,
        model_store,
        Notifier(event_tx),
    );

    Box::pin(async move {
        // Idle event
        let _ = event_rx.try_recv();

        for _ in 0..2 {
            sum_client = match sum_client.transition().await {
                TransitionOutcome::Pending(s) => s,
                TransitionOutcome::Complete(s) => s,
            };
        }

        // NewRound event
        let _ = event_rx.try_recv();

        for _ in 0..4 {
            sum_client = match sum_client.transition().await {
                TransitionOutcome::Pending(s) => s,
                TransitionOutcome::Complete(s) => s,
            };
        }

        Box::pin(async {
            loop {
                sum_client = match sum_client.transition().await {
                    TransitionOutcome::Pending(s) => s,
                    TransitionOutcome::Complete(s) => s,
                };
                if let Ok(Event::Idle) | Ok(Event::NewRound) = event_rx.try_recv() {
                    break;
                }
            }

            (sum_client, event_rx)
        }) as BoxFuture<'static, (StateMachine, mpsc::Receiver<Event>)>
    })
}

pub fn default_update_client(
    key_pair: SigningKeyPair,
    api_client: ApiClient<reqwest::Client>,
    model_store: LocalModel,
) -> BoxFuture<'static, (StateMachine, mpsc::Receiver<Event>)> {
    let (event_tx, mut event_rx) = mpsc::channel::<Event>(2);

    let mut update_client = StateMachine::new(
        PetSettings::new(key_pair),
        api_client,
        model_store,
        Notifier(event_tx),
    );

    Box::pin(async move {
        // Idle event
        let _ = event_rx.try_recv();

        for _ in 0..2 {
            update_client = match update_client.transition().await {
                TransitionOutcome::Pending(s) => s,
                TransitionOutcome::Complete(s) => s,
            };
        }

        // NewRound event
        let _ = event_rx.try_recv();

        loop {
            update_client = match update_client.transition().await {
                TransitionOutcome::Pending(s) => s,
                TransitionOutcome::Complete(s) => s,
            };
            if let Ok(Event::Idle) | Ok(Event::NewRound) = event_rx.try_recv() {
                break;
            }
        }

        (update_client, event_rx)
    })
}

pub enum Event {
    Idle,
    NewRound,
}

#[derive(Clone)]
pub struct Notifier(pub mpsc::Sender<Event>);

impl Notify for Notifier {
    fn idle(&mut self) {
        if let Err(e) = self.0.try_send(Event::Idle) {
            warn!("failed to notify participant: {}", e);
        }
    }

    fn new_round(&mut self) {
        if let Err(e) = self.0.try_send(Event::NewRound) {
            warn!("failed to notify participant: {}", e);
        }
    }
}

pub struct LocalModel(pub Arc<Model>);

#[async_trait]
impl ModelStore for LocalModel {
    type Model = Arc<Model>;
    type Error = std::convert::Infallible;

    async fn load_model(&mut self) -> Result<Option<Self::Model>, Self::Error> {
        Ok(Some(self.0.clone()))
    }
}
