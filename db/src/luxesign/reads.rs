//! The envelope read queries, moved out of `luxesign.rs` (2026-10-14).
//!
//! `luxesign.rs` had grown past the boundedness ceiling. Both methods are `pub` on `LuxesignDao`, so the `impl`
//! block repeating the parent's (generic-free) head keeps every call path: the move is text-exact.

use super::*;

impl LuxesignDao {
    /// Envelopes past their clock with an open canonical status. The
    /// sweep transitions each to Expired after expiring its recipients.
    pub async fn overdue_envelopes_tx(&self, tx: &mut DbTransaction) -> DbResult<Vec<String>> {
        sqlx::query_scalar::<_, String>(
            r#"
            select ds.signature_request_id::text
              from luxesign_config ds
              join luxesign_request sr on sr.id = ds.signature_request_id
             where ds.expires_at is not null
               and ds.expires_at <= now()
               and sr.status in ('sent', 'viewed', 'signed')
             order by ds.expires_at
            "#,
        )
        .fetch_all(tx.connection())
        .await
        .map_err(|error| DbFailure::from_sqlx("luxesign.overdue", &error))
    }

    /// Recent native envelopes with recipient progress, newest first. The
    /// ops desk list; detail still comes from `config` + `recipients`.
    pub async fn list_envelopes(&self, limit: i64) -> DbResult<Vec<LuxesignEnvelopeSummary>> {
        let rows = sqlx::query_as::<_, EnvelopeSummaryRow>(
            r#"
            select ds.signature_request_id::text as signature_request_id,
                   sr.transaction_document_id::text as transaction_document_id,
                   ds.subject as subject,
                   ds.signing_mode as signing_mode,
                   sr.status as status,
                   ds.issued_at as issued_at,
                   ds.expires_at as expires_at,
                   (select r2.recipient_name from luxesign_envelope_recipient r2
                     where r2.signature_request_id = ds.signature_request_id
                     order by r2.signer_order, r2.id limit 1) as client_name,
                   count(r.id)::bigint as recipient_total,
                   count(case when s.state = 'completed' then 1 end)::bigint as completed_total
              from luxesign_config ds
              join luxesign_request sr on sr.id = ds.signature_request_id
              left join luxesign_envelope_recipient r on r.signature_request_id = ds.signature_request_id
              left join luxesign_recipient_state s on s.recipient_id = r.id
             group by ds.signature_request_id, sr.transaction_document_id, ds.subject,
                      ds.signing_mode, sr.status, ds.issued_at, ds.expires_at, sr.updated_at
             order by sr.updated_at desc
             limit $1
            "#,
        )
        .bind(limit.clamp(1, 100))
        .fetch_all(self.db.pool())
        .await
        .map_err(|error| DbFailure::from_sqlx("luxesign.list", &error))?;
        rows.into_iter()
            .map(|row| {
                Ok(LuxesignEnvelopeSummary {
                    signature_request_id: row.signature_request_id,
                    transaction_document_id: row.transaction_document_id,
                    subject: row.subject,
                    signing_mode: row.signing_mode,
                    status: row.status,
                    issued_at: row.issued_at.map(|value| value.to_rfc3339()),
                    expires_at: row.expires_at.map(|value| value.to_rfc3339()),
                    client_name: row.client_name,
                    recipient_total: row.recipient_total,
                    completed_total: row.completed_total,
                })
            })
            .collect()
    }
}
