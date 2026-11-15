use hickory_resolver::config::{NameServerConfigGroup, ResolverConfig, ResolverOpts};
use hickory_resolver::TokioAsyncResolver;

use super::providers::Provider;
use super::{apply, check_with, Protocol};
use crate::error::DnsError;

pub async fn verify(provider: &Provider) -> Result<(), DnsError> {
    let servers = NameServerConfigGroup::from_ips_tls(
        provider.ips,
        provider.dot_port,
        provider.dot_host.to_string(),
        true,
    );
    let config = ResolverConfig::from_parts(None, Vec::new(), servers);
    let resolver = TokioAsyncResolver::tokio(config, ResolverOpts::default());
    check_with(&resolver, provider.dot_host, provider.name)
        .await
        .map(|_| ())
}

pub async fn set_dot(provider: &Provider) -> Result<(), DnsError> {
    verify(provider).await?;
    apply(provider, Protocol::Dot).await
}