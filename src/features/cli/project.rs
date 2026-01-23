use clap::Subcommand;
use uuid::Uuid;

use crate::shared::domain::Project;
use crate::shared::persistence::ProjectRepository;

#[derive(Subcommand)]
pub enum ProjectCommand {
    Create {
        #[arg(short, long)]
        id: i32,
        #[arg(short, long)]
        name: Option<String>,
        #[arg(short, long)]
        key: Option<String>,
    },
    Delete {
        #[arg(short, long)]
        id: i32,
    },
    List,
}

pub fn handle(command: ProjectCommand, repo: &ProjectRepository, server_addr: &str) {
    match command {
        ProjectCommand::Create { id, name, key } => {
            let public_key = key.unwrap_or_else(|| Uuid::new_v4().simple().to_string());
            let project = Project::new(id)
                .with_name(name)
                .with_public_key(Some(public_key.clone()));

            match repo.save(&project) {
                Ok(_) => {
                    println!("Project '{}' created successfully", id);
                    println!("DSN: http://{}@{}/{}", public_key, server_addr, id);
                }
                Err(e) => eprintln!("Failed to create project: {}", e),
            }
        }
        ProjectCommand::Delete { id } => match repo.delete(id) {
            Ok(_) => println!("Project '{}' deleted", id),
            Err(e) => eprintln!("Failed to delete project: {}", e),
        },
        ProjectCommand::List => match repo.list_all() {
            Ok(projects) => {
                if projects.is_empty() {
                    println!("No projects found");
                } else {
                    println!("{:<20} {:<34} {:<30} {:<20}", "ID", "PUBLIC_KEY", "NAME", "CREATED AT");
                    println!("{}", "-".repeat(110));
                    for p in projects {
                        println!(
                            "{:<20} {:<34} {:<30} {:<20}",
                            p.id,
                            p.public_key.as_deref().unwrap_or("-"),
                            p.name.as_deref().unwrap_or("-"),
                            p.created_at.format("%Y-%m-%d %H:%M:%S")
                        );
                    }
                }
            }
            Err(e) => eprintln!("Failed to list projects: {}", e),
        },
    }
}
