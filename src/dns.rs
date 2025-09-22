use std::net::IpAddr;

use cloudflare::{
    endpoints::dns::dns::{
        CreateDnsRecord, CreateDnsRecordParams, DeleteDnsRecord, DnsContent, DnsRecord,
        ListDnsRecords, ListDnsRecordsParams, UpdateDnsRecord, UpdateDnsRecordParams,
    },
    framework::{auth::Credentials, client::async_api::Client, Environment},
};
use worker::Env;

use crate::error::UpdateResult;

static CF_API_TOKEN: &str = "CF_API_TOKEN";
static CF_ZONE_ID: &str = "CF_ZONE_ID";
static RECORD_TTL: &str = "RECORD_TTL";
static DEFAULT_RECORD_TTL: u32 = 300;

pub(crate) struct ZoneClient {
    client: Client,
    zone_id: String,
    record_ttl: u32,
}

impl ZoneClient {
    pub(crate) async fn new(env: Env) -> UpdateResult<Self> {
        let token = env.secret(CF_API_TOKEN)?.to_string();
        let zone_id = env.secret(CF_ZONE_ID)?.to_string();
        let record_ttl: u32 = match env.secret(RECORD_TTL) {
            Err(_) => DEFAULT_RECORD_TTL,
            Ok(value) => match value.to_string().parse() {
                Ok(value) => value,
                Err(err) => {
                    log::warn!("RECORD_TTL is not a valid number: {err}");
                    DEFAULT_RECORD_TTL
                }
            },
        };
        let client = Client::new(
            Credentials::UserAuthToken { token },
            Default::default(),
            Environment::Production,
        )?;
        Ok(Self {
            client,
            zone_id,
            record_ttl,
        })
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

    pub(crate) async fn create_record(&self, name: &str, ip_addr: IpAddr) -> UpdateResult<()> {
        log::debug!("Create {ip_addr} for {name}");
        let request: CreateDnsRecord<'_> = CreateDnsRecord {
            zone_identifier: &self.zone_id,
            params: CreateDnsRecordParams {
                ttl: Some(self.record_ttl),
                priority: None,
                proxied: None,
                name,
                content: match ip_addr {
                    IpAddr::V4(addr) => DnsContent::A { content: addr },
                    IpAddr::V6(addr) => DnsContent::AAAA { content: addr },
                },
            },
        };
        self.client.request(&request).await?;
        Ok(())
    }

    pub(crate) async fn update_record(
        &self,
        record: &DnsRecord,
        ip_addr: IpAddr,
    ) -> UpdateResult<()> {
        log::debug!(
            "Update {:?} => {} for {}",
            record.content,
            ip_addr,
            record.name
        );
        let request = UpdateDnsRecord {
            zone_identifier: &self.zone_id,
            identifier: &record.id,
            params: UpdateDnsRecordParams {
                ttl: Some(record.ttl),
                proxied: Some(record.proxied),
                name: &record.name,
                content: match ip_addr {
                    IpAddr::V4(addr) => DnsContent::A { content: addr },
                    IpAddr::V6(addr) => DnsContent::AAAA { content: addr },
                },
            },
        };
        self.client.request(&request).await?;
        Ok(())
    }

    pub(crate) async fn delete_record(&self, record: &DnsRecord) -> UpdateResult<()> {
        log::debug!("Delete {:?} from {}", record.content, record.name);
        let request = DeleteDnsRecord {
            zone_identifier: &self.zone_id,
            identifier: &record.id,
        };
        self.client.request(&request).await?;
        Ok(())
    }
}
