# Skill: serena

Semantic code navigation — references, implementations, definitions, and precise rename/refactor across a project. Use it instead of grep when symbol identity matters.

## When to use
- **Architect / Lead / Smith** before any structural edit or refactor: confirm real references/implementations before changing a symbol.
- **Inspector** when reviewing a change's blast radius.

## Setup (per project)
`serena init` → `serena project`/`serena config` to build the index, then query via `serena tools` / `serena start-mcp-server` / `serena start-project-server`.

## Wiring status (honest)
Installed (global `~/.local/bin/serena`). **NOT yet wired** into the Forge runtime adapter as an exposed tool. Until it is, invoke through the shell/workspace if present — and never claim Serena ran when it did not.

## Never
- Grep for a symbol when `serena` can resolve implementations/references unambiguously (rename safety).

## Anchored to

Nothing in this repository yet, and that is stated rather than papered over: serena is a global tool
(`~/.local/bin/serena`) with no config, index or wrapper committed here, so this pack carries the one
standing `skill-not-anchored` warning. It clears the day the Forge runtime adapter exposes serena as a
tool (then this line points at the adapter) or the day the pack is deleted.

