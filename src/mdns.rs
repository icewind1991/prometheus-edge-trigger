use tokio::time::timeout;
use futures_util::{pin_mut, stream::StreamExt};
use mdns::{Record, RecordKind};
use std::{net::IpAddr, time::Duration};

const INTERVAL: Duration = Duration::from_secs(3);

#[derive(Debug)]
pub struct MdnsService {
    pub id: String,
    pub name: String,
    pub addr: IpAddr,
    pub port: u16,
}

pub async fn resolve_mdns(service: &str, search_name: &str) -> Result<Option<MdnsService>, mdns::Error> {
    let stream = mdns::discover::all(service, INTERVAL)?.listen();
    pin_mut!(stream);

    let process = async move {
        while let Some(Ok(response)) = stream.next().await {
            let id = response.records().find_map(to_id);
            let addr = response.records().find_map(to_ip_addr);
            let port = response.records().find_map(to_port);
            let name = response.records().find_map(to_name);

            if let (Some(id), Some(addr), Some(name), Some(port)) = (id, addr, name, port) {
                let service = MdnsService {
                    id,
                    name,
                    addr,
                    port,
                };

                if service.id == search_name {
                    return Some(service);
                }
            }
        }
        None
    };

    match timeout(Duration::from_secs(5), process).await {
        Err(_) => Ok(None),
        Ok(res) => Ok(res)
    }
}

fn to_id(record: &Record) -> Option<String> {
    match &record.kind {
        RecordKind::PTR(id) => {
            id.split('.').next().map(|s| s.to_string())
        }
        _ => None,
    }
}

fn to_ip_addr(record: &Record) -> Option<IpAddr> {
    match record.kind {
        RecordKind::A(addr) => Some(addr.into()),
        RecordKind::AAAA(addr) => Some(addr.into()),
        _ => None,
    }
}

fn to_port(record: &Record) -> Option<u16> {
    match record.kind {
        RecordKind::SRV { port, .. } => Some(port),
        _ => None,
    }
}

fn to_name(record: &Record) -> Option<String> {
    if let RecordKind::TXT(txt) = &record.kind {
        txt.iter()
            .find_map(|pair| {
                let mut parts = pair.split('=');
                if let (Some("name"), Some(value)) = (parts.next(), parts.next()) {
                    Some(value.to_string())
                } else {
                    None
                }
            })
    } else {
        None
    }
}