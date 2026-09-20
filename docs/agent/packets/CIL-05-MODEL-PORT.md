# CIL-05 — bind a model port (DeepSeek / OpenCode)

## Why

CIL-03 can refuse and persist. Deliberation needs a model. The vendor CLI is
not that model. We already own DeepSeek harness and an OpenCode adapter.

## Units

- **A** — `DeepSeekCouncilPort` and/or `OpenCodeCouncilPort` implementing
  `CouncilModelPort`. Prompt is panel contracts + question + evidence rules.
  Response parsed with a runtime schema into `RecordVerdictRequest`.
- **B** — Invalid model JSON is `COUNCIL_MODEL_INVALID`, session marked
  `failed`. Never cast.
- **C** — Default production port: cheapest bound harness that can return
  structured JSON. Record provider name in `raw_model`, not as a second fact.
- **D** — Token / cost goes through existing harness usage rows. Do not invent
  a council cost table.
- **E** — Hemlock: if the frame-break seat re-asks an answered question, the
  port injects "state a position in 50 words" and stops the loop. Max 2
  messages per pair.

## Acceptance

- A fixture model returning 0 dissent is rejected by `recordVerdict`.
- A fixture model returning valid shape persists one verdict.
- No import of vendor `install.sh` or slash-command runner.
- Harness adapters under `agent-runtime/` are not replaced.

## Not this story

- Not multi-provider fan-out across 18 seats.
- Not a full council.
