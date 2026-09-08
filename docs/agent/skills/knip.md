# Skill: knip

Find unused files, unused exports, unused dependencies, and dead code.

## When to use
- **Maintenance / hygiene** stories and before declaring a refactor complete (catches orphaned exports).
- Run: `pnpm knip` (uses the in-repo `knip.json` config).

## Wiring status (honest)
Installed (npm dep `^6.34.0`, script `knip => knip`). `knip.json` is currently **untracked** — keep the config committed so maintenance runs are deterministic.

## Never
- Delete a symbol solely on a knip "unused" hit without confirming it is not referenced by code/tests outside the default scan roots.
