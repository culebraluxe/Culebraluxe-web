# SOP — DSH version pin, upgrade, rollback and plugin vetting (FORGE-02)

**Status:** active 2026-10-19. **Owner:** Lead/Architect (code pin) with the application manager for
any paid model turn. **Trigger:** DSH security advisory, a required vendor feature, a probe failure
that names the version, or the quarterly backend review.

## Why this exists

Harness Evolution contrasts behaviour between two harness revisions. If the vendor harness changes
under an experiment, the contrast attributes vendor behaviour to the candidate revision. The adapter
therefore runs **one pinned DSH version** and fails closed on a mismatch before any claim or model
turn is spent.

- Pin source: `DSH_EXPECTED_VERSION`; when unset, the adapter default
  `DEFAULT_DSH_VERSION = "0.1.1-rc.2"` (`forge/src/engine/dsh/preflight.rs`).
- Enforcement: `preflight_with_version` runs `dsh --version`, parses the version, and refuses on a
  mismatch or an unparseable answer. It runs before a Forge claim (`dsh/harness.rs`,
  `dsh/advisory.rs`) and before a model-turn probe is authorized
  (`ModelTurnBackend::ensure_available`).
- Evidence: the verified version is written to the model-turn receipt (`vendor_version`) and to probe
  evidence as `model-turn:vendor-version:<version>`; probe/contrast rows keep that ref.

## Preconditions

- Written approval for any paid model turn in the test (same rule as any other paid run).
- A scratch environment: scratch `DSH_HOME`, `DSH_BIN` pointing at the candidate package, and a
  non-production `--environment` (`sandbox`/`development`/`ci`). Never point an upgrade test at
  production.
- Current pin and rollback package recorded in the change note.

## Upgrade procedure

1. **Record the candidate.** Vendor package and version (`@deepseek-ai/dsh@<version>`), release notes,
   compatibility notes, and the old version it replaces.
2. **Local contract checks (no spend).**
   - `DSH_EXPECTED_VERSION=<new> dsh --version` reports exactly `<new>`.
   - `dsh --profile headless --help` exits 0 (the profile boots).
   - `forge doctor`/DSH preflight path accepts the candidate only with the new pin; with the old pin
     it must fail closed (that failure is the proof the pin is enforced).
3. **Scratch-profile smoke (one small turn, approved).** Run a single headless turn in the scratch
   home and confirm: exit 0, final text present, session discovered, tokens measured (or explicitly
   `unmeasured` — never 0), and no unexpected files/network access. Record the receipt.
4. **Harness-evolution non-production probe.** With the candidate pinned, run one probe on a
   historical DEV story using the command sandbox (no spend) and, if approved, one model-turn probe.
   Record experiment/probe/contrast ids, the `model-turn:vendor-version:` refs, contrast dimensions,
   tokens/cost coverage, and any regression. A version-only change with identical behaviour is a
   pass; any behavioural delta is a harness change, not a free upgrade.
5. **Plugin vetting.** Permitted plugin surface is the shipped minimum: the `agent-default-model`
   `--patch` overlay (model pinning) only. Adding any plugin needs: source review, a written
   allowlist entry, no new network/file/exec authority, an offline test, and the application
   manager's approval. No plugin may be enabled by a profile that grants or widens permissions.
6. **Land the pin.** Update `DEFAULT_DSH_VERSION` and set `DSH_EXPECTED_VERSION` in the deployment
   environment in the same change; run `cargo test -p forge --lib dsh::preflight`,
   `cargo test -p forge --lib model_turn_sandbox`, `cargo check -p forge --all-targets`, and
   `pnpm forge:harness`. Record the version and evidence ids in the change note.
7. **Observe.** Keep the previous version installed for one week. Watch probe receipts for
   `usage_source`, cost coverage and preflight refusals; treat a refused preflight as the control
   working, not an outage to bypass.

## Rollback

1. Stop new probes/claims for the affected backend (`FORGE_HARNESS` unchanged; no queue drain needed —
   preflight refuses before a claim).
2. Set `DSH_EXPECTED_VERSION` back to the previous pin (or unset it to restore the adapter default) and
   repoint `DSH_BIN`/the installed package to the previous version.
3. Verify `dsh --version` matches and `cargo test -p forge --lib dsh::preflight` is green; run one
   no-spend command probe. Paid rollback verification needs the same approval as the upgrade smoke.
4. Record the rollback reason, the version pair, and the evidence ids of the refused runs.

## Evidence to record

- Old and new version, `DSH_EXPECTED_VERSION`, package source, date, approver.
- Preflight output (version accepted/refused), the mismatch refusal seen with the old pin.
- Scratch smoke receipt (exit, duration, tokens, cost or `unmeasured`).
- Probe experiment/candidate/probe/contrast ids, `model-turn:vendor-version:` refs, contrast
  summary, token/cost coverage, and any open finding.

## Do not

- Do not bypass the pin by setting `DSH_EXPECTED_VERSION` to the vendor's current value without the
  tests above — that turns the control into a comment.
- Do not test an upgrade against production, and do not run a paid turn without the same approval
  that governs any other paid run.
- Do not add a plugin, widen a permission, or change `DSH_PROFILE` under the cover of a version bump.
