use super::DbPool;
use chrono::{NaiveDateTime, Timelike, Utc};
use diesel::prelude::*;

use crate::shared::persistence::db::models::{
    NewBucketRateLimitDsnModel, NewBucketRateLimitGlobalModel, NewBucketRateLimitSubnetModel,
    NewBucketRequestLatencyModel,
};
use crate::shared::persistence::db::schema::{
    bucket_rate_limit_dsn, bucket_rate_limit_global, bucket_rate_limit_subnet,
    bucket_request_latency,
};

#[derive(Clone)]
pub struct AnalyticsRepository {
    pool: DbPool,
}

impl AnalyticsRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    fn bucket_start() -> NaiveDateTime {
        let now = Utc::now().naive_utc();
        now.with_second(0).unwrap().with_nanosecond(0).unwrap()
    }

    pub fn record_rate_limit_global(&self) -> Result<(), diesel::result::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|_| diesel::result::Error::BrokenTransactionManager)?;
        let bucket = Self::bucket_start();

        diesel::insert_into(bucket_rate_limit_global::table)
            .values(NewBucketRateLimitGlobalModel {
                bucket_start: bucket,
                hit_count: 1,
            })
            .on_conflict(bucket_rate_limit_global::bucket_start)
            .do_update()
            .set(bucket_rate_limit_global::hit_count.eq(bucket_rate_limit_global::hit_count + 1))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn record_rate_limit_dsn(
        &self,
        dsn: &str,
        project_id: Option<i32>,
    ) -> Result<(), diesel::result::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|_| diesel::result::Error::BrokenTransactionManager)?;
        let bucket = Self::bucket_start();

        diesel::insert_into(bucket_rate_limit_dsn::table)
            .values(NewBucketRateLimitDsnModel {
                dsn: dsn.to_string(),
                project_id,
                bucket_start: bucket,
                hit_count: 1,
            })
            .on_conflict((
                bucket_rate_limit_dsn::dsn,
                bucket_rate_limit_dsn::bucket_start,
            ))
            .do_update()
            .set(bucket_rate_limit_dsn::hit_count.eq(bucket_rate_limit_dsn::hit_count + 1))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn record_rate_limit_subnet(&self, ip: &str) -> Result<(), diesel::result::Error> {
        let subnet = Self::ip_to_subnet(ip);
        let mut conn = self
            .pool
            .get()
            .map_err(|_| diesel::result::Error::BrokenTransactionManager)?;
        let bucket = Self::bucket_start();

        diesel::insert_into(bucket_rate_limit_subnet::table)
            .values(NewBucketRateLimitSubnetModel {
                subnet,
                bucket_start: bucket,
                hit_count: 1,
            })
            .on_conflict((
                bucket_rate_limit_subnet::subnet,
                bucket_rate_limit_subnet::bucket_start,
            ))
            .do_update()
            .set(bucket_rate_limit_subnet::hit_count.eq(bucket_rate_limit_subnet::hit_count + 1))
            .execute(&mut conn)?;

        Ok(())
    }

    pub fn record_request_latency(
        &self,
        endpoint: &str,
        latency_ms: u32,
    ) -> Result<(), diesel::result::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|_| diesel::result::Error::BrokenTransactionManager)?;
        let bucket = Self::bucket_start();
        let latency = latency_ms as i32;

        let existing = bucket_request_latency::table
            .filter(bucket_request_latency::endpoint.eq(endpoint))
            .filter(bucket_request_latency::bucket_start.eq(bucket))
            .select((
                bucket_request_latency::min_ms,
                bucket_request_latency::max_ms,
            ))
            .first::<(Option<i32>, Option<i32>)>(&mut conn)
            .optional()?;

        match existing {
            Some((current_min, current_max)) => {
                let new_min = current_min.map(|m| m.min(latency)).unwrap_or(latency);
                let new_max = current_max.map(|m| m.max(latency)).unwrap_or(latency);

                diesel::update(bucket_request_latency::table)
                    .filter(bucket_request_latency::endpoint.eq(endpoint))
                    .filter(bucket_request_latency::bucket_start.eq(bucket))
                    .set((
                        bucket_request_latency::request_count
                            .eq(bucket_request_latency::request_count + 1),
                        bucket_request_latency::total_ms
                            .eq(bucket_request_latency::total_ms + latency),
                        bucket_request_latency::min_ms.eq(new_min),
                        bucket_request_latency::max_ms.eq(new_max),
                    ))
                    .execute(&mut conn)?;
            }
            None => {
                diesel::insert_into(bucket_request_latency::table)
                    .values(NewBucketRequestLatencyModel {
                        endpoint: endpoint.to_string(),
                        bucket_start: bucket,
                        request_count: 1,
                        total_ms: latency,
                        min_ms: Some(latency),
                        max_ms: Some(latency),
                    })
                    .execute(&mut conn)?;
            }
        }

        Ok(())
    }

    pub fn cleanup_old_buckets(&self, retention_days: i64) -> Result<usize, diesel::result::Error> {
        let mut conn = self
            .pool
            .get()
            .map_err(|_| diesel::result::Error::BrokenTransactionManager)?;
        let cutoff = Utc::now().naive_utc() - chrono::Duration::days(retention_days);

        let mut total = 0;

        total += diesel::delete(
            bucket_rate_limit_global::table
                .filter(bucket_rate_limit_global::bucket_start.lt(cutoff)),
        )
        .execute(&mut conn)?;

        total += diesel::delete(
            bucket_rate_limit_dsn::table.filter(bucket_rate_limit_dsn::bucket_start.lt(cutoff)),
        )
        .execute(&mut conn)?;

        total += diesel::delete(
            bucket_rate_limit_subnet::table
                .filter(bucket_rate_limit_subnet::bucket_start.lt(cutoff)),
        )
        .execute(&mut conn)?;

        total += diesel::delete(
            bucket_request_latency::table.filter(bucket_request_latency::bucket_start.lt(cutoff)),
        )
        .execute(&mut conn)?;

        Ok(total)
    }

    fn ip_to_subnet(ip: &str) -> String {
        let parts: Vec<&str> = ip.split('.').collect();
        if parts.len() >= 3 {
            format!("{}.{}.{}", parts[0], parts[1], parts[2])
        } else if ip.contains(':') {
            let parts: Vec<&str> = ip.split(':').collect();
            if parts.len() >= 4 {
                format!("{}:{}:{}:{}", parts[0], parts[1], parts[2], parts[3])
            } else {
                ip.to_string()
            }
        } else {
            ip.to_string()
        }
    }
}
