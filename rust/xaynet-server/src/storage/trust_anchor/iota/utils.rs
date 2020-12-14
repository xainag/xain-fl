use std::convert::TryFrom;

use iota::{client as iota_client, client::builder::Network};
use iota_streams::{
    app::transport::tangle::{
        client::{Client, SendTrytesOptions},
        PAYLOAD_BYTES,
    },
    app_channels::api::tangle::{Address, Author},
};

use super::client::AuthorState;
use crate::settings::IotaSettings;

fn network_to_min_weight_magnitude(network: &Network) -> u8 {
    match network {
        Network::Mainnet => 14,
        Network::Comnet => 10,
        Network::Devnet => 9,
    }
}

impl From<&IotaSettings> for SendTrytesOptions {
    fn from(settings: &IotaSettings) -> Self {
        let mut send_opt = SendTrytesOptions::default();
        send_opt.min_weight_magnitude = network_to_min_weight_magnitude(&settings.network);
        // we don't want to use local pow because there is a risk
        // that we would block the coordinator
        send_opt.local_pow = false;
        send_opt
    }
}

impl From<&IotaSettings> for iota_client::Client {
    fn from(settings: &IotaSettings) -> Self {
        iota_client::ClientBuilder::new()
            .node(&settings.url)
            // save unwrap: node cannot fail (until now)
            .unwrap()
            .network(settings.network.clone())
            .build()
            // save unwrap: node > 0, will fail if node == 0
            .unwrap()
    }
}

impl From<&IotaSettings> for Client {
    fn from(settings: &IotaSettings) -> Self {
        Client::new(
            SendTrytesOptions::from(settings),
            iota_client::Client::from(settings),
        )
    }
}

impl From<&IotaSettings> for Author<Client> {
    fn from(settings: &IotaSettings) -> Self {
        let client = Client::from(settings);
        Author::new(&settings.author_seed, "utf-8", PAYLOAD_BYTES, false, client)
    }
}

impl TryFrom<&AuthorState> for Address {
    type Error = anyhow::Error;

    fn try_from(state: &AuthorState) -> Result<Self, Self::Error> {
        Address::from_str(&state.announcement_message.0, &state.announcement_message.1).map_err(
            |_| {
                anyhow::anyhow!(
                    "creating a address from the string: {:?} failed",
                    state.announcement_message
                )
            },
        )
    }
}
