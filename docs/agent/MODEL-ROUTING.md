# Model routing — harness-aware roster

Status: active 2026-10-19. **The harness is part of the model's identity.** The same nominal model can
be reached through different harnesses/network paths; do not assume equivalence.
Credentials are session-local; routing never stores or prints secrets.

## Execution paths (pick the path first, then the model)

| Path | Used by | Model namespace | Rule |
| --- | --- | --- | --- |
| **DSH native** | this coordinator session, DSH-spawned agents | DSH-native DeepSeek (e.g. `deepseek-official/deepseek-v4-flash`, profile default) | use the native model; don't pass OpenCode ids into DSH |
| **OpenCode** | OpenCode harness, **Maestro** | exactly `opencode models` ids (list below) | only ids from the list; Forge probe dogfood = `*-free` only |
| **Claude CLI** | Claude models | Claude's own CLI + session auth | not in `opencode models`; T3 gate |
| **ChatGPT/Codex CLI** | GPT models | `codex` CLI + session auth | not in `opencode models`; T3 gate |
| **Grok** | Grok models | its own CLI (if configured) | not in `opencode models`; T3 gate |

**Rule:** if running in DSH → choose the DSH-native model. If running in Maestro/OpenCode → choose
from the OpenCode list below. Never substitute one harness's id for another's.

## Roster

### T1 — Daily Drivers
1. `opencode/deepseek-v4.1-flash-free` (OpenCode) — DSH equivalent: native DeepSeek daily model
2. `meta/muse-spark-1.3-contributor` (OpenCode)
3. `opencode/nemotron-3.5-super-vl-free` (OpenCode)
4. Claude Haiku 5.5 (Claude CLI)
5. `opencode/space-bunny-free` (OpenCode)
6. `opencode/mimo-v2.6-flash-free` (OpenCode)
7. GPT 6.0 Luna High (codex CLI)
8. GPT-6.1 Sol Medium (codex CLI)

### T2 — Judgement
Baseline candidate: `opencode/deepseek-v4.1-flash-free`
1. `opencode/nemotron-3-ultra-free`
2. `deepseek/deepseek-v4-pro`
3. Claude Sonnet 5.5 (Claude CLI)
4. GPT-6.1 Sol High (codex CLI)

### T3 — Ultra (conditional, expensive)
- `meta/muse-wonder-1.0`
- Claude Fable 5.1 (Claude CLI)
- Claude Opus 5.5 (Claude CLI)
- Grok
- GPT-6.0 Astra Light (codex CLI)
- GPT-6.0 Astra Medium (codex CLI)

### T3 — Experimental (free; experiments only, never authority)
- `opencode/glm-5.3-flash-free`
- `opencode/longcat-2.5-preview-free`
- `opencode/nemotron-3.5-lightning-free`
- `opencode/beam-free`
- `opencode/big-pickle`

## Exact OpenCode id list (authoritative for that path)

```
deepseek/deepseek-flash
deepseek/deepseek-v4-flash
deepseek/deepseek-v4-flash-vision-exp
deepseek/deepseek-v4-pro
meta/muse-spark-1.1
meta/muse-spark-1.2
meta/muse-spark-1.2-contributor
meta/muse-spark-1.3
meta/muse-spark-1.3-contributor
meta/muse-wonder-1.0
opencode/beam-free
opencode/big-pickle
opencode/deepseek-v4.1-flash-free
opencode/glm-5.3-flash-free
opencode/longcat-2.5-preview-free
opencode/mimo-v2.6-flash-free
opencode/nemotron-3-ultra-free
opencode/nemotron-3.5-lightning-free
opencode/nemotron-3.5-super-vl-free
opencode/space-bunny-free
```

If a model is not in this list and not a DSH-native model, it is **not available on this path**.

## Selection algorithm

1. Identify path: DSH-native vs OpenCode/Maestro vs Claude/Codex/Grok CLI.
2. Determine tier from role map.
3. Choose the highest-ranked model **available on that path**; fall back down the ranked list.
4. T1/T2: orchestrator chooses without asking.
5. **T3 Ultra:** requires principal request OR irreversible/high-stakes/Council; T0–T2 justified as
   insufficient; call logged (reason/model/expected/stop); max 3/session; never routine or to bypass
   authority/security.
6. **T3 Experimental:** experiments only, never authority.
7. Budget: over N=120 minutes flagged, not blocked (`RUN-BUDGET.md`).

## Role → tier

| Role | Tier |
| --- | --- |
| Chief of Staff | T2, fallback T1 |
| Strategist | T2 |
| Engineer | T1 (T2 architecture) |
| Operator | T1 |
| Researcher | T1/T2 |
| Comms | T1 |
| Finance/Commercial | T2 |
| Devil's Advocate | T2 |
| Forge Scout | T1 |
| Forge Architect | T2 |
| Forge Lead | T2 fallback T1 |
| Forge Smith | T1 |
| Forge QA/Inspector | T2 fallback T1 |
| Forge Assay | T2 |
| Forge DevOps | T1 |

## Mechanics

- **DSH:** native model via profile/`DSH_MODEL`; spawned teammates inherit the session model until
  per-tier profiles/plugin exist.
- **OpenCode:** `opencode run --model <exact id>`; Forge probes only `*-free`.
- **Maestro:** routes through OpenCode, so use the OpenCode list.
- **Claude/Codex/Grok:** own CLIs, T3 gate.
