# Forge Batch 2 execution controls

## Configuration

| Setting | Default | Range / behavior |
| --- | --- | --- |
| `FORGE_MAX_MODEL_TURNS_PER_GENERATION` | `10` | Parsed once when the generation starts; valid values clamp to `1..=100`. The generation is the `storyboard_story_run` id. |
| `FORGE_WITHIN_STORY_CONCURRENCY` | `1` | Valid values clamp to `1..=4`. A value above `1` is honored only by the durable registry/job path when every registered harness can be forked into an isolated worktree and supports scoped interruption. |
| `FORGE_STORY_WORKERS` | `4` | Resident story-worker concurrency, separately clamped to `1..=8`. This controls different stories, not lanes inside one story. |
| `FORGE_TURN_TIMEOUT_MINUTES` | `120` | Wall-clock ceiling for one model turn. `0`, `off`, or `none` explicitly disables the ceiling. |

Malformed or empty cap/concurrency values retain their defaults. Increasing the lane cap does not increase the generation's model-attempt budget. OpenCode can provide isolated forked sessions; a harness that cannot prove fork plus scoped interruption is refused before parallel job claims. Maestro remains serial until it can provide those capabilities.

## What the counters mean

- **Model-attempt allowance** is a durable, generation-scoped count of authorized model launches. Reservations are written before launch and stay spent when launch status is uncertain. A refused reservation does not consume another allowance.
- **Durable job attempts** count leased executions and retries. They are separate from model attempts: one job may use multiple corrective model attempts, while a retry may be refused by the generation cap.
- **Model usage** records provider-reported tokens and dollars on the Story Run. It is not used as the attempt counter.

The durable budget row fixes the cap for the generation. A resumed process with a different environment value observes the stored cap and usage; it cannot reset either. Legacy in-flight generations whose usage cannot be reconstructed are saturated by the migration backfill so the new binary fails closed.

## Reading execution diagnostics

`forge-model-attempt` entries identify generation, task, model attempt, reservation status, and used/cap. `forge-wave` entries report the effective concurrency cap, wave number, scheduling conflicts, and lane count. `forge-wave-lane` start/join records include story, instance, task, job, lease attempt, and active-lane count. `forge-interrupt` records include the same execution identity and whether the scoped interruption reached a live process.

`forge-execution-stop` distinguishes a model-attempt cap refusal and an interrupted turn from a role execution error. Cap refusal and interruption holds say that no role verdict was accepted; they do not claim the role itself failed. `forge-wave-refusal` names both tasks and the overlapping or unknown write path. These records keep the run/generation identity separate from the job lease attempt.

## Enabling within-story concurrency

Keep the default at `1` unless the worktree and harness prerequisites are verified. A higher setting is clamped at `4`; incompatible services are refused before any lane in the planned parallel batch is claimed. Unknown or conflicting write surfaces are scheduled serially. Each lane receives its own lease, worktree, runner, and scoped interrupt. Candidate commits are integrated sequentially in stable task-id order, and a merge conflict places the story on hold with the isolated candidate worktree retained.

Changing a setting does not bypass Workflow gates, release policy, or human decisions. Batch 2 adds no production cutover; deploy only after the batch-wide verification and the separately authorized rollout step.
