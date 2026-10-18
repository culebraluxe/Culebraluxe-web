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
  routing/usage/cost data, and the executor identity in the probe spec still says `opencode`
  while the actual sandbox was `dsh` (known defect).
- **Decision: no adoption, no experience writeback.** Automatic adoption stays OFF. Do not treat the
  elapsed delta as a harness win.
- Rows kept in DEV for audit: experiment, candidate, 2 probes, contrast, 2 revisions.

## Defects / follow-ups

1. Probe identity executor is `opencode` even though the sandbox executed `dsh` — the identity must
   name the actual executor (code fix).
2. DSH usage not captured (`session_id=null`, tokens/cost None): session discovery under the
   sandbox's sanitized `DSH_HOME` did not find the log; usage is honestly NOT CAPTURED, not zero.
3. One story is insufficient for a verdict; the next run should cover 3–5 stories.

## OpenCode deferral

OpenCode was the intended backend, but its infrastructure was attacked on 2026-10-18 and provider
routing failed before inference (`provider.no-route`). Deferred as tech debt; DSH is primary.
Revisit OpenCode when its service/provider routing is stable.
