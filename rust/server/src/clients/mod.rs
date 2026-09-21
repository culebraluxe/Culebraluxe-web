use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{ClientDao, DbResult};
use domain::{
    build_contact_history, relationship_activity, AssignableAgent, ClientAdminPageRequest,
    ClientAdminPageResult, ClientAdminRow, ClientContactHistoryResult, ClientDetail,
    ClientDirectoryPageRequest, ClientDirectoryRecord, ClientHistoryEventRecord,
    ClientHistoryRequest, ClientSummary, ClientsPageResult, RelationshipEvidenceRecord,
    CLIENT_MAX_PAGE_SIZE, CLIENT_RECENT_HISTORY_LIMIT,
};
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};
use std::collections::HashMap;

#[async_trait]
pub trait ClientRepository: Send {
    async fn directory_page(
        &mut self,
        request: &ClientDirectoryPageRequest,
    ) -> DbResult<(Vec<ClientDirectoryRecord>, i64)>;
    async fn admin_page(
        &mut self,
        request: &ClientAdminPageRequest,
    ) -> DbResult<(Vec<ClientAdminRow>, i64)>;
    async fn detail(&mut self, person_id: &str) -> DbResult<Option<ClientDetail>>;
    async fn assignable_agents(&mut self) -> DbResult<Vec<AssignableAgent>>;
    async fn evidence_for_people(
        &mut self,
        person_ids: &[String],
    ) -> DbResult<Vec<(String, RelationshipEvidenceRecord)>>;
    async fn history_events(
        &mut self,
        person_id: &str,
        limit: i64,
        offset: i64,
    ) -> DbResult<(Vec<ClientHistoryEventRecord>, i64)>;
    async fn covered_sources(&mut self, person_id: &str) -> DbResult<Vec<String>>;
}

#[async_trait]
impl ClientRepository for ClientDao {
    async fn directory_page(
        &mut self,
        request: &ClientDirectoryPageRequest,
    ) -> DbResult<(Vec<ClientDirectoryRecord>, i64)> {
        // Every method on this impl is a read, so every one of them may be attempted again: a page load that meets a
        // suspended Neon branch should not become a 503 when one more try would have answered it.
        db::retrying_read!(ClientDao::directory_page(self, request))
    }

    async fn admin_page(
        &mut self,
        request: &ClientAdminPageRequest,
    ) -> DbResult<(Vec<ClientAdminRow>, i64)> {
        db::retrying_read!(ClientDao::admin_page(self, request))
    }

    async fn detail(&mut self, person_id: &str) -> DbResult<Option<ClientDetail>> {
        db::retrying_read!(ClientDao::detail(self, person_id))
    }

    async fn assignable_agents(&mut self) -> DbResult<Vec<AssignableAgent>> {
        db::retrying_read!(ClientDao::assignable_agents(self))
    }

    async fn evidence_for_people(
        &mut self,
        person_ids: &[String],
    ) -> DbResult<Vec<(String, RelationshipEvidenceRecord)>> {
        db::retrying_read!(ClientDao::evidence_for_people(self, person_ids))
    }

    async fn history_events(
        &mut self,
        person_id: &str,
        limit: i64,
        offset: i64,
    ) -> DbResult<(Vec<ClientHistoryEventRecord>, i64)> {
        db::retrying_read!(ClientDao::history_events(self, person_id, limit, offset))
    }

    async fn covered_sources(&mut self, person_id: &str) -> DbResult<Vec<String>> {
        db::retrying_read!(ClientDao::covered_sources(self, person_id))
    }
}

pub struct ClientService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: ClientRepository> ClientService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn directory(
        &mut self,
        request: &ClientDirectoryPageRequest,
        context: &ServiceContext,
    ) -> Result<ClientsPageResult, CoreServiceError> {
        const OP: &str = "clients.directory";
        let decision = authorize(
            &self.runtime,
            "clients",
            "person.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;

        let result = async {
            let normalized = ClientDirectoryPageRequest {
                search: request.search.clone(),
                status: request.status.clone(),
                role: request.role.clone(),
                sort: request.sort.clone(),
                page: request.page.max(1),
                page_size: request.page_size.clamp(1, CLIENT_MAX_PAGE_SIZE),
            };
            let (rows, total) = self.repository.directory_page(&normalized).await?;
            let person_ids: Vec<String> = rows.iter().map(|row| row.id.clone()).collect();
            let evidence = group_evidence(self.repository.evidence_for_people(&person_ids).await?);
            let rows = rows
                .into_iter()
                .map(|row| ClientSummary {
                    relationship_activity: relationship_activity(
                        evidence.get(&row.id).map(Vec::as_slice).unwrap_or(&[]),
                    ),
                    id: row.id,
                    display_name: row.display_name,
                    name_resolved: row.name_resolved,
                    role: row.role,
                    status: row.status,
                    location: row.location,
                    primary_email: row.primary_email,
                    primary_phone: row.primary_phone,
                    assigned_agent: row.assigned_agent,
                    last_contact_label: row.last_contact_label,
                    sources: row.sources,
                })
                .collect();

            Ok(ClientsPageResult {
                rows,
                total,
                page: normalized.page,
                page_size: normalized.page_size,
            })
        }
        .await;

        audit_result(&self.runtime, "clients", OP, context, decision, &result).await?;
        result
    }

    pub async fn admin(
        &mut self,
        request: &ClientAdminPageRequest,
        context: &ServiceContext,
    ) -> Result<ClientAdminPageResult, CoreServiceError> {
        const OP: &str = "clients.admin";
        let decision = authorize(
            &self.runtime,
            "clients",
            "person.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;

        let result = async {
            let normalized = ClientAdminPageRequest {
                search: request.search.clone(),
                page: request.page.max(1),
                page_size: request.page_size.clamp(1, CLIENT_MAX_PAGE_SIZE),
            };
            let (rows, total) = self.repository.admin_page(&normalized).await?;
            Ok(ClientAdminPageResult {
                rows,
                total,
                page: normalized.page,
                page_size: normalized.page_size,
            })
        }
        .await;

        audit_result(&self.runtime, "clients", OP, context, decision, &result).await?;
        result
    }

    pub async fn detail(
        &mut self,
        person_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<ClientDetail>, CoreServiceError> {
        const OP: &str = "clients.detail";
        let decision = authorize(
            &self.runtime,
            "clients",
            "person.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;

        let result = async {
            let Some(mut client) = self.repository.detail(person_id).await? else {
                return Ok(None);
            };
            let evidence = self
                .repository
                .evidence_for_people(&[person_id.to_owned()])
                .await?;
            let rows: Vec<RelationshipEvidenceRecord> = evidence
                .into_iter()
                .filter_map(|(id, row)| (id == person_id).then_some(row))
                .collect();
            client.relationship_activity = Some(relationship_activity(&rows));
            Ok(Some(client))
        }
        .await;

        audit_result(&self.runtime, "clients", OP, context, decision, &result).await?;
        result
    }

    pub async fn agents(
        &mut self,
        context: &ServiceContext,
    ) -> Result<Vec<AssignableAgent>, CoreServiceError> {
        const OP: &str = "clients.agents";
        let decision = authorize(
            &self.runtime,
            "clients",
            "person.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;
        let result = self
            .repository
            .assignable_agents()
            .await
            .map_err(Into::into);
        audit_result(&self.runtime, "clients", OP, context, decision, &result).await?;
        result
    }

    pub async fn history(
        &mut self,
        request: &ClientHistoryRequest,
        context: &ServiceContext,
    ) -> Result<ClientContactHistoryResult, CoreServiceError> {
        const OP: &str = "clients.history";
        let decision = authorize(
            &self.runtime,
            "clients",
            "comms.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;

        let result = async {
            let page = request.page.max(1);
            let page_size = if request.recent {
                CLIENT_RECENT_HISTORY_LIMIT
            } else {
                request.page_size.clamp(1, CLIENT_MAX_PAGE_SIZE)
            };
            let offset = if request.recent {
                0
            } else {
                (page - 1) * page_size
            };
            let (events, total) = self
                .repository
                .history_events(&request.person_id, page_size, offset)
                .await?;
            let evidence = self
                .repository
                .evidence_for_people(&[request.person_id.clone()])
                .await?;
            let evidence: Vec<_> = evidence
                .into_iter()
                .filter_map(|(id, row)| (id == request.person_id).then_some(row))
                .collect();
            let covered_sources = self.repository.covered_sources(&request.person_id).await?;
            Ok(build_contact_history(
                events,
                &evidence,
                &covered_sources,
                total,
                page,
                page_size,
                request.recent,
            ))
        }
        .await;

        audit_result(&self.runtime, "clients", OP, context, decision, &result).await?;
        result
    }
}

fn group_evidence(
    rows: Vec<(String, RelationshipEvidenceRecord)>,
) -> HashMap<String, Vec<RelationshipEvidenceRecord>> {
    let mut grouped: HashMap<String, Vec<RelationshipEvidenceRecord>> = HashMap::new();
    for (person_id, row) in rows {
        grouped.entry(person_id).or_default().push(row);
    }
    grouped
}
