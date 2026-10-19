# Model routing — principal's authoritative roster

Status: active 2026-10-19 (supersedes earlier generic tier examples). Credentials are **session-local**
(Unix shell already holds Claude/ChatGPT/DeepSeek/OpenCode auth); routing never stores or prints
secrets.

## Roster (authoritative, ranked)

### T1 — Daily Drivers
1. `deepseek-v4.1-flash-free`
2. `meta/muse-spark-1.3-contributor`
3. `opencode/nemotron-3.5-super-vl-free`
4. Claude Haiku 5.5
5. `opencode/space-bunny-free`
6. `opencode/mimo-v2.6-flash-free`
7. GPT 6.0 Luna High
8. GPT-6.1 Sol Medium

### T2 — Judgement
Baseline candidate: `opencode/deepseek-v4.1-flash-free`
1. `opencode/nemotron-3-ultra-free`
2. `deepseek/deepseek-v4-pro`
3. Claude Sonnet 5.5
4. GPT-6.1 Sol High

### T3 — Ultra (conditional; expensive)
- `meta/muse-wonder-1.0`
- Claude Fable 5.1
- Claude Opus 5.5
- Grok
- GPT-6.0 Astra Light
- GPT-6.0 Astra Medium

### T3 — Experimental (free; experiments only, never authority)
- `opencode/glm-5.3-flash-free`
- `opencode/longcat-2.5-preview-free`
- `opencode/nemotron-3.5-lightning-free`
- `opencode/beam-free`
- `opencode/big-pickle`

Model IDs not visible in `opencode models` (Claude/GPT/Grok entries) are invoked through their own
CLIs with the session credentials; do not invent API ids for them.

## Selection algorithm

1. Determine the task tier from the role map (below).
2. Pick the **highest-ranked available** model in that tier; on failure/unavailability, fall back
   down the ranked list before crossing tiers.
3. T1/T2 may be chosen by the orchestrator without asking.
4. **T3 Ultra requires all of:** principal request OR irreversible/high-stakes/Council; T0–T2 tried
   or clearly insufficient (state why); call logged (reason/model/expected output/stop condition);
   max 3 Ultra calls/session unless the principal raises it; never for routine code/tests/docs; never
   to bypass authority/security.
5. **T3 Experimental** models may be used for experiments/spikes, but never as the authority for a
   decision or production change; same logging, free so no cost gate.
6. Budget: over N=120 minutes is flagged, not blocked (`RUN-BUDGET.md`).

## Role → tier

| Role | Tier | Notes |
| --- | --- | --- |
| Chief of Staff (orchestrator) | T2 (fallback T1) | one-way-door decisions may request Ultra |
| Strategist | T2 | major commitment may request Ultra |
| Engineer | T1 (T2 for architecture) | security-critical design may request Ultra |
| Operator | T1 | destructive plan may request Ultra |
| Researcher | T1/T2 | conflicting high-stakes evidence → T2 |
| Comms | T1 | high-stakes external comm: draft only, no send |
| Finance/Commercial | T2 | contract/commitment analysis |
| Devil's Advocate | T2 | pre-commit red team may request Ultra |
| Forge Scout | T1 | bulk/first pass |
| Forge Architect | T2 | architecture/security review |
| Forge Lead | T2 (fallback T1) | — |
| Forge Smith | T1 | implementation |
| Forge QA/Inspector | T2 (fallback T1) | production sign-off may request Ultra |
| Forge Assay | T2 | — |
| Forge DevOps | T1 | production release/rollback |

## Pin mechanics

- Forge probes: `--model <provider/model>`; actual model recorded in evidence.
- OpenCode: `opencode run --model <id>`; free models only for probe work.
- DSH: `DSH_MODEL` or profile default.
- Claude/ChatGPT/Grok: their CLIs, session auth; only under T3 conditions.
- Spawned teammates currently inherit the session model; per-role pinning needs per-tier DSH
  profiles or a vetted subagent model-picker (known gap).
