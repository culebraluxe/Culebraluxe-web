# Principal decisions needed — Harness Evolution + CTO Office

Status: blocked on the principal (2026-10-18). Everything else on
`docs/agent/ACTIVE-NOTECARD.md` is DONE or intentionally OFF. Answer these to unblock the cycle.

## 1. Paid dogfood run (D1 → D2)

The dogfood pipeline is wired and verified end-to-end on DEV with the command sandbox (seeded
base/candidate revisions, real historical story/run, experiment/candidate/2 probes/contrast
persisted, SELECT-verified, cleaned). The only remaining D1 step is the real model turn.

Decision: approve **two paid OpenCode turns** (one reference, one candidate) for the first dogfood
experiment? `dsh` is not installed on this Mac, so `--model-backend dsh` fails closed before spend.

Exact command after seeding two verified DEV revisions:

```sh
cd /Users/Shared/dev/src/lane-deep
set -a; . /Users/Shared/dev/.env.local; set +a
cargo run -q -p forge-cli -- harness-evolution probe \
  --domain rust-backend --story <REAL_STORY_ID> --run <REAL_RUN_ID> \
  --base-revision <DEV_BASE_REVISION_ID> --candidate-revision <DEV_CANDIDATE_REVISION_ID> \
  --authorize-model-run --model-backend opencode \
  --environment development --target dev --format json
```

After approval: run it, review the contrast manually (D2), write the first experience/correction
rows, and keep automatic adoption OFF.

## 2. CTO Office principal answers (C2, then C4/C5)

1. Money threshold: what amount always requires confirmation (L4)?
2. High-stakes external communications: all sends, or a defined subset (people/domains/topics)?
3. First real skill: which weekly task should become the office's first skill (measurable output)?
4. Off-machine backup target for CTO Office memory (git remote / encrypted storage / other)?

Until answered, conservative defaults are in force in `cto-office/constitution/AUTHORITY.md` and
`PRINCIPAL.md`; C4/C5 stay blocked.

## 3. Optional but recommended after D2

- Approve (or decline) the first **experience writeback** from a real experiment.
- Decide whether model-role metrics/calibration should feed routing automatically or stay read-only.
- Decide when automatic adoption may ever be considered (currently OFF; requires production evidence).

## What is already done (no decision needed)

H1–H6 hardening, O1–O3 operator surface, D1 sandbox + probe wiring, CTO Office Phase 1 + local
repo. Notecard has the details and commit references.
