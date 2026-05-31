use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Default)]
pub struct SessionData {
    pub values: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub data: SessionData,
    pub last_seen: Instant,
}

#[derive(Debug)]
pub struct SessionStore {
    sessions: HashMap<String, SessionEntry>,
    ttl: Duration,
    sequence: u64,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            ttl: Duration::from_secs(60 * 60),
            sequence: 0,
        }
    }

    pub fn get_or_create(&mut self, session_id: Option<&str>) -> (String, bool) {
        self.purge_expired();

        if let Some(id) = session_id {
            if let Some(entry) = self.sessions.get_mut(id) {
                entry.last_seen = Instant::now();
                return (id.to_string(), false);
            }
        }

        let id = self.next_session_id();
        self.sessions.insert(
            id.clone(),
            SessionEntry {
                data: SessionData::default(),
                last_seen: Instant::now(),
            },
        );
        (id, true)
    }

    pub fn get_mut(&mut self, session_id: &str) -> Option<&mut SessionData> {
        self.sessions.get_mut(session_id).map(|entry| {
            entry.last_seen = Instant::now();
            &mut entry.data
        })
    }

    pub fn purge_expired(&mut self) {
        let ttl = self.ttl;
        self.sessions.retain(|_, entry| entry.last_seen.elapsed() <= ttl);
    }

    fn next_session_id(&mut self) -> String {
        self.sequence = self.sequence.wrapping_add(1);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("s{}-{}", now, self.sequence)
    }
}

pub fn parse_cookie_header(raw: &str) -> HashMap<String, String> {
    let mut cookies = HashMap::new();
    for piece in raw.split(';') {
        let trimmed = piece.trim();
        if let Some((k, v)) = trimmed.split_once('=') {
            cookies.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    cookies
}
