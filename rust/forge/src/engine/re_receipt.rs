//! workflow_command_receipt claim-first. Same table as db/workflow-command-receipt.ts.
//!
//! Binds on the workspace pool. The previous version built every statement with `format!`, escaped each value by hand
//! with `sql_literal`, spelled a missing actor into the SQL as the literal `NULL`, and then parsed psql's text output by
//! splitting on `|`. Two things that fixes: a message containing a pipe is no longer truncated, and the column types
//! are now the database's business (`command_id` is text; `aggregate_id` and `actor_app_user_id` are uuid and say so).

use crate::engine::vendor_session::with_shared;
use sqlx::Row;

pub struct Receipt {
    pub outcome: String,
    pub message: Option<String>,
}

pub fn claim_receipt(command_id: &str, actor: Option<&str>) -> Result<Option<Receipt>, String> {
    let claimed = with_shared(|db, rt| {
        rt.block_on(async {
            sqlx::query(
                "insert into workflow_command_receipt (command_id, outcome, aggregate_id, message, actor_app_user_id)
                 values ($1, 'pending', null, null, $2::uuid)
                 on conflict (command_id) do nothing
                 returning command_id",
            )
            .bind(command_id)
            .bind(actor)
            .fetch_optional(db.pool())
            .await
            .map_err(|error| error.to_string())
        })
    })??;
    // A row came back: this call won the claim, so there is no previous outcome to report.
    if claimed.is_some() {
        return Ok(None);
    }

    let row = with_shared(|db, rt| {
        rt.block_on(async {
            sqlx::query(
                "select outcome, coalesce(message, '') as message
                   from workflow_command_receipt where command_id = $1 limit 1",
            )
            .bind(command_id)
            .fetch_optional(db.pool())
            .await
            .map_err(|error| error.to_string())
        })
    })??;

    let Some(row) = row else { return Ok(None) };
    let outcome: String = row.try_get("outcome").map_err(|error| error.to_string())?;
    let message: Option<String> = row.try_get("message").map_err(|error| error.to_string())?;
    let message = message.filter(|value| !value.is_empty());
    if outcome == "pending" {
        Ok(None)
    } else {
        Ok(Some(Receipt { outcome, message }))
    }
}

pub fn finalize_receipt(
    command_id: &str,
    outcome: &str,
    aggregate_id: Option<&str>,
    message: Option<&str>,
) -> Result<(), String> {
    with_shared(|db, rt| {
        rt.block_on(async {
            sqlx::query(
                "update workflow_command_receipt
                    set outcome = $2, aggregate_id = $3::uuid, message = $4
                  where command_id = $1",
            )
            .bind(command_id)
            .bind(outcome)
            .bind(aggregate_id)
            .bind(message)
            .execute(db.pool())
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
        })
    })??;
    Ok(())
}
