//! Port of workflow_app/facts.ts + configuration.ts class B defaults.

use workflow::Value;

use crate::engine::vendor_session::{psql_query, sql_literal};

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

pub fn resolve_legacy_deal_id_for_contract(contract_id: &str) -> Option<String> {
    let sql = format!(
        "SELECT f.deal_id::text FROM contract c \
         LEFT JOIN document_form_instance f ON f.id = c.source_form_instance_id \
         WHERE c.id = {} LIMIT 1",
        sql_literal(contract_id)
    );
    psql_query(&sql).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && s != "\\N")
}

pub fn deal_workflow_facts(deal_id: &str) -> Value {
    let sql = format!(
        "SELECT d.stage, COALESCE(d.financing_type,''), COALESCE(d.closing_date::text,''), \
                COALESCE(d.inspection_deadline::text,''), COALESCE(d.financing_deadline::text,''), \
                COALESCE(d.appraisal_required::text,''), COALESCE(d.lender_clear_to_close::text,''), \
                COALESCE(d.list_price::text,''), COALESCE(d.offer_price::text,'') \
         FROM deal d WHERE d.id = {} LIMIT 1",
        sql_literal(deal_id)
    );
    let raw = psql_query(&sql).unwrap_or_default();
    let mut facts = Value::object();
    facts.insert("dealId", Value::String(deal_id.into()));
    culebra_class_b(&mut facts);
    if raw.trim().is_empty() {
        insert_bool_opt(&mut facts, "financingApplicable", None);
        insert_bool_opt(&mut facts, "appraisalApplicable", None);
        insert_bool_opt(&mut facts, "lenderClearToClose", None);
        facts.insert("closingDocumentsReady", Value::from(false));
        facts.insert("closingDateScheduled", Value::from(false));
        facts.insert("inspectionDeadlineScheduled", Value::from(false));
        facts.insert("financingDeadlineScheduled", Value::from(false));
        return facts;
    }
    let cols: Vec<&str> = raw.split('|').map(str::trim).collect();
    let stage = cols.first().copied().unwrap_or("");
    let financing_type = cols.get(1).copied().filter(|s| !s.is_empty());
    let closing_date = cols.get(2).copied().filter(|s| !s.is_empty());
    let inspection = cols.get(3).copied().filter(|s| !s.is_empty());
    let financing_dl = cols.get(4).copied().filter(|s| !s.is_empty());
    let appraisal = cols.get(5).copied().filter(|s| !s.is_empty());
    let lender = cols.get(6).copied().filter(|s| !s.is_empty());
    facts.insert("stage", Value::String(stage.into()));
    insert_str_opt(&mut facts, "closingDate", closing_date);
    insert_str_opt(&mut facts, "inspectionDeadline", inspection);
    insert_str_opt(&mut facts, "financingDeadline", financing_dl);
    facts.insert("closingDateScheduled", Value::from(closing_date.is_some()));
    facts.insert("inspectionDeadlineScheduled", Value::from(inspection.is_some()));
    facts.insert("financingDeadlineScheduled", Value::from(financing_dl.is_some()));
    insert_bool_opt(&mut facts, "financingApplicable", financing_applicable_from_type(financing_type));
    insert_bool_opt(&mut facts, "appraisalApplicable", appraisal_applicable_from_required(appraisal));
    insert_bool_opt(&mut facts, "lenderClearToClose", appraisal_applicable_from_required(lender));
    facts.insert("closingDocumentsReady", Value::from(closing_documents_ready(deal_id)));
    facts
}

fn closing_documents_ready(deal_id: &str) -> bool {
    let sql = format!(
        "SELECT COUNT(*) FILTER (WHERE status NOT IN ('signed','final')), COUNT(*) \
         FROM transaction_document WHERE deal_id = {}",
        sql_literal(deal_id)
    );
    let raw = psql_query(&sql).unwrap_or_default();
    let mut parts = raw.split('|');
    let open: i64 = parts.next().unwrap_or("1").trim().parse().unwrap_or(1);
    let total: i64 = parts.next().unwrap_or("0").trim().parse().unwrap_or(0);
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
