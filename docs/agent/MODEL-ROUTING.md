# Model routing — orchestrator's team map

Status: active 2026-10-19. Purpose: the orchestrator chooses which model each agent runs, instead of
every agent inheriting one default. Credentials are **session-local** (the Unix shell/login already
holds Claude/ChatGPT/DeepSeek/OpenCode auth). Routing does **not** manage secrets and must never
write them to a file or repo.

## Providers and harnesses (what exists)

| Provider | Invocation | Notes |
| --- | --- | --- |
| DeepSeek | DSH (primary coordinator), `DSH_MODEL` / profile default; OpenCode `deepseek/*` | daily driver |
| Meta/Muse | OpenCode `meta/muse-spark-*` | strong agentic/reasoning, T2 |
| OpenCode | `opencode` harness + its models (`opencode/*-free`) | free tier for bulk work |
| Claude | `claude` CLI (separate shell auth) | **T3 — 4 expensive models, never auto-routed** |
| ChatGPT | `codex` CLI (separate shell auth) | T3 expensive; read-only second opinion by default |

## Tiers

| Tier | Models | Use for | Rule |
| --- | --- | --- | --- |
| **T0 free** | `opencode/deepseek-v4.1-flash-free`, `opencode/glm-5.3-flash-free`, `opencode/nemotron-3-ultra-free`, `opencode/space-bunny-free` | bulk drafts, test scaffolding, parallel grunt work, first-pass research | default for fan-out; no authority |
| **T1 daily** | `deepseek/deepseek-v4-flash`, `deepseek/deepseek-flash`, `meta/muse-spark-1.3` | normal implementation, docs, ops, QA runs | default working tier |
| **T2 judgment** | `deepseek/deepseek-v4-pro`, `meta/muse-spark-1.3-contributor`, `meta/muse-wonder-1.0` | architecture, strategy, adversarial review, Council, complex debugging | orchestrator may choose without asking |
| **T3 ultra** | Claude family (4 expensive models), ChatGPT high-end | irreversible/high-stakes decisions, final legal/security review, Council escalation | **conditional only — see below** |

## Role → tier map

| Agent/role | Default tier | Escalate to T3 when |
| --- | --- | --- |
| Chief of Staff (orchestrator) | T1/T2 | one-way-door decision, contradictory staff verdicts |
| Strategist | T2 | major strategy commitment |
| Engineer | T1 (T2 for architecture) | security-critical design review |
| Operator | T1 | destructive/irreversible ops plan |
| Researcher | T1/T2 | evidence conflicting with high-stakes claim |
| Comms | T1 | high-stakes external communication (draft only; no send) |
| Finance/Commercial | T2 | contract/commitment analysis |
| Devil's Advocate | T2 | final red-team before an irreversible commitment |
| Forge Scout | T0/T1 | — |
| Forge Architect | T2 | architecture drift/security review |
| Forge Lead | T1/T2 | — |
| Forge Smith | T1 | — |
| Forge QA/Inspector | T1/T2 | final production sign-off |
| Forge Assay | T2 | — |
| Forge DevOps | T1 | production release/rollback |

## T3 (Claude/ChatGPT) conditions — mandatory

Use T3 only when **all** are true:
1. The principal explicitly requested it, **or** the decision is irreversible/high-stakes/Council.
2. T0–T2 have been attempted or are clearly insufficient (state why).
3. The call is logged with: reason, requested tier, which T3 model, expected output, and a stop
   condition.
4. It is a single consult, not an autonomous loop: **max 3 T3 calls per session** unless the
   principal raises the cap.
5. No T3 for routine code, tests, docs, formatting, or "second opinion" where T2 suffices.
6. Never use T3 to bypass an authority/security gate — model choice is not authority.

If a T3 call is proposed without the principal present, produce an escalation packet with the
question and the expected value; do not spend.

## How the pin is applied (mechanics)

- **Forge probes:** `forge harness-evolution probe --model <provider/model>`; identity records the
  actual model. OpenCode only accepts `*-free` models for probes.
- **DSH coordinator/agents:** `DSH_MODEL=<provider/model>` or the profile default; per-seat model
  override requires a DSH subagent-model plugin/profile (not installed yet — see below).
- **OpenCode harness:** `opencode run --model <provider/model>`; free models only for probe work.
- **Claude/ChatGPT:** separate CLIs, session-local auth; invoked only under the T3 conditions.

## Known gap (to close)

The harness's `spawn_teammate` tool does not expose a per-spawn model parameter today; teammates
inherit the session model. To make role→model automatic we need one of:
1. a DSH profile per tier (e.g. `headless-t3`) launched with `DSH_MODEL`, or
2. a vetted subagent model-picker plugin (e.g. `@zim9729/dsh-subagent-model-picker`) installed in the
   profile, or
3. for Forge roles, set the model via the probe/CLI path (already supported).

Until then the orchestrator can honour the map by choosing the **profile/CLI** used to launch an
agent and by pinning Forge runs; T3 stays manual by design.

## Principal controls

- "use T3 for this" — approves one T3 consult.
- "stay T1/T2" — hard cap for the session.
- "free only" — T0/T1 only (no paid models).
- Budget rule: over N=120 minutes is flagged, not blocked (see `RUN-BUDGET.md`).
