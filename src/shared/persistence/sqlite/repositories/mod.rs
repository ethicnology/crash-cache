mod archive_repository;
mod device_specs_repository;
mod exception_message_repository;
mod issue_repository;
mod unwrap_repository;
mod project_repository;
mod queue_repository;
mod report_repository;
mod stacktrace_repository;

pub use archive_repository::ArchiveRepository;
pub use device_specs_repository::DeviceSpecsRepository;
pub use exception_message_repository::ExceptionMessageRepository;
pub use issue_repository::IssueRepository;
pub use unwrap_repository::*;
pub use project_repository::ProjectRepository;
pub use queue_repository::{QueueRepository, QueueErrorRepository};
pub use report_repository::{NewReport, ReportRepository};
pub use stacktrace_repository::StacktraceRepository;

use super::SqlitePool;

#[derive(Clone)]
pub struct Repositories {
    pub archive: ArchiveRepository,
    pub queue: QueueRepository,
    pub queue_error: QueueErrorRepository,
    pub project: ProjectRepository,
    pub report: ReportRepository,
    pub platform: UnwrapPlatformRepository,
    pub environment: UnwrapEnvironmentRepository,
    pub os_name: UnwrapOsNameRepository,
    pub os_version: UnwrapOsVersionRepository,
    pub manufacturer: UnwrapManufacturerRepository,
    pub brand: UnwrapBrandRepository,
    pub model: UnwrapModelRepository,
    pub chipset: UnwrapChipsetRepository,
    pub device_specs: DeviceSpecsRepository,
    pub locale_code: UnwrapLocaleCodeRepository,
    pub timezone: UnwrapTimezoneRepository,
    pub connection_type: UnwrapConnectionTypeRepository,
    pub orientation: UnwrapOrientationRepository,
    pub app_name: UnwrapAppNameRepository,
    pub app_version: UnwrapAppVersionRepository,
    pub app_build: UnwrapAppBuildRepository,
    pub user: UnwrapUserRepository,
    pub exception_type: UnwrapExceptionTypeRepository,
    pub exception_message: ExceptionMessageRepository,
    pub stacktrace: StacktraceRepository,
    pub issue: IssueRepository,
}

impl Repositories {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            archive: ArchiveRepository::new(pool.clone()),
            queue: QueueRepository::new(pool.clone()),
            queue_error: QueueErrorRepository::new(pool.clone()),
            project: ProjectRepository::new(pool.clone()),
            report: ReportRepository::new(pool.clone()),
            platform: UnwrapPlatformRepository::new(pool.clone()),
            environment: UnwrapEnvironmentRepository::new(pool.clone()),
            os_name: UnwrapOsNameRepository::new(pool.clone()),
            os_version: UnwrapOsVersionRepository::new(pool.clone()),
            manufacturer: UnwrapManufacturerRepository::new(pool.clone()),
            brand: UnwrapBrandRepository::new(pool.clone()),
            model: UnwrapModelRepository::new(pool.clone()),
            chipset: UnwrapChipsetRepository::new(pool.clone()),
            device_specs: DeviceSpecsRepository::new(pool.clone()),
            locale_code: UnwrapLocaleCodeRepository::new(pool.clone()),
            timezone: UnwrapTimezoneRepository::new(pool.clone()),
            connection_type: UnwrapConnectionTypeRepository::new(pool.clone()),
            orientation: UnwrapOrientationRepository::new(pool.clone()),
            app_name: UnwrapAppNameRepository::new(pool.clone()),
            app_version: UnwrapAppVersionRepository::new(pool.clone()),
            app_build: UnwrapAppBuildRepository::new(pool.clone()),
            user: UnwrapUserRepository::new(pool.clone()),
            exception_type: UnwrapExceptionTypeRepository::new(pool.clone()),
            exception_message: ExceptionMessageRepository::new(pool.clone()),
            stacktrace: StacktraceRepository::new(pool.clone()),
            issue: IssueRepository::new(pool),
        }
    }
}
