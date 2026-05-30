use crate::config::{Config, RouteConfig, ServerConfig};

pub fn select_server<'a>(
    config: &'a Config,
    local_host: &str,
    local_port: u16,
    host_header: Option<&str>,
) -> Option<&'a ServerConfig> {
    let mut candidates: Vec<&ServerConfig> = config
        .servers
        .iter()
        .filter(|s| s.host == local_host && s.ports.contains(&local_port))
        .collect();

    if candidates.is_empty() {
        candidates = config
            .servers
            .iter()
            .filter(|s| s.ports.contains(&local_port))
            .collect();
    }

    if candidates.is_empty() {
        return None;
    }

    let normalized_host = host_header.map(normalize_host_header);

    if let Some(host) = normalized_host {
        if let Some(named) = candidates
            .iter()
            .find(|s| s.server_name.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(&host)))
        {
            return Some(named);
        }
    }

    candidates
        .iter()
        .find(|s| s.server_name.is_none())
        .copied()
        .or_else(|| candidates.first().copied())
}

pub fn match_route<'a>(server: &'a ServerConfig, path: &str) -> Option<&'a RouteConfig> {
    server.match_route(path_without_query(path))
}

fn normalize_host_header(raw: &str) -> String {
    raw.split(':').next().unwrap_or(raw).trim().to_ascii_lowercase()
}

fn path_without_query(path: &str) -> &str {
    path.split('?').next().unwrap_or(path)
}
