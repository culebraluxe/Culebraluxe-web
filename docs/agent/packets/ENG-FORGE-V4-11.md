# ENG-FORGE-V4-11 — Slack Run Notifications

## Goal
Mirror useful Forge run events to Slack so Chris can watch Forge from his phone and ChatGPT can inspect the same channel without terminal copy/paste. Neon remains the system of record and Slack remains optional.

## Scope
Add one small, optional Slack notification adapter to the local Forge worker path.

- Use a single environment variable: `FORGE_SLACK_WEBHOOK_URL`.
- When unset, Forge behavior is unchanged and no network call is attempted.
- When set, post concise human-readable notifications for:
  - work claimed / lane started,
  - successful lane completion including candidate commit when present,
  - follow to next lane (for example Smith -> Assay),
  - terminal Hold/Error/Assay-failure conditions.
- Slack delivery is strictly fail-open: timeout, non-2xx response, malformed URL, or network failure must never fail, Hold, retry, or otherwise alter Forge execution.
- Do not put secrets, database URLs, provider keys, prompts, or full model transcripts in Slack.
- Include durable identifiers that let a human correlate back to Neon: story id, work item id, role/profile, and external run id when available.
- Prefer native Node `fetch`; no new Slack SDK dependency.
- Keep formatting plain and phone-readable.

## Architect brief
Slack is a human cockpit only. Forge/Neon own truth. The notifier must sit outside orchestration semantics: it observes lifecycle outcomes and mirrors them. It must not become a dependency of claim, execution, commit, Assay, or story completion. One-way only for this story; no inbound Slack commands yet.

The first implementation may post to the channel timeline rather than maintaining one thread per story. Thread correlation can be a later enhancement if it requires bot-token state or extra persistence. Do not add schema for Slack.

## Acceptance criteria
1. `FORGE_SLACK_WEBHOOK_URL` absent: no request is made and existing Forge behavior is unchanged.
2. Valid webhook: expected start/completion/follow/failure events produce concise payloads.
3. Slack network error, timeout, invalid response, or non-2xx result cannot fail or mutate the Forge run.
4. Notification payload includes story id and work item id; completion may also include external run id and commit SHA when available.
5. No secrets or full prompt/transcript content are sent.
6. No schema changes.
7. No changes to Smith/Assay lane selection, V4-08 execution-contract behavior, V4-09 harness-owned commit semantics, or V4-10 progress telemetry semantics.
8. Focused unit tests cover disabled notifier, successful POST payload, and fail-open delivery failure.

## Test mode
Scoped only. Do not run the full regression suite.

## Assay commands
- `pnpm exec tsx --test agent-runtime/slack-notifier.test.ts`
- `pnpm exec tsx --test agent-runtime/harness-owned-commit.test.ts agent-runtime/readiness.test.ts agent-runtime/gateway/cli-agent-adapter.test.ts agent-runtime/gateway/provider.test.ts agent-runtime/team.test.ts agent-runtime/orchestrate.test.ts agent-runtime/orchestrate-apply.test.ts agent-runtime/repositories.assay.test.ts`

## Skills
- TypeScript
- Node fetch / HTTP error handling
- Forge worker lifecycle
- test seams / dependency injection

## Loop
Architect packet -> Smith -> candidate commit -> Assay -> main. Preserve Forge code automatically through the harness-owned commit path. No swarm, no provider change, no schema.