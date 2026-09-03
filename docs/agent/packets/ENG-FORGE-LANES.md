# ENG-FORGE-LANES — Split agent harness on existing Forge

## Goal

Route Forge work by job shape (Scout / Architect / Smith / Inspector / Assay)
without replacing the DeepSeek harness, the invoker, or the Neon Story Board.

## Scope

- Lane policy in `agent-runtime/`
- Git-side architect packets so Grok can lead without Neon
- DeepSeek remains volume-lab (Scout, Smith, Assay)
- Grok remains judgment-lab (Architect, Inspector)
- Assay planner uses TUNIT as the instrument, never a second harness

Out of scope: a Grok CLI adapter, Neon exports, Command Console UI, Codex as
a daily provider.

## Architect brief

Neon `storyboard_story` is the control-plane packet. Grok cannot see it.
Therefore every active story also has a git packet at
`docs/agent/packets/<id>.md`.

`sessionFromStory` maps Neon fields. `sessionFieldsFromGitPacket` maps the markdown.
`buildLaneEnqueue` can take either. Smith requires an architect brief from
one of those two sources. Assay requires `## Assay commands`.

Do not register `reviewer-other` or `architect-pro` on `deepseek-harness`.
That would collapse Inspector and Architect onto Smith’s lab.

Non-builder roles (`scout`, `architect`, `reviewer`, `verifier`) must not
commit or write DEV. The invoker policy follows the persisted role.

## Context refs

- `agent-runtime/lanes.ts`
- `agent-runtime/lane-policy.ts`
- `agent-runtime/story-session.ts`
- `agent-runtime/git-packet.ts`
- `agent-runtime/assay-plan.ts`
- `agent-runtime/test-mode.ts`
- `agent-runtime/enqueue-lane.ts`
- `agent-runtime/factory.ts`
- `agent-runtime/invoker.ts`
- `docs/tunit-harvest-register.md`
- `docs/agent/TEST_ISOLATION.md`

## Acceptance criteria

- Grok can architect a story from git alone
- DeepSeek can implement from the same packet
- Inspector lineage ≠ Smith lineage
- Assay refuses empty command lists and SCOPED full-regression aliases
- No second work queue
- No schema change required for this story

## Preconditions

- Lane modules are on `main`
- DeepSeek harness adapter is live for `builder-flash`

## Postconditions

- `docs/agent/ARCHITECT.md` names Grok as judgment-lab
- Packets directory exists
- `planAssay` is the only way Assay commands enter special_instructions

## Test mode

SCOPED

## Assay commands

- node --test agent-runtime/assay-plan.test.ts
- node --test agent-runtime/lane-policy.test.ts
- node --test agent-runtime/git-packet.test.ts
