# workflow_app

`workflow_app` is the CulebraLuxe-specific integration / model layer.

- `workflow_engine` remains a generic workflow framework/runtime and must never
  depend on CulebraLuxe domain code.
- `workflow_app` may depend on both `workflow_engine` and CulebraLuxe
  application contracts (`lib/workflow`, application services, Portal helpers).
- Portal/UI may depend on `workflow_app`, not on `workflow_engine` directly.

No brokerage workflow implementation is added yet. This is the placeholder for
future CulebraLuxe workflow definitions, application adapters, fact
projections, and the workflow-to-CulebraLuxe command bridge.

## Engine V1 hardening seam

The engine now exposes a generic application integration seam (see
[`workflow_engine/lib/workflow/types.ts`](../workflow_engine/lib/workflow/types.ts)
— `ApplicationPort`, `ApplicationCommandRequest`, `ApplicationCommandResult`,
`ApplicationFacts`) plus engine-side runtime contracts:

- terminal process outcomes: `completed | cancelled | failed | conflict`
- token/branch outcomes: `completed | cancelled | failed | skipped`
- required/optional fork branches and AND-join with optional-branch skipping
- definition-level `timer` nodes bound to the existing SKIP-LOCKED job queue
- definition-level `command` nodes with a stable, deterministic `commandId`

[`engine-bridge.ts`](engine-bridge.ts) holds the pure type/transform seam between
the engine contract and the application contract in
[`lib/workflow/contracts.ts`](../lib/workflow/contracts.ts). A future
CulebraLuxe adapter implements `ApplicationPort` by routing through
[`lib/workflow/adapter.ts`](../lib/workflow/adapter.ts) — it is not built here
and must not be until CRM-14 brokerage policy is designed.
