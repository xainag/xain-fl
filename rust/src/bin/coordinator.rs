#[macro_use]
extern crate tracing;

use clap::{App, Arg};
use rand::seq::IteratorRandom;
use std::process;
use tokio::signal::ctrl_c;
use tracing_futures::Instrument;

use xain_fl::{
    aggregator,
    common::{
        client::ClientId,
        logging,
        metric_store::influxdb::{run_metricstore, InfluxDBMetricStore},
    },
    coordinator::{
        api,
        core::{Selector, Service, ServiceHandle},
        rpc,
        settings::{
            ApiSettings, FederatedLearningSettings, MetricStoreSettings, RpcSettings, Settings,
        },
    },
};

#[tokio::main]
async fn main() {
    let matches = App::new("coordinator")
        .version("0.0.1")
        .about("XAIN FL coordinator service")
        .arg(
            Arg::with_name("config")
                .short("c")
                .takes_value(true)
                .required(true)
                .help("Path to the config file"),
        )
        .get_matches();
    let config_file = matches.value_of("config").unwrap();

    let settings = Settings::new(config_file).unwrap_or_else(|err| {
        eprintln!("Problem parsing configuration file: {}", err);
        process::exit(1);
    });

    let Settings {
        rpc,
        api,
        federated_learning,
        aggregator_url,
        metric_store,
        logging,
        ..
    } = settings;
    logging::configure(logging);

    let span = trace_span!("root");
    _main(rpc, api, federated_learning, aggregator_url, metric_store)
        .instrument(span)
        .await;
}

async fn _main(
    rpc: RpcSettings,
    api: ApiSettings,
    federated_learning: FederatedLearningSettings,
    aggregator_url: String,
    metric_store: MetricStoreSettings,
) {
    let (service_handle, service_requests) = ServiceHandle::new();

    // Start the RPC server
    let rpc_server = rpc::serve(rpc.bind_address.clone(), service_handle.clone())
        .instrument(trace_span!("rpc_server"));
    let rpc_server_task_handle = tokio::spawn(rpc_server);

    // Start the RPC client
    let rpc_client = aggregator::rpc::client_connect(rpc.aggregator_address.clone())
        .instrument(trace_span!("rpc_client"))
        .await
        .unwrap();

    // Start the metric store
    let (influx_client, metric_sender) = InfluxDBMetricStore::new(
        &metric_store.database_url[..],
        &metric_store.database_name[..],
    );

    let _ = tokio::spawn(async move { run_metricstore(influx_client).await });

    // Start the api server
    let api_server_task_handle = tokio::spawn(
        async move { api::serve(api.bind_address.as_str(), service_handle.clone()).await }
            .instrument(trace_span!("api_server")),
    );

    // Create the service
    let service = Service::new(
        RandomSelector,
        federated_learning,
        aggregator_url,
        rpc_client,
        service_requests,
        metric_sender,
    );

    // Run the service, and wait for one of the tasks to terminate
    tokio::select! {
        _ = service.instrument(trace_span!("service")) => {
            info!("shutting down: CoordinatorService terminated");
        }
        _ = api_server_task_handle => {
            info!("shutting down: API task terminated");
        }
        _ = rpc_server_task_handle => {
            info!("shutting down: RPC server task terminated");
        }
        result = ctrl_c() => {
            match result {
                Ok(()) => info!("shutting down: received SIGINT"),
                Err(e) => error!("shutting down: error while waiting for SIGINT: {}", e),

            }
        }
    }
}

pub struct RandomSelector;

impl Selector for RandomSelector {
    fn select(
        &mut self,
        min_count: usize,
        waiting: impl Iterator<Item = ClientId>,
        _selected: impl Iterator<Item = ClientId>,
    ) -> Vec<ClientId> {
        waiting.choose_multiple(&mut rand::thread_rng(), min_count)
    }
}
