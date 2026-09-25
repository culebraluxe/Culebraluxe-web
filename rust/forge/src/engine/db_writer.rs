//! Forge state writer. SQL/persistence lives in db::ForgeEngineDao.

use crate::engine::vendor_session::with_shared;
use crate::engine::writer::ForgeStateWriter;
use db::ForgeEngineDao;

pub struct DbForgeStateWriter;

impl DbForgeStateWriter {
    pub fn connect_env() -> Result<Self, String> {
        if crate::engine::vendor_session::database_url().is_none() {
            return Err("DATABASE_URL / DATABASE_URL_DEV is not set".into());
        }
        Ok(Self)
    }

    fn run(&self, f: impl FnOnce(ForgeEngineDao, &tokio::runtime::Runtime) -> Result<(), String>) -> Result<(), String> {
        with_shared(|db, rt| f(ForgeEngineDao::new(db.clone()), rt))?
    }
}

impl ForgeStateWriter for DbForgeStateWriter {
    fn mark_story_in_progress(&self, story_id: &str) -> Result<(), String> {
        self.run(|dao, rt| rt.block_on(async {
            dao.mark_story_in_progress(story_id).await.map_err(|e| e.to_string())
        }))
    }

    fn mark_story_human_hold(&self, story_id: &str, reason: &str) -> Result<(), String> {
        self.run(|dao, rt| rt.block_on(async {
            dao.mark_story_human_hold(story_id, reason).await.map_err(|e| e.to_string())
        }))
    }

    fn mark_story_complete(&self, story_id: &str) -> Result<(), String> {
        self.run(|dao, rt| rt.block_on(async {
            dao.mark_story_complete(story_id).await.map_err(|e| e.to_string())
        }))
    }

    fn append_run_detail(&self, run_id: &str, detail: &str) -> Result<(), String> {
        self.run(|dao, rt| rt.block_on(async {
            dao.append_run_detail(run_id, detail).await.map_err(|e| e.to_string())
        }))
    }
}
