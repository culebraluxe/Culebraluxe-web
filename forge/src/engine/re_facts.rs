//! Port of workflow_app/facts.ts + configuration.ts class B defaults.

use db::{DealWorkflowFactRow, ForgeEngineDao};
use workflow::Value;

use crate::engine::vendor_session::with_shared;

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

/// `with_shared` nests two Results: the shared-pool lookup and the repository call. Collapse both, or `None`.
fn shared_ok<T>(result: Result<Result<T, String>, String>) -> Option<T> {
    result.ok().and_then(|inner| inner.ok())
}

pub fn resolve_legacy_deal_id_for_contract(contract_id: &str) -> Option<String> {
    shared_ok(with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.deal_id_for_contract(contract_id)
                .await
                .map_err(|error| error.to_string())
        })
    }))
    .flatten()
    .filter(|value| !value.is_empty())
}

pub fn deal_workflow_facts(deal_id: &str) -> Value {
    let mut facts = Value::object();
    facts.insert("dealId", Value::String(deal_id.into()));
    culebra_class_b(&mut facts);

    let row = shared_ok(with_shared(|db, rt| {
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.deal_workflow_facts(deal_id)
                .await
                .map_err(|error| error.to_string())
        })
    }))
    .flatten();

    let Some(DealWorkflowFactRow {
        stage,
        financing_type,
        closing_date,
        inspection_deadline: inspection,
        financing_deadline: financing_dl,
        appraisal_required: appraisal,
        lender_clear_to_close: lender,
    }) = row
    else {
        insert_bool_opt(&mut facts, "financingApplicable", None);
        insert_bool_opt(&mut facts, "appraisalApplicable", None);
        insert_bool_opt(&mut facts, "lenderClearToClose", None);
        facts.insert("closingDocumentsReady", Value::from(false));
        facts.insert("closingDateScheduled", Value::from(false));
        facts.insert("inspectionDeadlineScheduled", Value::from(false));
        facts.insert("financingDeadlineScheduled", Value::from(false));
        return facts;
    };

    let closing_date = closing_date.as_deref();
    let inspection = inspection.as_deref();
    let financing_dl = financing_dl.as_deref();
    let appraisal = appraisal.as_deref();
    let lender = lender.as_deref();
    facts.insert("stage", Value::String(stage.unwrap_or_default()));
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
        let dao = ForgeEngineDao::new(db.clone());
        rt.block_on(async {
            dao.closing_document_counts(deal_id)
                .await
                .map_err(|error| error.to_string())
        })
    }));
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
