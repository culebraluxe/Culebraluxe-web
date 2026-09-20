//! Port of workflow_app/forge/db-state-writer.ts.
//! Same SQL as db/forge-story-state.ts and db/forge-run.ts. No new tables.

use crate::engine::vendor_session::{psql_query, sql_literal};
use crate::engine::writer::ForgeStateWriter;

pub struct DbForgeStateWriter;

impl DbForgeStateWriter {
    pub fn connect_env() -> Result<Self, String> {
        if crate::engine::vendor_session::database_url().is_none() {
            return Err("DATABASE_URL / DATABASE_URL_DEV is not set".into());
        }
        Ok(Self)
    }
}

impl ForgeStateWriter for DbForgeStateWriter {
    fn mark_story_in_progress(&self, story_id: &str, ) -> Result<(), String> {
        let id = sql_literal(story_id);
        psql_query(&format!(
            "UPDATE storyboard_story
                SET status = 'In Progress', completion = 0, completed_at = NULL, updated_at = now()
              WHERE id = {id}"
        )).map(|_| ())
    }

    fn mark_story_human_hold(&self, story_id: &str, reason: &str) -> Result<(), String> {
        let id = sql_literal(story_id);
        let reason = reason.trim();
        let sql = if reason.is_empty() {
            format!(
                "UPDATE storyboard_story
                    SET status = 'Hold', completed_at = NULL, updated_at = now()
                  WHERE id = {id}"
            )
        } else {
            let r = sql_literal(reason);
            format!(
                "UPDATE storyboard_story
                    SET status = 'Hold',
                        completed_at = NULL,
                        notes = CASE
                          WHEN notes IS NULL OR notes = '' THEN {r}
                          ELSE notes || E'\\n' || {r}
                        END,
                        updated_at = now()
                  WHERE id = {id}"
            )
        };
        psql_query(&sql).map(|_| ())
    }

    fn mark_story_complete(&self, story_id: &str) -> Result<(), String> {
        let id = sql_literal(story_id);
        psql_query(&format!(
            "UPDATE storyboard_story
                SET status = 'Complete',
                    completion = 100,
                    completed_at = COALESCE(completed_at, now()),
                    updated_at = now()
              WHERE id = {id}"
        )).map(|_| ())
    }

    fn append_run_detail(&self, run_id: &str, detail: &str) -> Result<(), String> {
        let text = detail.trim();
        if text.is_empty() {
            return Ok(());
        }
        let id = sql_literal(run_id);
        let d = sql_literal(text);
        psql_query(&format!(
            "UPDATE storyboard_story_run
                SET evidence_detail = CASE
                      WHEN evidence_detail IS NULL OR evidence_detail = ''
                        THEN to_char(now(), 'YYYY-MM-DD HH24:MI:SS') || ' — ' || {d}
                      ELSE evidence_detail || E'\\n' || to_char(now(), 'YYYY-MM-DD HH24:MI:SS') || ' — ' || {d}
                    END,
                    updated_at = now()
              WHERE id = {id}"
        )).map(|_| ())
    }
}
