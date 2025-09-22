use cloudflare::{
    endpoints::dns::dns::{DnsRecord, ListDnsRecords, ListDnsRecordsParams},
    framework::{auth::Credentials, client::async_api::Client, Environment},
};
use worker::Env;

use crate::error::UpdateResult;

static CF_API_TOKEN: &str = "CF_API_TOKEN";
static CF_ZONE_ID: &str = "CF_ZONE_ID";

pub(crate) struct ZoneClient {
    client: Client,
    zone_id: String,
}

impl ZoneClient {
    pub(crate) async fn new(env: Env) -> UpdateResult<Self> {
        let token = env.secret(CF_API_TOKEN)?.to_string();
        let zone_id = env.secret(CF_ZONE_ID)?.to_string();
        let client = Client::new(
            Credentials::UserAuthToken { token },
            Default::default(),
            Environment::Production,
        )?;
        Ok(Self { client, zone_id })
    }

    pub(crate) async fn list_records(&self, name: String) -> UpdateResult<Vec<DnsRecord>> {
        let requset = ListDnsRecords {
            zone_identifier: self.zone_id.as_ref(),
            params: ListDnsRecordsParams {
                name: Some(name),
                ..Default::default()
            },
        };
        let resp = self.client.request(&requset).await?;
        Ok(resp.result)
    }
}
