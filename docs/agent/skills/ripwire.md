# Skill: ripwire

Repo-intelligence / ranked-symbol tool — "the ripgrep of AI context". Deterministic XML map to stdout; zero runtime deps.

## When to use
- **Scout** — map a codebase cold: `ripwire <dir> [--top-k=N] [--max-tokens=N]`
- **Architect / Lead** — reuse + blast radius: `ripwire <dir> --for="<task>"` (ranked signatures + metrics), or `--pack-signatures`
- **Smith** — pre-edit blast radius: `ripwire <dir> --callers=<Sym>`, `--callees=<Sym>`, `--impact=<Sym>` (counts are FLOORS; use `file:name` to disambiguate)
- **QA / Assay** — regression/test surface: `ripwire <dir> --test-gate`, `--quality-delta`
- Wire into an editor harness: `ripwire wrap <agent>`

## Forge wiring (partial — do not over-claim)
`workflow_app/forge/forge-ripwire-surface.ts` turns a ripwire pack (`p=…` / `ccx=…` / `rel="caller"`) into a **surface multiplier** on the estimator's prior. If a ripwire pack is already attached to the packet, the estimator applies it — do NOT hand-re-run ripwire to "fix" an estimate.

## Never
- Treat counts as totals (they are floors).
- Paste raw full-file dumps into context when `--callers` / `--impact` / `--for` already gives the slice.
