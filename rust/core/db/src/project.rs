use crate::{Database, DbFailure, DbResult};
use chrono::{DateTime, Utc};
use domain::{
    CompleteProjectRequest, CreateProjectRequest, Project, ProjectStatus, UpdateProjectRequest,
    WbsCategory,
};
use sqlx::FromRow;

#[derive(Debug, FromRow)]
struct ProjectRow {
    id: String,
    name: String,
    owner: Option<String>,
    status: String,
    description: String,
    areas: Vec<String>,
    starts_at: Option<DateTime<Utc>>,
    ends_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    project_type: Option<String>,
    playbook_id: Option<String>,
    playbook_version: Option<i32>,
    person_id: Option<String>,
    property_id: Option<String>,
    contract_id: Option<String>,
}

impl TryFrom<ProjectRow> for Project {
    type Error = DbFailure;

    fn try_from(row: ProjectRow) -> Result<Self, Self::Error> {
        let status = ProjectStatus::try_from(row.status.as_str())
            .map_err(|error| DbFailure::schema_mismatch("project.map", error.to_string()))?;

        let mut areas = Vec::with_capacity(row.areas.len());
        for area in row.areas {
            areas.push(
                WbsCategory::try_from(area.as_str())
                    .map_err(|error| DbFailure::schema_mismatch("project.map", error.to_string()))?,
            );
        }

        Ok(Self {
            id: row.id,
            name: row.name,
            owner: row.owner,
            status,
            description: row.description,
            areas,
            starts_at: row.starts_at,
            ends_at: row.ends_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
            project_type: row.project_type,
            playbook_id: row.playbook_id,
            playbook_version: row.playbook_version,
            person_id: row.person_id,
            property_id: row.property_id,
            contract_id: row.contract_id,
        })
    }
}

#[derive(Clone)]
pub struct ProjectDao {
    db: Database,
}

impl ProjectDao {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn get(&self, id: &str) -> DbResult<Option<Project>> {
        let row = sqlx::query_as::<_, ProjectRow>(
            r#"
            select id, name, owner, status, description, areas,
                   starts_at, ends_at, created_at, updated_at,
                   project_type, playbook_id, playbook_version,
                   person_id, property_id, contract_id
            from project
            where id = $1
            limit 1
            "#,
        )
        .bind(id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("project.get", &error))?;

        row.map(Project::try_from).transpose()
    }

    pub async fn list(&self) -> DbResult<Vec<Project>> {
        let rows = sqlx::query_as::<_, ProjectRow>(
            r#"
            select id, name, owner, status, description, areas,
                   starts_at, ends_at, created_at, updated_at,
                   project_type, playbook_id, playbook_version,
                   person_id, property_id, contract_id
            from project
            order by created_at desc, id
            "#,
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("project.list", &error))?;

        rows.into_iter().map(Project::try_from).collect()
    }

    pub async fn create(&self, request: &CreateProjectRequest) -> DbResult<Project> {
        let areas: Vec<String> = request
            .areas
            .iter()
            .map(|area| area.as_str().to_owned())
            .collect();

        let row = sqlx::query_as::<_, ProjectRow>(
            r#"
            insert into project (
                id, name, owner, description, areas, starts_at, ends_at,
                project_type, playbook_id, playbook_version,
                person_id, property_id, contract_id
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            returning id, name, owner, status, description, areas,
                      starts_at, ends_at, created_at, updated_at,
                      project_type, playbook_id, playbook_version,
                      person_id, property_id, contract_id
            "#,
        )
        .bind(&request.id)
        .bind(&request.name)
        .bind(&request.owner)
        .bind(&request.description)
        .bind(areas)
        .bind(request.starts_at)
        .bind(request.ends_at)
        .bind(&request.project_type)
        .bind(&request.playbook_id)
        .bind(request.playbook_version)
        .bind(&request.person_id)
        .bind(&request.property_id)
        .bind(&request.contract_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("project.create", &error))?;

        Project::try_from(row)
    }

    pub async fn update(&self, request: &UpdateProjectRequest) -> DbResult<Option<Project>> {
        let areas: Option<Vec<String>> = request.areas.as_ref().map(|values| {
            values
                .iter()
                .map(|area| area.as_str().to_owned())
                .collect()
        });
        let status = request.status.as_ref().map(ProjectStatus::as_str);

        let row = sqlx::query_as::<_, ProjectRow>(
            r#"
            update project
            set name = coalesce($2, name),
                owner = coalesce($3, owner),
                status = coalesce($4, status),
                description = coalesce($5, description),
                areas = coalesce($6, areas),
                starts_at = coalesce($7, starts_at),
                ends_at = coalesce($8, ends_at),
                project_type = coalesce($9, project_type),
                playbook_id = coalesce($10, playbook_id),
                playbook_version = coalesce($11, playbook_version),
                person_id = coalesce($12, person_id),
                property_id = coalesce($13, property_id),
                contract_id = coalesce($14, contract_id),
                updated_at = now()
            where id = $1
            returning id, name, owner, status, description, areas,
                      starts_at, ends_at, created_at, updated_at,
                      project_type, playbook_id, playbook_version,
                      person_id, property_id, contract_id
            "#,
        )
        .bind(&request.id)
        .bind(&request.name)
        .bind(&request.owner)
        .bind(status)
        .bind(&request.description)
        .bind(areas)
        .bind(request.starts_at)
        .bind(request.ends_at)
        .bind(&request.project_type)
        .bind(&request.playbook_id)
        .bind(request.playbook_version)
        .bind(&request.person_id)
        .bind(&request.property_id)
        .bind(&request.contract_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("project.update", &error))?;

        row.map(Project::try_from).transpose()
    }

    pub async fn complete(&self, request: &CompleteProjectRequest) -> DbResult<Option<Project>> {
        let row = sqlx::query_as::<_, ProjectRow>(
            r#"
            update project
            set status = 'done', updated_at = now()
            where id = $1
            returning id, name, owner, status, description, areas,
                      starts_at, ends_at, created_at, updated_at,
                      project_type, playbook_id, playbook_version,
                      person_id, property_id, contract_id
            "#,
        )
        .bind(&request.id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("project.complete", &error))?;

        row.map(Project::try_from).transpose()
    }
}
