# First dogfood run — DSH backend — 2026-10-18

Status: **executed; real DSH turns; candidate matched the base and was faster; NOT adopted.**
Backend switched from OpenCode (deferred — infrastructure incident 2026-10-18) to DSH.

## Setup

- Domain `rust-backend`, k=1, prompt/tool-guidance only.
- Base `d1-dogfood-rust-base-20261018` (`8272825f…`), candidate
  `d1-dogfood-rust-cand-20261018` (`2725b371…`), one green SystemPrompt change.
- Task: story `TST-FORGE-CLAIM-001-owned-tsth-18dd8a00ee00ef20`, run
  `fa94dff8-654b-47b9-a88a-612818700b10`.
- Backend `dsh` 0.1.1-rc.2, profile `headless`, no explicit model pin (profile default),
  authorized `--authorize-model-run`, two turns.
- Experiment `probe-TST-FORGE-CLAIM-001-owned-tsth-18dd8a00e-1792353093608471000`
  (state `compared`).

## Evidence

- Receipts: reference exit 0, 166,095 ms; candidate exit 0, 68,004 ms; both `model=deepseek-chat`,
  `model_pin_applied=false`, `session_id=null`, `usage_source=unmeasured`, tokens/cost `None`.
- Contrast: **elapsed improved 166,095 → 68,004 ms**; success 1→1, tests_passed 1→1, tests_failed
  0→0, qa 1→1, all other dimensions unchanged; **tokens and cost not captured**.
- No errors, no regressions; nothing adopted (`forge_harness_activation` = 0).

## Review (D2)

- Candidate behavior was **not worse** on this task and completed faster. That is a weak, single-task
  signal, not evidence of a general improvement: one historical story, no independent QA, no
  routing/usage/cost data. (The probe-spec executor identity defect noted at run time was fixed
  afterwards: the spec now records the actual `dsh`/`command` backend; see
  `probe_identity_names_the_actual_executor_and_model`. DSH version pinning landed 2026-10-19, see
  `docs/agent/SOP-DSH-UPGRADE.md`.)
- **Decision: no adoption, no experience writeback.** Automatic adoption stays OFF. Do not treat the
  elapsed delta as a harness win.
- Rows kept in DEV for audit: experiment, candidate, 2 probes, contrast, 2 revisions.

## Defects / follow-ups (status 2026-10-19)

1. Probe identity executor said `opencode` while the sandbox executed `dsh` — **FIXED**: the probe
   spec now records the actual backend (`dsh`/`command`) and the explicit model run.
2. DSH usage not captured at first (`session_id=null`, tokens/cost None) — **PARTIALLY FIXED**:
   canonical session discovery now meters 8/10 turns in the 5-story cohort; cost stays
   NOT CAPTURED until the operator sets `DSH_PRICE_*` rates (never zero).
3. One story was insufficient — **RESOLVED** by the 5-story cohort (7 improved / 2 regressed /
   65 unchanged; mixed signal; no adoption).
4. This run predates the FORGE-02 version pin — future model-turn probes fail closed unless
   `dsh --version` matches `DSH_EXPECTED_VERSION` (default 0.1.1-rc.2) and record the verified
   version in the receipt and probe evidence (`model-turn:vendor-version:`); this run's vendor was
   `dsh 0.1.1-rc.2`, which is now the pinned default.

## OpenCode deferral

OpenCode was the intended backend, but its infrastructure was attacked on 2026-10-18 and provider
routing failed before inference (`provider.no-route`). Deferred as tech debt; DSH is primary.
Revisit OpenCode when its service/provider routing is stable.

## Cohort — 5 stories (2026-10-18, after fixes)

Backend DSH, model `deepseek-official/deepseek-v4-flash`, k=1, base/candidate as above, 900s turn
budget. All 5 exit 0, state `compared`, nothing adopted.

| Story | Contrast | Elapsed (ref→cand) | Tokens (ref→cand) |
| --- | --- | --- | --- |
| CRM-14F | 1 improved, 13 unchanged, 2 not captured | 303s→193s improved | unmeasured (pre-discovery run) |
| CRM-14J | 2 improved, 13 unchanged, 1 not captured | 233s→119s improved | 86,885→55,244 improved |
| APP-WHATSAPP-ATTRIBUTION-01 | 2 improved, 13 unchanged, 1 not captured | 247s→154s improved | 80,052→60,628 improved |
| AUTH-CAPABILITIES-01 | 1 improved, **1 regressed**, 13 unchanged, 1 not captured | 303s→203s improved | 66,789→73,819 **regressed** |
| CRM-14H | 1 improved, **1 regressed**, 13 unchanged, 1 not captured | 206s→217s **regressed** | 124,805→104,000 improved |

Aggregate: **7 improved, 2 regressed, 65 unchanged, 6 not captured**. Token coverage 8/10 probe
turns (input 444,578 / output 207,644); cost 0 priced turns (NULL/unknown, never 0). Wall 36.3 min
(≈7.3 min/story) — well under the 12h flag.

## Decision (D2)

- The candidate is **not a clear win**: faster in 4/5 and token-lighter in 3/5, but AUTH used 10.5%
  more tokens and CRM-14H was 5% slower. Behavior dimensions (success/tests/QA) are unchanged.
- **Do not adopt.** Keep the base harness. The signal is worth one more cohort with cost rates
  configured and more stories, not a promotion.
- No experience writeback: adoption was not attempted, so there is no adopted/rejected decision to
  write back. Recorded here for the next review.
- Rows kept in DEV: 5 compared experiments (+1 failed pre-fix) with candidates/probes/contrasts.

## Controlled cohort (pre-registered, interleaved) — 2026-10-18

Pre-registered before data: fixed order, side order alternating (1/3/5 reference-first, 2/4
candidate-first), 5 stories incl. the two prior regressions, pinned DSH model, 900s turns, cost
rates exported. Mechanical criteria all PASS: 5/5 probes exit 0, 10/10 turns exit 0, usage 10/10
measured, cost 10/10 priced (placeholder rates), identity correct, order matched, wall 25m44s,
activation rows 0.

| # | Story | Side order | Contrast | Elapsed | Tokens |
|---|---|---|---|---|---|
| 1 | AUTH-CAPABILITIES-01 | ref-first | 2 imp, 1 reg (cost) | 116.6→77.8s ✓ | 41.9k→41.3k ✓ |
| 2 | CRM-14H | cand-first | 3 improved | 237.0→166.0s ✓ | 113.6k→95.3k ✓ |
| 3 | CRM-14J | ref-first | 3 regressed | 147.4→200.5s ✗ | 72.8k→73.6k ✗ |
| 4 | APP-WHATSAPP-ATTRIBUTION-01 | cand-first | 2 imp, 1 reg | 104.3→87.3s ✓ | 47.2k→48.0k ✗ |
| 5 | CRM-14F | ref-first | 3 regressed | 88.1→293.2s ✗ | 48.3k→85.2k ✗ |

**Aggregate: 7 improved, 8 regressed, 65 unchanged, 0 not captured.** Tokens 467,288 in / 199,776
out; cost $0.595 at placeholder rates; 25m44s.

**Observations:** the two previously-known regressions did **not** replicate (AUTH improved
elapsed/tokens, CRM-14H improved all three); instead CRM-14J and CRM-14F (both reference-first)
regressed on elapsed/tokens/cost. That pattern is consistent with run-order/session variance, and
the exploratory cohort's positive signal is **not confirmed**. This is why the rule was mechanical
only — the behavioral result is mixed/negative.

**Decision: no adoption.** Keep base harness. The candidate is not supported by controlled
evidence. Do not run a third cohort until the variance source is understood (order/warmth, session
state, model non-determinism); the next experiment should isolate order, not add stories.

### Free OpenCode arm (sanity, not decision evidence)

Same stories/order on `opencode/deepseek-v4.1-flash-free`: 5/5 exit 0, 10/10 turns, identity
executor=opencode; contrasts 5 improved (elapsed), 0 regressed, 65 unchanged, 10 not captured
(usage export unwired); 37m33s. Candidate faster on elapsed in 5/5, but no token/cost comparison —
not adoption evidence.
