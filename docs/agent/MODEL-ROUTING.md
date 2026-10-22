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

## Enforcement (machine-readable)

`config/model-roster.json` is the registry: provider paths, exact model lists, tier rankings, and
gates. `scripts/model-route.py` validates and resolves:

```
python3 scripts/model-route.py list
python3 scripts/model-route.py resolve --tier T2 --path opencode
python3 scripts/model-route.py check --path opencode --model python3 scripts/model-route.py check --path claude --model "Claude Opus 5.5"   # refuses: T3 needs approval
python3 scripts/model-route.py gate --tier T3_ULTRA --approve-ultra --model "Claude Opus 5.5" --reason "..."
```

Rules enforced: model must belong to the named path; T3 Ultra refuses without `--approve-ultra`;
unknown models are refused. No secrets/network are read. This is the resolver the orchestrator should
call before spawning/pinning an agent.


## Roster maintenance

The registry is `config/model-roster.json` (`version`, `updated`, `paths`, `tiers`, `watchlist`,
`retired`). Active harness output is the authority for the OpenCode path:

```bash
# Read-only drift check; exits 1 when the active list and registry disagree.
python3 scripts/model-roster-sync.py
python3 scripts/model-roster-sync.py --json

# Preview without failing (useful in a daily pulse).
python3 scripts/model-roster-sync.py --report-only
```

A new model starts on the **watchlist** or T3 Experimental, never straight into T1/T2. It is
either promoted after an eval lane records evidence in the roster/docs, or moved to `retired`
with a date and reason. Removing an id from the active list does not erase it from `retired`, so
old lane notes and docs still resolve.

**MiniMax note (2026-10-22):** `opencode/minimax-m3.1-flash-free` appears in `opencode models`
but the provider rejects it as deprecated; it is retired. The provider's replacement,
`opencode/deepseek-v4.1-flash-free`, remains the free-model baseline.

## Principal assessments (2026-10-22)

Do not infer scores or promote a model without an eval lane. Assessments live in
`config/model-roster.json` under `assessments`; blocked models are never called.

| Model | Assessment |
| --- | --- |
| `meta/muse-spark-1.3-contributor` | **9/10 — online**, T1 |
| `opencode/nemotron-3-ultra-free` | **9/10 — online**, T2 |
| `opencode/deepseek-v4.1-flash-free` | **9/10 — best free baseline**; DSH-equivalent path, do not route DSH through OpenCode just to call itself |
| `opencode/nemotron-3.5-lightning-free` | **8/10 — evaluate**, T3 experimental |
| `opencode/mimo-v2.6-flash-free` | **7/10**, used in past |
| `opencode/longcat-2.5-preview-free` | mixed results in past |
| `meta/muse-spark-1.3` | **do not touch** |
| `meta/muse-wonder-1.0` | **do not touch** (frontier/expensive) |
| `opencode/big-pickle` | **not free — blocked** |
| `opencode/beam-free`, `opencode/glm-5.3-flash-free`, `opencode/nemotron-3.5-super-vl-free` | unknown; watch/evaluate |
| `opencode/minimax-m3.1-flash-free` | listed but direct run says deprecated; **blocked pending config refresh**, not deleted |

## Existing DEV lanes

Lane directories are evidence, not authority; never delete one when a model is
retired from the active list. Current mapping is in `config/model-roster.json`
under `lanes` (mimo, longcat, lightning, nemotron, muse, spacebunny, Claude, GPT,
Grok). A new eval lane is `lane-<model>`; the roster gets the result and the
lane stays in place.
