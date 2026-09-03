# Forge V3 candidates (not built)

V2/V2.1 stay the A1 factory: packet → hydrate → Smith → Assay.

V3 is a *session host* for B2 debug fan-out and maybe A2 parallel cognition.
It must consume packets, skills, and worktrees from this repo. It must not
replace `agent_work_item`.

## Warp

Pros: DeepSeek V4 already in Warp Agent; mixed models per task; cheap Devs +
expensive Lead/QA; you already live in a terminal.
Cons: Swarm lives in Warp, not in the Story Board. Easy to skip Ready.
Use if: you want Dev/QA *windows* that still dump findings into `docs/agent/packets/`.

## OpenClaw

Pros: explicit sub-agents, worktrees, swarm primitives closer to B2.
Cons: second runtime next to DeepSeek harness; more moving parts.
Use if: you want a manager process that fans out investigations and writes one packet.

## Rule for either pick

Lead/QA may be GPT/Claude in Warp or OpenClaw.
Smith that is allowed to commit still goes through Forge A1 (DeepSeek + Assay)
unless you later promote a second builder profile on purpose.
