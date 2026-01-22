use clap::Subcommand;

use crate::shared::domain::Project;
use crate::shared::persistence::ProjectRepository;

#[derive(Subcommand)]
pub enum ProjectCommand {
    Create {
        #[arg(short, long)]
        id: String,
        #[arg(short, long)]
        name: Option<String>,
        #[arg(short, long)]
        key: Option<String>,
    },
    Delete {
        #[arg(short, long)]
        id: String,
    },
    List,
}

pub fn handle(command: ProjectCommand, repo: &ProjectRepository) {
    match command {
        ProjectCommand::Create { id, name, key } => {
            let project = Project::new(id.clone())
                .with_name(name)
                .with_public_key(key);

            match repo.save(&project) {
                Ok(_) => println!("Project '{}' created successfully", id),
                Err(e) => eprintln!("Failed to create project: {}", e),
            }
        }
        ProjectCommand::Delete { id } => match repo.delete(&id) {
            Ok(_) => println!("Project '{}' deleted", id),
            Err(e) => eprintln!("Failed to delete project: {}", e),
        },
        ProjectCommand::List => match repo.list_all() {
            Ok(projects) => {
                if projects.is_empty() {
                    println!("No projects found");
                } else {
                    println!("{:<20} {:<30} {:<20}", "ID", "NAME", "CREATED AT");
                    println!("{}", "-".repeat(70));
                    for p in projects {
                        println!(
                            "{:<20} {:<30} {:<20}",
                            p.id,
                            p.name.unwrap_or_default(),
                            p.created_at.format("%Y-%m-%d %H:%M:%S")
                        );
                    }
                }
            }
            Err(e) => eprintln!("Failed to list projects: {}", e),
        },
    }
}
