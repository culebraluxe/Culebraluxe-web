# Architect

Lead architect for Forge judgment-lab is Grok.

Grok does not read Neon. DeepSeek does, inside the worktree. The side path
that makes Grok useful is **git**.

## Split

| Lane | Who | Reads | Writes |
|---|---|---|---|
| Scout | DeepSeek | repo worktree | `docs/agent/packets/<id>.md` context refs |
| Architect | Grok | git packet + repo | `architect brief` section of that packet |
| Smith | DeepSeek | packet + worktree | code on `agent/<story>/<run>` |
| Inspector | Grok | packet + diff | review notes in packet or PR |
| Assay | DeepSeek | packet acceptance + tests | run evidence |

Neon `storyboard_story` stays the control plane. Git packets are the
architect-readable projection. When they drift, git is what Grok trusts;
DeepSeek can copy a brief back to Neon.

## Rules

- Architect does not tool the repo. Scout gathers. Grok decomposes.
- A human brief on the packet already satisfies Smith. Do not make Grok
  rewrite a brief that is already there.
- Inspector is Grok, never DeepSeek reviewing DeepSeek.
- Do not put secrets, connection strings, or raw Neon rows in packets.

See `docs/agent/packets/README.md` and `docs/FORGE-LANES.md`.
