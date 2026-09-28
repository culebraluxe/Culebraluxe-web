use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{ClientRoomDao, DbResult};
use domain::ClientRoomSnapshot;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait ClientRoomRepository: Send {
    async fn snapshot(&self, person_id: &str) -> DbResult<Option<ClientRoomSnapshot>>;
}

#[async_trait]
impl ClientRoomRepository for ClientRoomDao {
    async fn snapshot(&self, person_id: &str) -> DbResult<Option<ClientRoomSnapshot>> {
        ClientRoomDao::snapshot(self, person_id).await
    }
}

pub struct ClientRoomService<R> {
    repository: R,
    runtime: ServiceRuntime,
}

impl<R: ClientRoomRepository> ClientRoomService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self {
            repository,
            runtime: ServiceRuntime::new(infrastructure),
        }
    }

    pub async fn snapshot(
        &self,
        person_id: &str,
        context: &ServiceContext,
    ) -> Result<Option<ClientRoomSnapshot>, CoreServiceError> {
        const OP: &str = "clientRoom.snapshot";
        let decision = authorize(
            &self.runtime,
            "client-room",
            "client.room.read",
            OP,
            OperationKind::Query,
            context,
        )
        .await?;

        let result = if person_id.trim().is_empty() {
            Err(CoreServiceError::business(
                "CLIENT_ROOM_PERSON_REQUIRED",
                "This account is not linked to a client record yet.",
            ))
        } else {
            self.repository.snapshot(person_id).await.map_err(Into::into)
        };
        audit_result(&self.runtime, "client-room", OP, context, decision, &result).await?;
        result
    }
}
