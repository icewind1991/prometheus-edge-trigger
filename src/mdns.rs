use std::net::SocketAddr;
use std::time::Duration;

pub async fn resolve_mdns(
    service: &str,
    search_name: &str,
) -> Result<Option<SocketAddr>, mdns::Error> {
    Ok(mdns::resolve::one(
        service,
        &format!("{}.{}", search_name, service),
        Duration::from_secs(15),
    )
    .await?
    .and_then(|response| response.socket_address()))
}
