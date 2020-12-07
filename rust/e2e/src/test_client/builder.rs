use super::{
    runner::ClientRunner,
    utils::{default_sum_client, default_update_client, generate_client, ClientType, LocalModel},
};
use crate::utils::concurrent_futures::ConcurrentFutures;
use std::sync::Arc;
use xaynet_core::{
    crypto::SigningKeyPair,
    mask::{FromPrimitives, Model},
};
use xaynet_sdk::{client::Client as ApiClient, XaynetClient};
use xaynet_server::settings::Settings as CoordinatorSettings;

pub struct TestClientBuilderSettings {
    number_of_sum: u64,
    number_of_update: u64,
    number_of_sum2: u64,
    model_length: usize,
}

impl TestClientBuilderSettings {
    pub fn new(
        number_of_sum: u64,
        number_of_update: u64,
        number_of_sum2: u64,
        model_length: usize,
    ) -> Self {
        Self {
            number_of_sum,
            number_of_update,
            number_of_sum2,
            model_length,
        }
    }
}

impl From<CoordinatorSettings> for TestClientBuilderSettings {
    fn from(settings: CoordinatorSettings) -> Self {
        Self {
            number_of_sum: settings.pet.min_sum_count,
            number_of_update: settings.pet.min_update_count,
            number_of_sum2: settings.pet.min_sum2_count,
            model_length: settings.model.length,
        }
    }
}

pub struct TestClientBuilder {
    settings: TestClientBuilderSettings,
    api_client: ApiClient<reqwest::Client>,
    model: Arc<Model>,
}

impl TestClientBuilder {
    pub fn new(
        settings: TestClientBuilderSettings,
        api_client: ApiClient<reqwest::Client>,
    ) -> Self {
        let model = Model::from_primitives(vec![1; settings.model_length].into_iter()).unwrap();
        Self {
            api_client,
            settings,
            model: Arc::new(model),
        }
    }

    pub async fn build_client<F, R>(
        &mut self,
        r#type: &ClientType,
        func: F,
    ) -> anyhow::Result<ConcurrentFutures<R>>
    where
        F: Fn(SigningKeyPair, ApiClient<reqwest::Client>, LocalModel) -> R,
        R: Send + 'static + futures::Future,
        <R as futures::Future>::Output: Send + 'static,
    {
        let round_params = self.api_client.get_round_params().await?;
        let mut clients = ConcurrentFutures::<R>::new(100);

        let number_of_clients = match r#type {
            ClientType::Sum => self.settings.number_of_sum,
            ClientType::Update => self.settings.number_of_update,
            _ => return Err(anyhow::anyhow!("client type is not supported")),
        };

        for _ in 0..number_of_clients {
            let key_pair = generate_client(r#type, &round_params);
            let client = func(
                key_pair,
                self.api_client.clone(),
                LocalModel(self.model.clone()),
            );

            clients.push(client);
        }

        Ok(clients)
    }

    pub async fn build_clients(&mut self) -> anyhow::Result<ClientRunner> {
        let sum_clients = self
            .build_client(&ClientType::Sum, default_sum_client)
            .await?;

        let update_clients = self
            .build_client(&ClientType::Update, default_update_client)
            .await?;

        Ok(ClientRunner::new(
            sum_clients,
            update_clients,
            self.settings.number_of_sum2,
        ))
    }
}
