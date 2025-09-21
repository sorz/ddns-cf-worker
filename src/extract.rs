use std::{collections::HashSet, net::IpAddr};

use axum::http::{HeaderName, HeaderValue};
use axum_extra::headers::{self, Header};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct IpAddrs {
    #[serde(default)]
    pub(crate) ip: HashSet<IpAddr>,
}

static CF_CONNECTING_IP: HeaderName = HeaderName::from_static("cf-connecting-ip");

pub(crate) struct CfConnectingIp(pub(crate) IpAddr);

impl Header for CfConnectingIp {
    fn name() -> &'static HeaderName {
        &CF_CONNECTING_IP
    }

    fn decode<'i, I>(values: &mut I) -> Result<Self, headers::Error>
    where
        Self: Sized,
        I: Iterator<Item = &'i HeaderValue>,
    {
        let value = values
            .next()
            .ok_or_else(headers::Error::invalid)?
            .to_str()
            .map_err(|_| headers::Error::invalid())?
            .parse()
            .map_err(|_| headers::Error::invalid())?;
        Ok(Self(value))
    }

    fn encode<E: Extend<HeaderValue>>(&self, _values: &mut E) {
        unimplemented!()
    }
}
