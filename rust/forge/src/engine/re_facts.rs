//! Port of workflow_app/facts.ts + configuration.ts class B defaults.

use workflow::Value;

use crate::engine::vendor_session::with_shared;
use sqlx::Row;

pub fn financing_applicable_from_type(financing_type: Option<&str>) -> Option<bool> {
    match financing_type {
        Some("financed") => Some(true),
        Some("cash") => Some(false),
        _ => None,
    }
}

pub fn appraisal_applicable_from_required(raw: Option<&str>) -> Option<bool> {
    match raw.map(str::trim) {
        Some("t") | Some("true") | Some("1") => Some(true),
        Some("f") | Some("false") | Some("0") => Some(false),
        _ => None,
    }
}

fn insert_bool_opt(facts: &mut Value, k: &str, v: Option<bool>) {
    match v {
        Some(b) => facts.insert(k, Value::from(b)),
        None => facts.insert(k, Value::Null),
    }
}

fn insert_str_opt(facts: &mut Value, k: &str, v: Option<&str>) {
    match v.map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => facts.insert(k, Value::String(s.to_string())),
        None => facts.insert(k, Value::Null),
    }
}

/// Culebra, PR operating defaults. Application config, never engine behavior.
pub fn culebra_class_b(facts: &mut Value) {
    facts.insert("closingAgentRole", Value::String("notario".into()));
    facts.insert("requiresNotario", Value::from(true));
    facts.insert("requiresTitleCompany", Value::from(false));
    facts.insert("requiresCrimClearance", Value::from(true));
    facts.insert("requiresRegistryFollowup", Value::from(true));
    facts.insert("inspectionApplicable", Value::from(true));
    facts.insert("insuranceApplicable", Value::from(true));
    facts.insert("requiresSurvey", Value::from(false));
    facts.insert("requiresHoaClearance", Value::from(false));
    facts.insert("closingConfirmationRequired", Value::from(true));
}

/// `with_shared` nests two Results: the shared-pool lookup and the query itself. Collapse both, or `None`.
fn shared_ok<T>(result: Result<Result<T, String>, String>) -> Option<T> {
    result.ok().and_then(|inner| inner.ok())
}

pub fn resolve_legacy_deal_id_for_contract(contract_id: &str) -> Option<String> {
    // A NULL now arrives as `None` rather than as psql's `\N` sentinel, so the sentinel check that used to follow this
    // is gone with it. The cast is on the bind, not the column, so the primary-key index still gets used.
    shared_ok(with_shared(|db, rt| {
        rt.block_on(async {
            sqlx::query_scalar::<_, String>(
                "select f.deal_id::text from contract c
                   left join document_form_instance f on f.id = c.source_form_instance_id
                  where c.id = $1::uuid limit 1",
            )
            .bind(contract_id)
            .fetch_optional(db.pool())
            .await
            .map_err(|error| error.to_string())
        })
    }))
    .flatten()
    .filter(|value| !value.is_empty())
}

/// The nine text columns of a deal, read by name.
#[derive(Default)]
struct DealFactColumns {
    stage: String,
    financing_type: Option<String>,
    closing_date: Option<String>,
    inspection_deadline: Option<String>,
    financing_deadline: Option<String>,
    appraisal_required: Option<String>,
    lender_clear_to_close: Option<String>,
}

impl DealFactColumns {
    fn from_row(row: &sqlx::postgres::PgRow) -> Result<Self, String> {
        let text = |name: &str| -> Result<Option<String>, String> {
            let value: Option<String> = row.try_get(name).map_err(|error| error.to_string())?;
            Ok(value.filter(|value| !value.is_empty()))
        };
        Ok(Self {
            stage: row
                .try_get::<Option<String>, _>("stage")
                .map_err(|error| error.to_string())?
                .unwrap_or_default(),
            financing_type: text("financing_type")?,
            closing_date: text("closing_date")?,
            inspection_deadline: text("inspection_deadline")?,
            financing_deadline: text("financing_deadline")?,
            appraisal_required: text("appraisal_required")?,
            lender_clear_to_close: text("lender_clear_to_close")?,
        })
    }
}

pub fn deal_workflow_facts(deal_id: &str) -> Value {
    let mut facts = Value::object();
    facts.insert("dealId", Value::String(deal_id.into()));
    culebra_class_b(&mut facts);

    // The old statement also selected list_price and offer_price and never read them; they are gone rather than carried
    // along as dead columns. Every value comes back as text or NULL, exactly as the pipe-split version had it, so the
    // facts below are built the same way from the same values.
    let row = shared_ok(with_shared(|db, rt| {
        rt.block_on(async {
            sqlx::query(
                "select d.stage as stage,
                        coalesce(d.financing_type, '') as financing_type,
                        coalesce(d.closing_date::text, '') as closing_date,
                        coalesce(d.inspection_deadline::text, '') as inspection_deadline,
                        coalesce(d.financing_deadline::text, '') as financing_deadline,
                        coalesce(d.appraisal_required::text, '') as appraisal_required,
                        coalesce(d.lender_clear_to_close::text, '') as lender_clear_to_close
                   from deal d
                  where d.id = $1::uuid
                  limit 1",
            )
            .bind(deal_id)
            .fetch_optional(db.pool())
            .await
            .map_err(|error| error.to_string())
        })
    }))
    .flatten()
    .and_then(|row| DealFactColumns::from_row(&row).ok());

    let Some(cols) = row else {
        // No such deal, or a read that failed: the same facts the empty result used to produce.
        insert_bool_opt(&mut facts, "financingApplicable", None);
        insert_bool_opt(&mut facts, "appraisalApplicable", None);
        insert_bool_opt(&mut facts, "lenderClearToClose", None);
        facts.insert("closingDocumentsReady", Value::from(false));
        facts.insert("closingDateScheduled", Value::from(false));
        facts.insert("inspectionDeadlineScheduled", Value::from(false));
        facts.insert("financingDeadlineScheduled", Value::from(false));
        return facts;
    };

    let DealFactColumns {
        stage,
        financing_type,
        closing_date,
        inspection_deadline: inspection,
        financing_deadline: financing_dl,
        appraisal_required: appraisal,
        lender_clear_to_close: lender,
    } = cols;
    let closing_date = closing_date.as_deref();
    let inspection = inspection.as_deref();
    let financing_dl = financing_dl.as_deref();
    let appraisal = appraisal.as_deref();
    let lender = lender.as_deref();
    facts.insert("stage", Value::String(stage));
    insert_str_opt(&mut facts, "closingDate", closing_date);
    insert_str_opt(&mut facts, "inspectionDeadline", inspection);
    insert_str_opt(&mut facts, "financingDeadline", financing_dl);
    facts.insert("closingDateScheduled", Value::from(closing_date.is_some()));
    facts.insert(
        "inspectionDeadlineScheduled",
        Value::from(inspection.is_some()),
    );
    facts.insert(
        "financingDeadlineScheduled",
        Value::from(financing_dl.is_some()),
    );
    insert_bool_opt(
        &mut facts,
        "financingApplicable",
        financing_applicable_from_type(financing_type.as_deref()),
    );
    insert_bool_opt(
        &mut facts,
        "appraisalApplicable",
        appraisal_applicable_from_required(appraisal),
    );
    insert_bool_opt(
        &mut facts,
        "lenderClearToClose",
        appraisal_applicable_from_required(lender),
    );
    facts.insert(
        "closingDocumentsReady",
        Value::from(closing_documents_ready(deal_id)),
    );
    facts
}

fn closing_documents_ready(deal_id: &str) -> bool {
    let counts = shared_ok(with_shared(|db, rt| {
        rt.block_on(async {
            sqlx::query_as::<_, (i64, i64)>(
                "select count(*) filter (where status not in ('signed','final'))::bigint,
                        count(*)::bigint
                   from transaction_document where deal_id = $1::uuid",
            )
            .bind(deal_id)
            .fetch_one(db.pool())
            .await
            .map_err(|error| error.to_string())
        })
    }));
    // A read that failed means "not ready", which is what the old defaults (open=1, total=0) produced.
    let (open, total) = counts.unwrap_or((1, 0));
    total > 0 && open == 0
}

pub fn contract_workflow_facts(contract_id: &str) -> Value {
    let mut facts = Value::object();
    facts.insert("contractId", Value::String(contract_id.into()));
    culebra_class_b(&mut facts);
    if let Some(deal_id) = resolve_legacy_deal_id_for_contract(contract_id) {
        let mut deal = deal_workflow_facts(&deal_id);
        if let workflow::Value::Object(map) = &mut deal {
            for (k, v) in map.clone() {
                facts.insert(k, v);
            }
        }
        facts.insert("dealId", Value::String(deal_id));
    } else {
        facts.insert("closingDocumentsReady", Value::from(false));
        facts.insert("closingDateScheduled", Value::from(false));
        insert_bool_opt(&mut facts, "financingApplicable", None);
        insert_bool_opt(&mut facts, "appraisalApplicable", None);
        insert_bool_opt(&mut facts, "lenderClearToClose", None);
    }
    facts
}
