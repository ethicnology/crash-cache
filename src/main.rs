use clap::{Parser, Subcommand};

use crash_cache::config::Settings;
use crash_cache::features::cli::{ArchiveCommand, ProjectCommand, archive, project, ruminate};
use crash_cache::features::serve::run_server;
use crash_cache::shared::persistence::{
    ProjectRepository, establish_connection_pool, run_migrations,
};

#[derive(Parser)]
#[command(name = "crash-cache")]
#[command(about = "Crash reporting server and CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the crash-cache server
    Serve,
    /// Manage projects
    Project {
        #[command(subcommand)]
        action: ProjectCommand,
    },
    /// Export/import archives
    Archive {
        #[command(subcommand)]
        action: ArchiveCommand,
    },
    /// Re-digest all archives from scratch (clears all data except archives and projects)
    Ruminate {
        #[arg(short, long, help = "Skip confirmation prompt")]
        yes: bool,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    dotenvy::dotenv().ok();

    match cli.command {
        Commands::Serve => {
            run_server().await;
        }
        Commands::Project { action } => {
            let settings = Settings::from_env();
            let pool = establish_connection_pool(&settings.database_url);
            run_migrations(&pool);
            let project_repo = ProjectRepository::new(pool.clone());
            let server_addr = settings.server_addr();
            project::handle(action, &project_repo, &server_addr);
        }
        Commands::Archive { action } => {
            let settings = Settings::from_env();
            let pool = establish_connection_pool(&settings.database_url);
            run_migrations(&pool);
            archive::handle(action, &pool);
        }
        Commands::Ruminate { yes } => {
            let settings = Settings::from_env();
            let pool = establish_connection_pool(&settings.database_url);
            run_migrations(&pool);
            ruminate::handle(&pool, yes);
        }
    }
}
