use super::DbPool;
use crate::shared::domain::DomainError;
use crate::shared::persistence::db::models::*;
use crate::shared::persistence::db::schema::*;
use diesel::prelude::*;

macro_rules! impl_unwrap_repository {
    ($repo_name:ident, $table:ident, $model:ident, $new_model:ident) => {
        #[derive(Clone)]
        pub struct $repo_name {
            pool: DbPool,
        }

        impl $repo_name {
            pub fn new(pool: DbPool) -> Self {
                Self { pool }
            }

            pub fn get_or_create(&self, val: &str) -> Result<i32, DomainError> {
                let mut conn = self.pool.get().map_err(|e| {
                    DomainError::ConnectionPool(format!("Connection pool error: {}", e))
                })?;

                if let Some(existing) = $table::table
                    .filter($table::value.eq(val))
                    .select($model::as_select())
                    .first::<$model>(&mut conn)
                    .optional()
                    .map_err(|e| DomainError::Database(e.to_string()))?
                {
                    return Ok(existing.id);
                }

                let new_record = $new_model {
                    value: val.to_string(),
                };

                diesel::insert_into($table::table)
                    .values(&new_record)
                    .execute(&mut conn)
                    .map_err(|e| DomainError::Database(e.to_string()))?;

                let inserted = $table::table
                    .filter($table::value.eq(val))
                    .select($model::as_select())
                    .first::<$model>(&mut conn)
                    .map_err(|e| DomainError::Database(e.to_string()))?;

                Ok(inserted.id)
            }

            pub fn find_by_id(&self, id: i32) -> Result<Option<$model>, DomainError> {
                let mut conn = self.pool.get().map_err(|e| {
                    DomainError::ConnectionPool(format!("Connection pool error: {}", e))
                })?;

                $table::table
                    .filter($table::id.eq(id))
                    .select($model::as_select())
                    .first::<$model>(&mut conn)
                    .optional()
                    .map_err(|e| DomainError::Database(e.to_string()))
            }

            pub fn find_by_value(&self, val: &str) -> Result<Option<$model>, DomainError> {
                let mut conn = self.pool.get().map_err(|e| {
                    DomainError::ConnectionPool(format!("Connection pool error: {}", e))
                })?;

                $table::table
                    .filter($table::value.eq(val))
                    .select($model::as_select())
                    .first::<$model>(&mut conn)
                    .optional()
                    .map_err(|e| DomainError::Database(e.to_string()))
            }
        }
    };
}

impl_unwrap_repository!(
    UnwrapPlatformRepository,
    unwrap_platform,
    UnwrapPlatformModel,
    NewUnwrapPlatformModel
);
impl_unwrap_repository!(
    UnwrapEnvironmentRepository,
    unwrap_environment,
    UnwrapEnvironmentModel,
    NewUnwrapEnvironmentModel
);
impl_unwrap_repository!(
    UnwrapConnectionTypeRepository,
    unwrap_connection_type,
    UnwrapConnectionTypeModel,
    NewUnwrapConnectionTypeModel
);
impl_unwrap_repository!(
    UnwrapOrientationRepository,
    unwrap_orientation,
    UnwrapOrientationModel,
    NewUnwrapOrientationModel
);
impl_unwrap_repository!(
    UnwrapOsNameRepository,
    unwrap_os_name,
    UnwrapOsNameModel,
    NewUnwrapOsNameModel
);
impl_unwrap_repository!(
    UnwrapOsVersionRepository,
    unwrap_os_version,
    UnwrapOsVersionModel,
    NewUnwrapOsVersionModel
);
impl_unwrap_repository!(
    UnwrapManufacturerRepository,
    unwrap_manufacturer,
    UnwrapManufacturerModel,
    NewUnwrapManufacturerModel
);
impl_unwrap_repository!(
    UnwrapBrandRepository,
    unwrap_brand,
    UnwrapBrandModel,
    NewUnwrapBrandModel
);
impl_unwrap_repository!(
    UnwrapModelRepository,
    unwrap_model,
    UnwrapModelModel,
    NewUnwrapModelModel
);
impl_unwrap_repository!(
    UnwrapChipsetRepository,
    unwrap_chipset,
    UnwrapChipsetModel,
    NewUnwrapChipsetModel
);
impl_unwrap_repository!(
    UnwrapLocaleCodeRepository,
    unwrap_locale_code,
    UnwrapLocaleCodeModel,
    NewUnwrapLocaleCodeModel
);
impl_unwrap_repository!(
    UnwrapTimezoneRepository,
    unwrap_timezone,
    UnwrapTimezoneModel,
    NewUnwrapTimezoneModel
);
impl_unwrap_repository!(
    UnwrapAppNameRepository,
    unwrap_app_name,
    UnwrapAppNameModel,
    NewUnwrapAppNameModel
);
impl_unwrap_repository!(
    UnwrapAppVersionRepository,
    unwrap_app_version,
    UnwrapAppVersionModel,
    NewUnwrapAppVersionModel
);
impl_unwrap_repository!(
    UnwrapAppBuildRepository,
    unwrap_app_build,
    UnwrapAppBuildModel,
    NewUnwrapAppBuildModel
);
impl_unwrap_repository!(
    UnwrapUserRepository,
    unwrap_user,
    UnwrapUserModel,
    NewUnwrapUserModel
);
impl_unwrap_repository!(
    UnwrapExceptionTypeRepository,
    unwrap_exception_type,
    UnwrapExceptionTypeModel,
    NewUnwrapExceptionTypeModel
);
