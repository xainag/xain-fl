use super::utils::Event;
use crate::utils::concurrent_futures::ConcurrentFutures;
use futures::{future::BoxFuture, StreamExt};
use tokio::sync::mpsc;
use xaynet_sdk::StateMachine;

pub struct ClientRunner {
    sum_clients: Option<
        ConcurrentFutures<
            BoxFuture<'static, BoxFuture<'static, (StateMachine, mpsc::Receiver<Event>)>>,
        >,
    >,
    update_clients:
        Option<ConcurrentFutures<BoxFuture<'static, (StateMachine, mpsc::Receiver<Event>)>>>,
    sum2_clients:
        Option<ConcurrentFutures<BoxFuture<'static, (StateMachine, mpsc::Receiver<Event>)>>>,
    sum2_count: u64,
}

impl ClientRunner {
    pub fn new(
        sum_clients: ConcurrentFutures<
            BoxFuture<'static, BoxFuture<'static, (StateMachine, mpsc::Receiver<Event>)>>,
        >,
        update_clients: ConcurrentFutures<
            BoxFuture<'static, (StateMachine, mpsc::Receiver<Event>)>,
        >,
        sum2_count: u64,
    ) -> Self {
        Self {
            sum_clients: Some(sum_clients),
            update_clients: Some(update_clients),
            sum2_clients: None,
            sum2_count,
        }
    }

    pub async fn run_sum_clients(&mut self) -> anyhow::Result<()> {
        let mut sum2_clients = ConcurrentFutures::<
            BoxFuture<'static, (StateMachine, mpsc::Receiver<Event>)>,
        >::new(100);

        let mut sum_clients = self
            .sum_clients
            .take()
            .ok_or_else(|| anyhow::anyhow!("No sum clients available"))?;

        let mut summer2 = 0;
        while let Some(sum_client) = sum_clients.next().await {
            if summer2 < self.sum2_count {
                sum2_clients.push(sum_client?);
                summer2 += 1;
            }
        }

        self.sum2_clients = Some(sum2_clients);

        Ok(())
    }

    pub async fn run_update_clients(&mut self) -> anyhow::Result<()> {
        let mut update_clients = self
            .update_clients
            .take()
            .ok_or_else(|| anyhow::anyhow!("No update clients available"))?;

        while update_clients.next().await.is_some() {}

        Ok(())
    }
    pub async fn run_sum2_clients(&mut self) -> anyhow::Result<()> {
        let mut sum2_clients = self
            .sum2_clients
            .take()
            .ok_or_else(|| anyhow::anyhow!("No sum2 clients available"))?;

        while sum2_clients.next().await.is_some() {}

        Ok(())
    }
}
