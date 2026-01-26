use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SentryAuth {
    pub sentry_key: Option<String>,
    pub sentry_secret: Option<String>,
    pub sentry_version: Option<String>,
    pub sentry_client: Option<String>,
    pub sentry_timestamp: Option<String>,
}

impl SentryAuth {
    pub fn from_header(header: &str) -> Option<Self> {
        let header = header.strip_prefix("Sentry ")?;

        let mut params: HashMap<String, String> = HashMap::new();

        for part in header.split(',') {
            let part = part.trim();
            if let Some((key, value)) = part.split_once('=') {
                params.insert(key.to_string(), value.to_string());
            }
        }

        Some(Self {
            sentry_key: params.get("sentry_key").cloned(),
            sentry_secret: params.get("sentry_secret").cloned(),
            sentry_version: params.get("sentry_version").cloned(),
            sentry_client: params.get("sentry_client").cloned(),
            sentry_timestamp: params.get("sentry_timestamp").cloned(),
        })
    }

    pub fn from_query_params(query: &str) -> Option<Self> {
        let mut params: HashMap<String, String> = HashMap::new();

        for part in query.split('&') {
            if let Some((key, value)) = part.split_once('=') {
                params.insert(key.to_string(), value.to_string());
            }
        }

        let sentry_key = params.get("sentry_key").cloned();
        sentry_key.as_ref()?;

        Some(Self {
            sentry_key,
            sentry_secret: params.get("sentry_secret").cloned(),
            sentry_version: params.get("sentry_version").cloned(),
            sentry_client: params.get("sentry_client").cloned(),
            sentry_timestamp: None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SentryDsn {
    pub public_key: String,
    pub secret_key: Option<String>,
    pub host: String,
    pub project_id: String,
}

impl SentryDsn {
    pub fn parse(dsn: &str) -> Option<Self> {
        let dsn = dsn.strip_prefix("http://").or_else(|| dsn.strip_prefix("https://"))?;

        let (auth_part, rest) = dsn.split_once('@')?;
        let (public_key, secret_key) = if auth_part.contains(':') {
            let (pk, sk) = auth_part.split_once(':')?;
            (pk.to_string(), Some(sk.to_string()))
        } else {
            (auth_part.to_string(), None)
        };

        let (host, project_id) = rest.rsplit_once('/')?;

        Some(Self {
            public_key,
            secret_key,
            host: host.to_string(),
            project_id: project_id.to_string(),
        })
    }
}
