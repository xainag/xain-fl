use chrono::{DateTime, Utc};
use influxdb::{integrations::serde_integration::DatabaseQueryResult, Client, Query};
use xaynet_server::{settings::InfluxSettings, state_machine::phases::PhaseName};
#[derive(Clone)]
pub struct InfluxClient {
    client: Client,
}

impl InfluxClient {
    pub fn new(settings: InfluxSettings) -> Self {
        Self {
            client: Client::new(settings.url, settings.db),
        }
    }

    pub async fn ping(&self) -> anyhow::Result<()> {
        self.client
            .ping()
            .await
            .map_err(|err| anyhow::anyhow!(err))?;
        Ok(())
    }

    pub async fn get_current_phase(&self) -> anyhow::Result<PhaseName> {
        let read_query = Query::raw_read_query("SELECT LAST(value) FROM phase GROUP BY *");
        let read_result = self
            .client
            .json_query(read_query)
            .await
            .map_err(|err| anyhow::anyhow!(err))?;
        deserialize_phase(read_result)
    }
}

fn deserialize_phase(mut read_result: DatabaseQueryResult) -> anyhow::Result<PhaseName> {
    let phase = read_result
        .deserialize_next::<PhaseReading>()
        .map_err(|err| anyhow::anyhow!("no phase: {}", err))?
        .series
        .first()
        .ok_or_else(|| anyhow::anyhow!("no phase"))?
        .values
        .first()
        .ok_or_else(|| anyhow::anyhow!("no phase"))?
        .last;
    Ok(from_u8(phase)?)
}

fn from_u8(phase: u8) -> anyhow::Result<PhaseName> {
    let phase_name = match phase {
        0 => PhaseName::Idle,
        1 => PhaseName::Sum,
        2 => PhaseName::Update,
        3 => PhaseName::Sum2,
        4 => PhaseName::Unmask,
        5 => PhaseName::Error,
        6 => PhaseName::Shutdown,
        _ => return Err(anyhow::anyhow!("unknown phase")),
    };
    Ok(phase_name)
}

#[derive(Debug, serde::Deserialize)]
pub struct PhaseReading {
    time: DateTime<Utc>,
    last: u8,
}
