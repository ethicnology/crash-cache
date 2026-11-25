use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Double, Integer, Text, Timestamp},
};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

use crate::shared::{parser::Envelope, persistence::DbConnection};

struct Point {
    bucket: NaiveDateTime,
    timestamp: DateTime<Utc>,
    name: String,
    release: String,
    environment: String,
    from_schema: i32,
    to_schema: i32,
    chain: String,
    outcome: String,
    destination_network: String,
    direction: String,
    value: f64,
}

fn keys(map: &Map<String, Value>, allowed: &[&str]) -> bool {
    map.keys().all(|key| allowed.contains(&key.as_str()))
}

fn string_attr<'a>(attrs: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    let attr = attrs.get(key)?.as_object()?;
    if attr.len() != 2 || attr.get("type")?.as_str()? != "string" {
        return None;
    }
    attr.get("value")?.as_str()
}

fn release_valid(s: &str) -> bool {
    let Some((version, build)) = s.split_once('+') else {
        return false;
    };
    let part = |s: &str| !s.is_empty() && s.len() <= 6 && s.bytes().all(|c| c.is_ascii_digit());
    let versions: Vec<_> = version.split('.').collect();
    versions.len() == 3 && versions.into_iter().all(part) && part(build)
}

fn timestamp(value: &Value) -> Option<DateTime<Utc>> {
    if let Some(s) = value.as_str() {
        return DateTime::parse_from_rfc3339(s).ok().map(|x| x.to_utc());
    }
    let n = value.as_f64()?;
    if !n.is_finite() || n < 0.0 || n > i64::MAX as f64 {
        return None;
    }
    let seconds = n.floor() as i64;
    let nanos = ((n - seconds as f64) * 1_000_000_000.0) as u32;
    DateTime::from_timestamp(seconds, nanos)
}

fn validate(envelope: &Envelope, now: DateTime<Utc>) -> Option<Vec<Point>> {
    if envelope.items.len() != 1 {
        return None;
    }
    let item = &envelope.items[0];
    if item.header.item_type != "trace_metric"
        || item.header.content_type.as_deref()
            != Some("application/vnd.sentry.items.trace-metric+json")
        || item.header.extra.keys().any(|key| key != "item_count")
    {
        return None;
    }
    let count = item.header.extra.get("item_count")?.as_u64()? as usize;
    let payload: Value = serde_json::from_slice(&item.payload).ok()?;
    let body = payload.as_object()?;
    if !keys(body, &["items"]) {
        return None;
    }
    let items = body.get("items")?.as_array()?;
    if items.is_empty() || items.len() != count || items.len() > 1000 {
        return None;
    }
    let mut points = Vec::with_capacity(items.len());
    for item in items {
        let item = item.as_object()?;
        if !keys(
            item,
            &[
                "timestamp",
                "trace_id",
                "type",
                "name",
                "value",
                "attributes",
            ],
        ) {
            return None;
        }
        let trace = item.get("trace_id")?.as_str()?;
        if trace.len() != 32
            || !trace.bytes().all(|b| b.is_ascii_hexdigit())
            || trace.bytes().all(|b| b == b'0')
        {
            return None;
        }
        let time = timestamp(item.get("timestamp")?)?;
        if time < now - Duration::days(30) || time > now + Duration::minutes(5) {
            return None;
        }
        let attrs = item.get("attributes")?.as_object()?;
        let release = string_attr(attrs, "sentry.release")?;
        if !release_valid(release) {
            return None;
        }
        let environment = string_attr(attrs, "sentry.environment")?;
        if !["regtest", "testnet", "mainnet"].contains(&environment) {
            return None;
        }
        let name = item.get("name")?.as_str()?;
        let kind = item.get("type")?.as_str()?;
        let value = item.get("value")?.as_f64()?;
        if kind != "counter" || value != 1.0 {
            return None;
        }
        if name != "bull.installation.active"
            || !keys(attrs, &["sentry.release", "sentry.environment"])
        {
            return None;
        }
        let seconds = time.timestamp().div_euclid(3600) * 3600;
        let bucket = DateTime::from_timestamp(seconds, 0)?.naive_utc();
        points.push(Point {
            bucket,
            timestamp: time,
            name: name.to_owned(),
            release: release.to_owned(),
            environment: environment.to_owned(),
            from_schema: -1,
            to_schema: -1,
            chain: String::new(),
            outcome: String::new(),
            destination_network: String::new(),
            direction: String::new(),
            value,
        });
    }
    Some(points)
}

pub(super) fn ingest(
    conn: &mut DbConnection,
    project: i32,
    digest: &str,
    envelope: &Envelope,
) -> Result<bool, MetricError> {
    conn.transaction::<bool, MetricError, _>(|conn| {
        // Cleanup must not expire a replay marker between validation and insertion.
        diesel::sql_query("SELECT pg_advisory_xact_lock_shared(733240121)").execute(conn)?;
        let points = validate(envelope, Utc::now()).ok_or(MetricError::Invalid)?;
        let buckets: BTreeSet<_> = points.iter().map(|p| p.bucket).collect();
        let expires = points.iter().map(|p| p.timestamp).max().ok_or(MetricError::Invalid)? + Duration::days(30);
        // All metric writers for a project serialize before inspecting hourly cardinality.
        diesel::sql_query("SELECT pg_advisory_xact_lock(733240122, $1)")
            .bind::<Integer, _>(project).execute(conn)?;
        let inserted = diesel::sql_query("INSERT INTO metric_replay (project_id, digest, expires_at) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING")
            .bind::<Integer, _>(project).bind::<Text, _>(digest).bind::<Timestamp, _>(expires.naive_utc()).execute(conn)?;
        if inserted == 0 { return Ok(false); }
        for p in points {
            diesel::sql_query(
                "INSERT INTO metric_hour (
                    project_id, bucket_start, name, release, environment, from_schema, to_schema,
                    chain, outcome, destination_network, direction, sample_count, value_sum,
                    value_min, value_max
                 ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,1,$12,$12,$12)
                 ON CONFLICT (project_id,bucket_start,name,release,environment,from_schema,to_schema,
                              chain,outcome,destination_network,direction)
                 DO UPDATE SET sample_count=metric_hour.sample_count+1,
                               value_sum=metric_hour.value_sum+EXCLUDED.value_sum",
            )
            .bind::<Integer,_>(project).bind::<Timestamp,_>(p.bucket).bind::<Text,_>(&p.name)
            .bind::<Text,_>(&p.release).bind::<Text,_>(&p.environment).bind::<Integer,_>(p.from_schema)
            .bind::<Integer,_>(p.to_schema).bind::<Text,_>(&p.chain).bind::<Text,_>(&p.outcome)
            .bind::<Text,_>(&p.destination_network).bind::<Text,_>(&p.direction)
            .bind::<Double,_>(p.value).execute(conn)?;
        }
        #[derive(QueryableByName)]
        struct SeriesCount {
            #[diesel(sql_type = BigInt)]
            count: i64,
        }
        for bucket in buckets {
            let count: SeriesCount = diesel::sql_query(
                "SELECT COUNT(*) AS count FROM metric_hour WHERE project_id=$1 AND bucket_start=$2",
            ).bind::<Integer, _>(project).bind::<Timestamp, _>(bucket).get_result(conn)?;
            if count.count > 1024 {
                return Err(MetricError::SeriesLimit);
            }
        }
        Ok(true)
    })
}

pub(super) enum MetricError {
    Invalid,
    SeriesLimit,
    Database,
}

impl From<diesel::result::Error> for MetricError {
    fn from(_: diesel::result::Error) -> Self {
        Self::Database
    }
}
