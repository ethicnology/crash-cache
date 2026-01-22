use clap::{Parser, Subcommand};

use crash_cache::config::Settings;
use crash_cache::features::cli::{project, ProjectCommand};
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
    Project {
        #[command(subcommand)]
        action: ProjectCommand,
    },
}

fn main() {
    let cli = Cli::parse();

    dotenvy::dotenv().ok();
    let settings = Settings::from_env();
    let pool = establish_connection_pool(&settings.database_url);
    run_migrations(&pool);

    let project_repo = ProjectRepository::new(pool);

    match cli.command {
        Commands::Project { action } => project::handle(action, &project_repo),
    }
}
