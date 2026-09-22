use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{CommsDao, DbResult};
use domain::{
    active_source_count, moment_dto, source_dto, summarize_relationship_evidence, ActivityFeedEntry,
    CommsAggregate, CommsMomentPage, CommsPanel, CommsSourceRecord, CommsTimeline,
    GetCommsPanelRequest, GetCommsTimelineRequest, LastContactRecord, RelationshipEvidenceRecord,
    COMMS_MAX_PAGE_SIZE, COMMS_MOMENT_LIMIT, COMMS_PAGE_SIZE, COMMS_SOURCE_SLOT_COUNT,
};
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait CommsRepository: Send {
    async fn activity(&mut self, limit: i64) -> DbResult<Vec<ActivityFeedEntry>>;
    async fn sources(&mut self, person_id: &str) -> DbResult<Vec<CommsSourceRecord>>;
    async fn evidence(&mut self, person_id: &str) -> DbResult<Vec<RelationshipEvidenceRecord>>;
    async fn last_contact(&mut self, person_id: &str) -> DbResult<LastContactRecord>;
    async fn moments(
        &mut self,
        person_id: &str,
        limit: i64,
        offset: i64,
    ) -> DbResult<CommsMomentPage>;
}

#[async_trait]
impl CommsRepository for CommsDao {
    async fn activity(&mut self, limit: i64) -> DbResult<Vec<ActivityFeedEntry>> {
        CommsDao::activity(self, limit).await
    }

    async fn sources(&mut self, person_id: &str) -> DbResult<Vec<CommsSourceRecord>> {
        CommsDao::sources(self, person_id).await
    }

    async fn evidence(&mut self, person_id: &str) -> DbResult<Vec<RelationshipEvidenceRecord>> {
        CommsDao::evidence(self, person_id).await
    }

    async fn last_contact(&mut self, person_id: &str) -> DbResult<LastContactRecord> {
        CommsDao::last_contact(self, person_id).await
    }

    async fn moments(
        &mut self,
        person_id: &str,
        limit: i64,
        offset: i64,
    ) -> DbResult<CommsMomentPage> {
        CommsDao::moments(self, person_id, limit, offset).await
    }
}

pub struct CommsService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: CommsRepository> CommsService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn activity(
        &mut self,
        limit: i64,
        context: &ServiceContext,
    ) -> Result<Vec<ActivityFeedEntry>, CoreServiceError> {
        const OP: &str = "comms.activity";
        let decision = authorize(
            &self.runtime,
            "comms",
            "comms.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;

        let result = self
            .repository
            .activity(limit.clamp(1, 500))
            .await
            .map_err(Into::into);

        audit_result(&self.runtime, "comms", OP, context, decision, &result).await?;
        result
    }

    pub async fn panel(
        &mut self,
        request: &GetCommsPanelRequest,
        context: &ServiceContext,
    ) -> Result<CommsPanel, CoreServiceError> {
        const OP: &str = "comms.panel";
        let decision = authorize(
            &self.runtime,
            "comms",
            "comms.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;

        let result = async {
            let limit = clamp(
                request.moment_limit,
                0,
                COMMS_MAX_PAGE_SIZE,
                COMMS_MOMENT_LIMIT,
            );
            let sources = self.repository.sources(&request.person_id).await?;
            let evidence = self.repository.evidence(&request.person_id).await?;
            let last_contact = self.repository.last_contact(&request.person_id).await?;
            let page = self
                .repository
                .moments(&request.person_id, limit, 0)
                .await?;

            let summary = summarize_relationship_evidence(&evidence);
            let active_count = active_source_count(&sources);
            let mut source_rows: Vec<_> = sources.into_iter().map(source_dto).collect();
            source_rows.sort_by(|left, right| {
                left.channel
                    .order()
                    .cmp(&right.channel.order())
                    .then_with(|| left.source.cmp(&right.source))
            });

            Ok(CommsPanel {
                person_id: request.person_id.clone(),
                aggregate: CommsAggregate {
                    observed_count: summary.inbound_count + summary.outbound_count,
                    inbound_count: summary.inbound_count,
                    outbound_count: summary.outbound_count,
                    two_way: summary.two_way,
                    first_observed_at: summary.first_observed_at,
                    last_inbound_at: summary.last_inbound_at,
                    last_outbound_at: summary.last_outbound_at,
                    last_contact_at: last_contact.at.or(summary.last_meaningful_contact_at),
                    last_contact_label: last_contact.label,
                    active_source_count: active_count,
                    source_count: COMMS_SOURCE_SLOT_COUNT,
                },
                sources: source_rows,
                moments: page.moments.into_iter().map(moment_dto).collect(),
                moment_count: page.total,
            })
        }
        .await;

        audit_result(&self.runtime, "comms", OP, context, decision, &result).await?;
        result
    }

    pub async fn timeline(
        &mut self,
        request: &GetCommsTimelineRequest,
        context: &ServiceContext,
    ) -> Result<CommsTimeline, CoreServiceError> {
        const OP: &str = "comms.timeline";
        let decision = authorize(
            &self.runtime,
            "comms",
            "comms.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;

        let result = async {
            let page_size = clamp(request.page_size, 1, COMMS_MAX_PAGE_SIZE, COMMS_PAGE_SIZE);
            let page = clamp(request.page, 1, i64::MAX, 1);
            let rows = self
                .repository
                .moments(&request.person_id, page_size, (page - 1) * page_size)
                .await?;

            Ok(CommsTimeline {
                person_id: request.person_id.clone(),
                moments: rows.moments.into_iter().map(moment_dto).collect(),
                total: rows.total,
                page,
                page_size,
            })
        }
        .await;

        audit_result(&self.runtime, "comms", OP, context, decision, &result).await?;
        result
    }
}

fn clamp(value: Option<i64>, min: i64, max: i64, fallback: i64) -> i64 {
    value.unwrap_or(fallback).clamp(min, max)
}
