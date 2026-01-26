use clap::{Parser, Subcommand};

use crash_cache::config::Settings;
use crash_cache::features::cli::{archive, project, ruminate, ArchiveCommand, ProjectCommand};
use crash_cache::shared::persistence::{establish_connection_pool, run_migrations, ProjectRepository};

#[derive(Parser)]
#[command(name = "crash-cli")]
#[command(about = "CLI for crash-cache management", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
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

fn main() {
    let cli = Cli::parse();

    dotenvy::dotenv().ok();
    let settings = Settings::from_env();
    let pool = establish_connection_pool(&settings.database_url);
    run_migrations(&pool);

    let project_repo = ProjectRepository::new(pool.clone());

    let server_addr = settings.server_addr();

    match cli.command {
        Commands::Project { action } => project::handle(action, &project_repo, &server_addr),
        Commands::Archive { action } => archive::handle(action, &pool),
        Commands::Ruminate { yes } => ruminate::handle(&pool, yes),
    }
}
