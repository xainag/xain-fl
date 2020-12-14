use std::{path::PathBuf, process};

use structopt::StructOpt;
use tokio::signal;
use tracing::warn;
use tracing_subscriber::*;

#[cfg(feature = "metrics")]
use xaynet_server::{metrics, settings::InfluxSettings};

use xaynet_server::{
    rest::{serve, RestError},
    services,
    settings::{LoggingSettings, RedisSettings, Settings},
    state_machine::StateMachineInitializer,
    storage::{coordinator_storage::redis, Storage, Store},
};
#[cfg(feature = "iota")]
use xaynet_server::{settings::IotaSettings, storage::trust_anchor::iota::IotaClient};
#[cfg(feature = "model-persistence")]
use xaynet_server::{settings::S3Settings, storage::model_storage::s3};

#[derive(Debug, StructOpt)]
#[structopt(name = "Coordinator")]
struct Opt {
    /// Path of the configuration file
    #[structopt(short, parse(from_os_str))]
    config_path: PathBuf,
}

#[tokio::main]
async fn main() {
    let opt = Opt::from_args();

    let settings = Settings::new(opt.config_path).unwrap_or_else(|err| {
        eprintln!("{}", err);
        process::exit(1);
    });
    let Settings {
        pet: pet_settings,
        mask: mask_settings,
        api: api_settings,
        log: log_settings,
        model: model_settings,
        redis: redis_settings,
        ..
    } = settings;

    init_tracing(log_settings);

    // This should already called internally when instantiating the
    // state machine but it doesn't hurt making sure the crypto layer
    // is correctly initialized
    sodiumoxide::init().unwrap();

    #[cfg(feature = "metrics")]
    init_metrics(settings.metrics.influxdb);

    let store = init_store(
        redis_settings,
        #[cfg(feature = "model-persistence")]
        settings.s3,
        #[cfg(feature = "iota")]
        settings.trust_anchor.iota,
    )
    .await;

    let (state_machine, requests_tx, event_subscriber) = StateMachineInitializer::new(
        pet_settings,
        mask_settings,
        model_settings,
        #[cfg(feature = "model-persistence")]
        settings.restore,
        store,
    )
    .init()
    .await
    .expect("failed to initialize state machine");

    let fetcher = services::fetchers::fetcher(&event_subscriber);
    let message_handler =
        services::messages::PetMessageHandler::new(&event_subscriber, requests_tx);

    tokio::select! {
        _ = state_machine.run() => {
            warn!("shutting down: Service terminated");
        }
        result = serve(api_settings, fetcher, message_handler) => {
            match result {
                Ok(()) => warn!("shutting down: REST server terminated"),
                Err(RestError::InvalidTlsConfig) => {
                    warn!("shutting down: invalid TLS settings for REST server");
                },
            }
        }
        _ =  signal::ctrl_c() => {}
    }
}

fn init_tracing(settings: LoggingSettings) {
    let _fmt_subscriber = FmtSubscriber::builder()
        .with_env_filter(settings.filter)
        .with_ansi(true)
        .init();
}

#[cfg(feature = "metrics")]
fn init_metrics(settings: InfluxSettings) {
    let recorder = metrics::Recorder::new(settings);
    if metrics::GlobalRecorder::install(recorder).is_err() {
        warn!("failed to install metrics recorder");
    };
}

async fn init_store(
    redis_settings: RedisSettings,
    #[cfg(feature = "model-persistence")] s3_settings: S3Settings,
    #[cfg(feature = "iota")] iota_settings: IotaSettings,
) -> impl Storage {
    let coordinator_store = redis::Client::new(redis_settings.url)
        .await
        .expect("failed to establish a connection to Redis");

    let model_store = {
        #[cfg(not(feature = "model-persistence"))]
        {
            xaynet_server::storage::model_storage::noop::NoOp
        }

        #[cfg(feature = "model-persistence")]
        {
            let s3 = s3::Client::new(s3_settings).expect("failed to create S3 client");
            s3.create_global_models_bucket()
                .await
                .expect("failed to create bucket for global models");
            s3
        }
    };
    #[cfg(not(feature = "iota"))]
    {
        Store::new(coordinator_store, model_store)
    }

    #[cfg(feature = "iota")]
    {
        let trust_anchor = IotaClient::new(iota_settings)
            .await
            .expect("failed to create trust anchor");
        Store::new_with_trust_anchor(coordinator_store, model_store, trust_anchor)
    }
}
