# Council port pack — copy into CulebraLuxe-web

Generated 2026-09-19. **[proposed]**, not on HEAD.

This pack is meant to be copied onto the root of `culebraluxe/Culebraluxe-web`
and executed as the CIL story stack. It does **not** replace Forge lanes,
DeepSeek, OpenCode, or Pi/Warp adapters.

## What you got

```
docs/COUNCIL-PORT.md                 operator + architecture contract
docs/agent/skills/council.md
docs/agent/packets/CIL-00-STACK.md   ordered backlog
docs/agent/packets/CIL-01…13         stories
db/migrations/NNN_council.sql        rename to next free number
db/council.ts                        only writer (adapt neon helper on HEAD)
services/council/                    domain service
services/composition.patch.md        CIL-04 notes
scripts/council-vendor-sync.sh
vendor/council-pin.json
vendor/README.md
tests/council-refuse.test.ts
```

## Operator sequence

1. Stop the scheduler. `pnpm forge:doctor` → 0 open engine tasks.
2. Read `docs/COUNCIL-PORT.md` §0 laws.
3. Copy this tree onto the repo. Do not copy into a worktree farm.
4. Run CIL-01 (submodule + pin SHA).
5. Mark CIL-01 Complete only when `council-vendor-sync.sh verify` is green.
6. Ready CIL-02 only after CIL-01 is Complete.

## Hard no

- `./vendor/council-of-high-intelligence/install.sh`
- `council` lane in `agent-runtime/lanes.ts`
- FORGE_SDLC XML node named council
- Two inflight sessions for one subject
- Persona names on the public site
