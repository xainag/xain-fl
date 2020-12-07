use e2e::{
    test_client::builder::{TestClientBuilder, TestClientBuilderSettings},
    test_env::{utils, TestEnvironment, TestEnvironmentSettings},
};
use tokio::{
    signal,
    time::{timeout, Duration},
};
use tracing::info;
use xaynet_server::state_machine::phases::PhaseName;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let env_settings = TestEnvironmentSettings::from_file("src/bin/test_case_1")?;
    let env = TestEnvironment::new(env_settings.clone()).await?;

    tokio::select! {
        res = timeout(Duration::from_secs(6000), run(env)) => {
            res?
        }
        _ =  signal::ctrl_c() => { Ok(()) }
    }
}

async fn run(mut env: TestEnvironment) -> anyhow::Result<()> {
    let k8s = env.get_k8s_client().await?;
    k8s.deploy_with_config(env.get_env_settings().coordinator.config)
        .await?;
    let handle = k8s
        .save_coordinator_logs("src/bin/test_case_1/coordinator.log")
        .await?;

    let _pfi_guard = k8s.port_forward_influxdb()?;
    let _pfc_guard = if env.get_env_settings().api_client.certificates.is_none() {
        Some(k8s.port_forward_coordinator().await?)
    } else {
        None
    };

    let mut api_client = env.get_api_client()?;
    let mut influx_client = env.get_influx_client();

    info!("wait until clients are ready");
    let _ = tokio::join!(
        utils::wait_until_client_is_ready(&mut api_client),
        utils::wait_until_client_is_ready(&mut influx_client),
    );
    utils::wait_until_phase(&influx_client, PhaseName::Sum).await;

    ////////////////////////////////////////////////////////////////////////////////////////////////

    let coordinator_settings = env.get_coordinator_settings()?;
    let test_client_builder_settings = TestClientBuilderSettings::from(coordinator_settings);

    let mut test_client_builder = TestClientBuilder::new(test_client_builder_settings, api_client);

    ////////////////////////////////////////////////////////////////////////////////////////////////

    for round in 0..10 {
        info!("Round: {}", round);

        let mut runner = test_client_builder.build_clients().await?;
        info!("run sum clients...");
        runner.run_sum_clients().await?;
        utils::wait_until_phase(&influx_client, PhaseName::Update).await;
        info!("run update clients...");
        runner.run_update_clients().await?;
        utils::wait_until_phase(&influx_client, PhaseName::Sum2).await;
        info!("run sum2 clients...");
        runner.run_sum2_clients().await?;
        utils::wait_until_phase(&influx_client, PhaseName::Sum).await;
    }

    ////////////////////////////////////////////////////////////////////////////////////////////////

    timeout(Duration::from_secs(10), handle).await???;
    Ok(())
}
