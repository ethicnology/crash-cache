use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;

use crate::shared::persistence::sqlite::models::{UnwrapDeviceSpecsModel, NewUnwrapDeviceSpecsModel};
use crate::shared::persistence::sqlite::schema::unwrap_device_specs;

type SqlitePool = Pool<ConnectionManager<SqliteConnection>>;

/// Parameters for device specifications lookup/creation
#[derive(Default, Clone)]
pub struct DeviceSpecsParams {
    pub screen_width: Option<i32>,
    pub screen_height: Option<i32>,
    pub screen_density: Option<f32>,
    pub screen_dpi: Option<i32>,
    pub processor_count: Option<i32>,
    pub memory_size: Option<i64>,
    pub archs: Option<String>,
}

#[derive(Clone)]
pub struct DeviceSpecsRepository {
    pool: SqlitePool,
}

impl DeviceSpecsRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub fn get_or_create(&self, params: DeviceSpecsParams) -> Result<i32, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get connection");

        let mut query = unwrap_device_specs::table.into_boxed();

        query = match params.screen_width {
            Some(v) => query.filter(unwrap_device_specs::screen_width.eq(v)),
            None => query.filter(unwrap_device_specs::screen_width.is_null()),
        };
        query = match params.screen_height {
            Some(v) => query.filter(unwrap_device_specs::screen_height.eq(v)),
            None => query.filter(unwrap_device_specs::screen_height.is_null()),
        };
        query = match params.screen_density {
            Some(v) => query.filter(unwrap_device_specs::screen_density.eq(v)),
            None => query.filter(unwrap_device_specs::screen_density.is_null()),
        };
        query = match params.screen_dpi {
            Some(v) => query.filter(unwrap_device_specs::screen_dpi.eq(v)),
            None => query.filter(unwrap_device_specs::screen_dpi.is_null()),
        };
        query = match params.processor_count {
            Some(v) => query.filter(unwrap_device_specs::processor_count.eq(v)),
            None => query.filter(unwrap_device_specs::processor_count.is_null()),
        };
        query = match params.memory_size {
            Some(v) => query.filter(unwrap_device_specs::memory_size.eq(v)),
            None => query.filter(unwrap_device_specs::memory_size.is_null()),
        };
        query = match &params.archs {
            Some(v) => query.filter(unwrap_device_specs::archs.eq(v)),
            None => query.filter(unwrap_device_specs::archs.is_null()),
        };

        let existing = query
            .select(UnwrapDeviceSpecsModel::as_select())
            .first::<UnwrapDeviceSpecsModel>(&mut conn)
            .optional()?;

        if let Some(existing) = existing {
            return Ok(existing.id);
        }

        let new_record = NewUnwrapDeviceSpecsModel {
            screen_width: params.screen_width,
            screen_height: params.screen_height,
            screen_density: params.screen_density,
            screen_dpi: params.screen_dpi,
            processor_count: params.processor_count,
            memory_size: params.memory_size,
            archs: params.archs,
        };

        diesel::insert_into(unwrap_device_specs::table)
            .values(&new_record)
            .execute(&mut conn)?;

        let id = diesel::select(diesel::dsl::sql::<diesel::sql_types::Integer>(
            "last_insert_rowid()",
        ))
        .get_result::<i32>(&mut conn)?;

        Ok(id)
    }

    pub fn find_by_id(&self, id: i32) -> Result<Option<UnwrapDeviceSpecsModel>, diesel::result::Error> {
        let mut conn = self.pool.get().expect("Failed to get connection");

        unwrap_device_specs::table
            .filter(unwrap_device_specs::id.eq(id))
            .select(UnwrapDeviceSpecsModel::as_select())
            .first::<UnwrapDeviceSpecsModel>(&mut conn)
            .optional()
    }
}
