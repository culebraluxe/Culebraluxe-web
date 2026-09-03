# ENG-FORGE-LANES — Split agent harness on existing Forge

## Goal

Route Forge work by job shape (Scout / Architect / Smith / Inspector / Assay)
without replacing the DeepSeek harness, the invoker, or the Neon Story Board.

## Scope

- Lane policy in `agent-runtime/`
- Git-side architect packets so Grok can lead without Neon
- DeepSeek remains volume-lab (Scout, Smith, Assay)
- Grok remains judgment-lab (Architect, Inspector)

Out of scope: a Grok CLI adapter, Neon exports, Command Console UI, Codex as
a daily provider.

## Architect brief

Neon `storyboard_story` is the control-plane packet. Grok cannot see it.
Therefore every active story also has a git packet at
`docs/agent/packets/<id>.md`.

`sessionFromStory` maps Neon fields. `sessionFieldsFromGitPacket` maps the markdown.
`buildLaneEnqueue` can take either. Smith requires an architect brief from
one of those two sources.

Do not register `reviewer-other` or `architect-pro` on `deepseek-harness`.
That would collapse Inspector and Architect onto Smith’s lab.

Next bounded slice after this packet: Command Console calls `buildLaneEnqueue`
with the story row plus, if present, the git packet.

## Context refs

- `agent-runtime/lanes.ts`
- `agent-runtime/lane-policy.ts`
- `agent-runtime/story-session.ts`
- `agent-runtime/git-packet.ts`
- `agent-runtime/enqueue-lane.ts`
- `agent-runtime/factory.ts`
- `agent-runtime/invoker.ts`
- `db/storyboard.ts`
- `db/agent-work.ts`
- `docs/agent/STORY_EXECUTION_CONTRACT.md`
- `docs/agent/ARCHITECT.md`
- `docs/FORGE-LANES.md`
- `AGENTS.md`

## Acceptance criteria

- Grok can architect a story from git alone
- DeepSeek can implement from the same packet
- Inspector lineage ≠ Smith lineage
- No second work queue
- No schema change required for this story

## Preconditions

- Lane modules are on `main`
- DeepSeek harness adapter is live for `builder-flash`

## Postconditions

- `docs/agent/ARCHITECT.md` names Grok as judgment-lab
- Packets directory exists and this story is the first packet
- `sessionFieldsFromGitPacket` parses the headings above
