use crate::service_support::{audit_result, authorize, CoreServiceError};
use async_trait::async_trait;
use db::{DbResult, TechCockpitDao};
use domain::{TechCockpitSnapshot, TechCommandRequest, TechCommandResult};
use serde_json::Value;
use service::{OperationKind, ServiceContext, ServiceInfrastructure, ServiceRuntime};

#[async_trait]
pub trait TechCockpitRepository: Send {
    async fn snapshot(&mut self, selected: Option<&str>) -> DbResult<TechCockpitSnapshot>;
    async fn clear_active_work(&mut self)->DbResult<u64>;
    async fn set_active_work(&mut self,id:&str,active:bool,actor:&str)->DbResult<()>;
    async fn story_status(&mut self,id:&str,status:&str)->DbResult<()>;
    async fn active_agent_work(&mut self,id:&str)->DbResult<Vec<Value>>;
    async fn set_dispatch_options(&mut self,id:&str,stop:Option<&str>)->DbResult<u64>;
    async fn withdraw_ready(&mut self,id:&str)->DbResult<(u64,i64)>;
    async fn staged_story_ids(&mut self)->DbResult<Vec<String>>;
    async fn cancel_batch(&mut self,id:&str)->DbResult<u64>;
    async fn schedule_flight(&mut self,when:&str,actor:&str)->DbResult<i64>;
    async fn launch_flight(&mut self,actor:&str)->DbResult<Option<(i64,i64)>>;
}

#[async_trait]
impl TechCockpitRepository for TechCockpitDao {
    async fn snapshot(&mut self, selected: Option<&str>) -> DbResult<TechCockpitSnapshot> { TechCockpitDao::snapshot(self, selected).await }
    async fn clear_active_work(&mut self)->DbResult<u64>{TechCockpitDao::clear_active_work(self).await}
    async fn set_active_work(&mut self,id:&str,active:bool,actor:&str)->DbResult<()>{TechCockpitDao::set_active_work(self,id,active,actor).await}
    async fn story_status(&mut self,id:&str,status:&str)->DbResult<()>{TechCockpitDao::story_status(self,id,status).await}
    async fn active_agent_work(&mut self,id:&str)->DbResult<Vec<Value>>{TechCockpitDao::active_agent_work(self,id).await}
    async fn set_dispatch_options(&mut self,id:&str,stop:Option<&str>)->DbResult<u64>{TechCockpitDao::set_dispatch_options(self,id,stop).await}
    async fn withdraw_ready(&mut self,id:&str)->DbResult<(u64,i64)>{TechCockpitDao::withdraw_ready(self,id).await}
    async fn staged_story_ids(&mut self)->DbResult<Vec<String>>{TechCockpitDao::staged_story_ids(self).await}
    async fn cancel_batch(&mut self,id:&str)->DbResult<u64>{TechCockpitDao::cancel_batch(self,id).await}
    async fn schedule_flight(&mut self,when:&str,actor:&str)->DbResult<i64>{TechCockpitDao::schedule_flight(self,when,actor).await}
    async fn launch_flight(&mut self,actor:&str)->DbResult<Option<(i64,i64)>>{TechCockpitDao::launch_flight(self,actor).await}
}

pub struct TechCockpitService<R> { repository: R, runtime: ServiceRuntime }

impl<R: TechCockpitRepository> TechCockpitService<R> {
    pub fn new(repository: R, infrastructure: ServiceInfrastructure) -> Self {
        Self { repository, runtime: ServiceRuntime::new(infrastructure) }
    }

    pub async fn snapshot(&mut self, selected: Option<&str>, context: &ServiceContext)
        -> Result<TechCockpitSnapshot, CoreServiceError> {
        const OP: &str = "tech.cockpitSnapshot";
        let decision = authorize(&self.runtime, "tech", "tech.access", OP, OperationKind::Query, context).await?;
        let result = self.repository.snapshot(selected).await.map_err(Into::into);
        audit_result(&self.runtime, "tech", OP, context, decision, &result).await?;
        result
    }
    pub async fn command(&mut self, request: TechCommandRequest, context: &ServiceContext)
        -> Result<TechCommandResult, CoreServiceError> {
        const OP:&str="tech.command";
        let decision=authorize(&self.runtime,"tech","tech.operate",OP,OperationKind::Command,context).await?;
        let actor=context.actor.id.as_deref().unwrap_or("tech-operator");
        let id=request.story_id.as_deref().unwrap_or("").trim();
        let result:Result<TechCommandResult,CoreServiceError>=async {
            let ok=|message:String| Ok(TechCommandResult{ok:true,message});
            match request.action.as_str() {
              "clearWorkbench" => { let n=self.repository.clear_active_work().await?; ok(format!("Cleared {n} stories from the Workbench. Story statuses were not changed.")) }
              "goodToGo" => {
                if id.is_empty(){return Err(CoreServiceError::business("VALIDATION", "Missing story id."))}
                let active=self.repository.active_agent_work(id).await?;
                if let Some(w)=active.first(){return Err(CoreServiceError::business("CONFLICT", format!("{id} is already with Forge ({}).",w["state"].as_str().unwrap_or("active"))))}
                self.repository.set_active_work(id,false,actor).await?;
                self.repository.story_status(id,"Ready").await?;
                if self.repository.set_dispatch_options(id,None).await?==0{return Err(CoreServiceError::business("CONFLICT", "No queued work item to hand off — this story is already running."))}
                ok(format!("{id} handed to Forge for the full chain. Ready queued a real work item."))
              }
              "scopedRun" => {
                if id.is_empty(){return Err(CoreServiceError::business("VALIDATION", "Missing story id."))}
                let stop=request.stop_after.as_deref().unwrap_or("");
                if !matches!(stop,"scout"|"architect"|"lead"){return Err(CoreServiceError::business("VALIDATION", format!("Unsupported scoped stop: {stop}")))}
                let active=self.repository.active_agent_work(id).await?;
                if let Some(w)=active.iter().find(|w|matches!(w["state"].as_str(),Some("Claimed"|"Running"|"Paused"))){return Err(CoreServiceError::business("CONFLICT", format!("{id} is already executing ({}); a live run cannot be re-scoped.",w["state"].as_str().unwrap_or("active"))))}
                if !active.iter().any(|w|w["state"]=="Ready"){self.repository.story_status(id,"Ready").await?}
                if self.repository.set_dispatch_options(id,Some(stop)).await?==0{return Err(CoreServiceError::business("CONFLICT", "No queued work item to scope — this story is already running."))}
                ok(format!("{id} queued through {stop}; it remains on the Workbench for review."))
              }
              "moveWorkbench" => {
                if id.is_empty(){return Err(CoreServiceError::business("VALIDATION", "Missing story id."))}
                let target=request.target.as_deref().unwrap_or("");
                let status=match target{"backlog"=>"Backlog","closed"=>"Closed","next"=>"Next Version",_=>return Err(CoreServiceError::business("VALIDATION", format!("Unsupported Workbench destination: {target}")))};
                let (withdrawn,live)=self.repository.withdraw_ready(id).await?;
                self.repository.set_active_work(id,false,actor).await?;
                self.repository.story_status(id,status).await?;
                let note=if withdrawn>0{format!(" Withdrew {withdrawn} queued scoped request(s).")}else if live>0{" Forge is already running this story; that live run continues.".into()}else{String::new()};
                ok(format!("{id} moved to {status}.{note}"))
              }
              "cancelFlight" => {
                let batch=request.batch_id.as_deref().unwrap_or("").trim();
                if batch.is_empty(){return Err(CoreServiceError::business("VALIDATION", "Missing Flight id."))}
                if self.repository.cancel_batch(batch).await?==0{return Err(CoreServiceError::business("CONFLICT", "That Flight is not waiting to fire."))}
                ok("Scheduled Flight cancelled. Nothing was dispatched.".into())
              }
              "launchFlight" => {
                match self.repository.launch_flight(actor).await? {
                  None=>ok("Nothing is staged in the current Flight.".into()),
                  Some((queued,stamped))=>ok(format!("Flight launched: {queued} stories queued for Forge; {stamped} routing stamps applied."))
                }
              }
              "scheduleFlight" => {
                let when=request.scheduled_for.as_deref().unwrap_or("").trim();
                if when.is_empty(){return Err(CoreServiceError::business("VALIDATION","The Flight time could not be read."))}
                if self.repository.staged_story_ids().await?.is_empty(){return Err(CoreServiceError::business("VALIDATION","Nothing is staged in the current Flight."))}
                let count=self.repository.schedule_flight(when,actor).await?;
                ok(format!("Scheduled {count} stories for {when}."))
              }
              other=>Err(CoreServiceError::business("VALIDATION", format!("Unknown TECH Cockpit command: {other}")))
            }
        }.await;
        audit_result(&self.runtime,"tech",OP,context,decision,&result).await?;
        result
    }

}
