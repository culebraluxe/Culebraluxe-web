use db::{Database, ProjectDao};
use server::projects::ProjectService;
use std::error::Error;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let command = std::env::args().nth(1).unwrap_or_default();

    let result = match command.as_str() {
        "db-smoke" => db_smoke().await,
        _ => {
            eprintln!("usage: cargo run -p cli -- db-smoke");
            return ExitCode::from(2);
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("rust smoke failed: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn db_smoke() -> Result<(), Box<dyn Error>> {
    let db = Database::connect_from_env().await?;
    db.ping().await?;

    let target = db.target();
    let service = ProjectService::new(ProjectDao::new(db));
    let projects = service.list().await?;

    println!(
        "rust db/service smoke ok target={} projects={}",
        target.as_str(),
        projects.len()
    );

    if let Some(project) = projects.first() {
        println!(
            "project sample id={} status={} areas={}",
            project.id,
            project.status,
            project.areas.len()
        );
    }

    Ok(())
}
