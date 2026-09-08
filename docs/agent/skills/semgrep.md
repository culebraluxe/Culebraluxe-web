# Skill: semgrep

Static analysis: security, correctness, and dataflow/taint ("sonar lite"). Finds issues grep can't (cross-function taint, injection, hardcoded secrets).

## When to use
- **Assay / QA** pre-release over the changed scope:
  - Broad correctness/security: `semgrep scan --config=auto <changed-dirs>` (whole repo: `semgrep scan --config=auto .`)
  - OWASP/security focus: `semgrep scan --config=p/owasp-top-ten <changed-dirs>`
  - Dataflow/taint: `semgrep scan --config=p/security-audit <changed-dirs>`
  - Repo-specific rules should live under a `rules/` dir so scans are deterministic.

## Wiring status (honest)
Installed (Homebrew). **NOT yet integrated** into the Assay runner — run manually in QA until wired.

## Never
- Treat a clean `semgrep` as proof of security; treat hits as review findings, not auto-fixes.
