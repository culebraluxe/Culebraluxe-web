//! Port of qa-repair-policy.ts. Pure. No DB.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QaVerdict { Pass, Fail }
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QaDisposition { Repair, Replan, Escalate }

pub const QA_PASS_DISPOSITION: &str = "PASS";
pub const QA_STORED_DISPOSITIONS: &[&str] = &["PASS", "REPAIR", "REPLAN", "ESCALATE"];
pub const QA_UNKNOWN_DISPOSITION: &str = "UNKNOWN";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QaDispositionReading { Pass, Repair, Replan, Escalate, Unknown }

pub fn classify_stored_qa_disposition(raw: Option<&str>) -> Option<QaDispositionReading> {
    let text = raw?.trim();
    if text.is_empty() { return None; }
    Some(match text {
        "PASS" => QaDispositionReading::Pass,
        "REPAIR" => QaDispositionReading::Repair,
        "REPLAN" => QaDispositionReading::Replan,
        "ESCALATE" => QaDispositionReading::Escalate,
        _ => QaDispositionReading::Unknown,
    })
}

pub fn stored_reading_to_qa_result(reading: Option<QaDispositionReading>) -> (QaVerdict, Option<QaDisposition>) {
    match reading {
        Some(QaDispositionReading::Pass) => (QaVerdict::Pass, None),
        Some(QaDispositionReading::Repair) => (QaVerdict::Fail, Some(QaDisposition::Repair)),
        Some(QaDispositionReading::Replan) => (QaVerdict::Fail, Some(QaDisposition::Replan)),
        Some(QaDispositionReading::Escalate) => (QaVerdict::Fail, Some(QaDisposition::Escalate)),
        _ => (QaVerdict::Fail, None),
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RepairAttemptState { pub repair_attempts: u32, pub replan_attempts: u32 }
#[derive(Debug, Clone, Copy)]
pub struct RepairBudget { pub max_repair_attempts: u32, pub max_replan_attempts: u32 }
impl Default for RepairBudget {
    fn default() -> Self { Self { max_repair_attempts: 3, max_replan_attempts: 2 } }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepairRouting {
    Pass,
    Smith { repair_attempts: u32 },
    Architect { replan_attempts: u32 },
    Hold { reason: String },
}

pub fn route_qa_result(
    verdict: QaVerdict, disposition: Option<QaDisposition>, state: RepairAttemptState,
    budget: RepairBudget, no_progress: bool, verification_gap: bool,
) -> RepairRouting {
    if verdict == QaVerdict::Pass { return RepairRouting::Pass; }
    if verification_gap {
        return RepairRouting::Hold { reason: "VERIFICATION GAP: QA could not form/run a valid assay command plan (missing ## Assay commands or no frozen plan). Repair cannot help; fix the packet/config, then re-dispatch.".into() };
    }
    if no_progress {
        return RepairRouting::Hold { reason: "NO_PROGRESS: the same candidate SHA re-failed the same machine classification with no new candidate in between — do not auto-launch another repair cycle.".into() };
    }
    match disposition {
        None => RepairRouting::Hold { reason: "QA FAIL without a legal disposition (got None) — engine cannot choose a safe repair route; operator/Lead review required.".into() },
        Some(QaDisposition::Escalate) => RepairRouting::Hold { reason: "QA requested ESCALATE — operator/Lead intervention required.".into() },
        Some(QaDisposition::Repair) => {
            if state.repair_attempts >= budget.max_repair_attempts {
                RepairRouting::Hold { reason: format!("Repair budget exhausted ({}/{}) — operator/Lead required.", state.repair_attempts, budget.max_repair_attempts) }
            } else {
                RepairRouting::Smith { repair_attempts: state.repair_attempts + 1 }
            }
        }
        Some(QaDisposition::Replan) => {
            if state.replan_attempts >= budget.max_replan_attempts {
                RepairRouting::Hold { reason: format!("Replan budget exhausted ({}/{}) — operator/Lead required.", state.replan_attempts, budget.max_replan_attempts) }
            } else {
                RepairRouting::Architect { replan_attempts: state.replan_attempts + 1 }
            }
        }
    }
}
