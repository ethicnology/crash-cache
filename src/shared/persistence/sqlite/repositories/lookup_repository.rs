use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::sqlite::SqliteConnection;

use crate::shared::persistence::sqlite::models::*;
use crate::shared::persistence::sqlite::schema::*;

type SqlitePool = Pool<ConnectionManager<SqliteConnection>>;

macro_rules! impl_lookup_repository {
    ($repo_name:ident, $table:ident, $model:ident, $new_model:ident) => {
        #[derive(Clone)]
        pub struct $repo_name {
            pool: SqlitePool,
        }

        impl $repo_name {
            pub fn new(pool: SqlitePool) -> Self {
                Self { pool }
            }

            pub fn get_or_create(&self, val: &str) -> Result<i32, diesel::result::Error> {
                let mut conn = self.pool.get().expect("Failed to get connection");

                if let Some(existing) = $table::table
                    .filter($table::value.eq(val))
                    .select($model::as_select())
                    .first::<$model>(&mut conn)
                    .optional()?
                {
                    return Ok(existing.id);
                }

                let new_record = $new_model {
                    value: val.to_string(),
                };

                diesel::insert_into($table::table)
                    .values(&new_record)
                    .execute(&mut conn)?;

                let inserted = $table::table
                    .filter($table::value.eq(val))
                    .select($model::as_select())
                    .first::<$model>(&mut conn)?;

                Ok(inserted.id)
            }

            pub fn find_by_id(&self, id: i32) -> Result<Option<$model>, diesel::result::Error> {
                let mut conn = self.pool.get().expect("Failed to get connection");

                $table::table
                    .filter($table::id.eq(id))
                    .select($model::as_select())
                    .first::<$model>(&mut conn)
                    .optional()
            }

            pub fn find_by_value(&self, val: &str) -> Result<Option<$model>, diesel::result::Error> {
                let mut conn = self.pool.get().expect("Failed to get connection");

                $table::table
                    .filter($table::value.eq(val))
                    .select($model::as_select())
                    .first::<$model>(&mut conn)
                    .optional()
            }
        }
    };
}

impl_lookup_repository!(LookupPlatformRepository, lookup_platform, LookupPlatformModel, NewLookupPlatformModel);
impl_lookup_repository!(LookupEnvironmentRepository, lookup_environment, LookupEnvironmentModel, NewLookupEnvironmentModel);
impl_lookup_repository!(LookupConnectionTypeRepository, lookup_connection_type, LookupConnectionTypeModel, NewLookupConnectionTypeModel);
impl_lookup_repository!(LookupOrientationRepository, lookup_orientation, LookupOrientationModel, NewLookupOrientationModel);
impl_lookup_repository!(LookupOsNameRepository, lookup_os_name, LookupOsNameModel, NewLookupOsNameModel);
impl_lookup_repository!(LookupOsVersionRepository, lookup_os_version, LookupOsVersionModel, NewLookupOsVersionModel);
impl_lookup_repository!(LookupManufacturerRepository, lookup_manufacturer, LookupManufacturerModel, NewLookupManufacturerModel);
impl_lookup_repository!(LookupBrandRepository, lookup_brand, LookupBrandModel, NewLookupBrandModel);
impl_lookup_repository!(LookupModelRepository, lookup_model, LookupModelModel, NewLookupModelModel);
impl_lookup_repository!(LookupChipsetRepository, lookup_chipset, LookupChipsetModel, NewLookupChipsetModel);
impl_lookup_repository!(LookupLocaleCodeRepository, lookup_locale_code, LookupLocaleCodeModel, NewLookupLocaleCodeModel);
impl_lookup_repository!(LookupTimezoneRepository, lookup_timezone, LookupTimezoneModel, NewLookupTimezoneModel);
impl_lookup_repository!(LookupAppNameRepository, lookup_app_name, LookupAppNameModel, NewLookupAppNameModel);
impl_lookup_repository!(LookupAppVersionRepository, lookup_app_version, LookupAppVersionModel, NewLookupAppVersionModel);
impl_lookup_repository!(LookupAppBuildRepository, lookup_app_build, LookupAppBuildModel, NewLookupAppBuildModel);
impl_lookup_repository!(LookupUserRepository, lookup_user, LookupUserModel, NewLookupUserModel);
impl_lookup_repository!(LookupExceptionTypeRepository, lookup_exception_type, LookupExceptionTypeModel, NewLookupExceptionTypeModel);
