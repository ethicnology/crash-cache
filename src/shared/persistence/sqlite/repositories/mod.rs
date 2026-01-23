mod archive_repository;
mod device_specs_repository;
mod exception_message_repository;
mod issue_repository;
mod lookup_repository;
mod project_repository;
mod queue_repository;
mod report_repository;
mod stacktrace_repository;

pub use archive_repository::ArchiveRepository;
pub use device_specs_repository::DeviceSpecsRepository;
pub use exception_message_repository::ExceptionMessageRepository;
pub use issue_repository::IssueRepository;
pub use lookup_repository::*;
pub use project_repository::ProjectRepository;
pub use queue_repository::QueueRepository;
pub use report_repository::{NewReport, ReportRepository};
pub use stacktrace_repository::StacktraceRepository;

use super::SqlitePool;

#[derive(Clone)]
pub struct Repositories {
    pub archive: ArchiveRepository,
    pub queue: QueueRepository,
    pub project: ProjectRepository,
    pub report: ReportRepository,
    pub platform: LookupPlatformRepository,
    pub environment: LookupEnvironmentRepository,
    pub os_name: LookupOsNameRepository,
    pub os_version: LookupOsVersionRepository,
    pub manufacturer: LookupManufacturerRepository,
    pub brand: LookupBrandRepository,
    pub model: LookupModelRepository,
    pub chipset: LookupChipsetRepository,
    pub device_specs: DeviceSpecsRepository,
    pub locale_code: LookupLocaleCodeRepository,
    pub timezone: LookupTimezoneRepository,
    pub connection_type: LookupConnectionTypeRepository,
    pub orientation: LookupOrientationRepository,
    pub app_name: LookupAppNameRepository,
    pub app_version: LookupAppVersionRepository,
    pub app_build: LookupAppBuildRepository,
    pub user: LookupUserRepository,
    pub exception_type: LookupExceptionTypeRepository,
    pub exception_message: ExceptionMessageRepository,
    pub stacktrace: StacktraceRepository,
    pub issue: IssueRepository,
}

impl Repositories {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            archive: ArchiveRepository::new(pool.clone()),
            queue: QueueRepository::new(pool.clone()),
            project: ProjectRepository::new(pool.clone()),
            report: ReportRepository::new(pool.clone()),
            platform: LookupPlatformRepository::new(pool.clone()),
            environment: LookupEnvironmentRepository::new(pool.clone()),
            os_name: LookupOsNameRepository::new(pool.clone()),
            os_version: LookupOsVersionRepository::new(pool.clone()),
            manufacturer: LookupManufacturerRepository::new(pool.clone()),
            brand: LookupBrandRepository::new(pool.clone()),
            model: LookupModelRepository::new(pool.clone()),
            chipset: LookupChipsetRepository::new(pool.clone()),
            device_specs: DeviceSpecsRepository::new(pool.clone()),
            locale_code: LookupLocaleCodeRepository::new(pool.clone()),
            timezone: LookupTimezoneRepository::new(pool.clone()),
            connection_type: LookupConnectionTypeRepository::new(pool.clone()),
            orientation: LookupOrientationRepository::new(pool.clone()),
            app_name: LookupAppNameRepository::new(pool.clone()),
            app_version: LookupAppVersionRepository::new(pool.clone()),
            app_build: LookupAppBuildRepository::new(pool.clone()),
            user: LookupUserRepository::new(pool.clone()),
            exception_type: LookupExceptionTypeRepository::new(pool.clone()),
            exception_message: ExceptionMessageRepository::new(pool.clone()),
            stacktrace: StacktraceRepository::new(pool.clone()),
            issue: IssueRepository::new(pool),
        }
    }
}
