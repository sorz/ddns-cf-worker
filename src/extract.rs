use std::{collections::HashSet, net::IpAddr};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct IpAddrs {
    #[serde(default)]
    pub(crate) ip: HashSet<IpAddr>,
}
