# Production-readiness plan + work orders (Forge + ForgeVCS)

Status: active 2026-10-18. Owner: Chief of Staff. Backup standard: **local zip on the Mac +
Neon-hosted backups** is accepted; no third-party offsite required.

## Definition of "production ready" (both systems)

All P0 work orders closed with tests/evidence, one soak/proof run, a written readiness register,
and a named sign-off. Green unit tests alone do not count.

## ForgeVCS work orders (source: forgevcs/docs/PRODUCTION-PACKET.md)

| WO | Scope | Acceptance | Status |
|---|---|---|---|
| FVCS-01 | Repository-scoped ACL/isolation | agent-a cannot read/write repo-b; owner/ACL on init; tests; existing path/ref rules intact | DONE (ffbe5dd; 52 service tests; residual: live DB round-trip in soak, legacy repos backfill) |
| FVCS-02 | Path-prefix write scopes on push | write on `lane/a/**` allows only those paths; outside refused; ref scopes intact | DONE (ffbe5dd; path-prefix push tests; acl.toml must be narrowed per lane for enforcement) |
| FVCS-03 | Rate limits + per-agent quotas | burst → 429 + Retry-After, no 5xx cascade, quotas observable | OPEN (next) |
| FVCS-04 | 10-agent soak | 10 agents, independent repos, concurrent pushes, rename/conflict; 0 lost refs; latency/error report | OPEN (blocked by 01–03) |
| FVCS-05 | Backup: Mac zip + restore drill + alert | scheduled zip, restore drill passes, failure alert, retention; psql dependency documented | OPEN (started) |
| FVCS-06 | Conflict resolution workflow | paused-conflict state + resolve/continue; tests | OPEN |
| FVCS-07 | Staging + CI | second environment + checked pipeline (gate/smoke/deploy) | OPEN |
| FVCS-08 | SLO/alerting + retention | readiness/error/latency/DB alerts; auth-decision retention | OPEN |

## Forge work orders (delivery + Harness Evolution + CTO layer)

| WO | Scope | Acceptance | Status |
|---|---|---|---|
| FORGE-01 | Controlled DSH cohort + decision | pre-registered interleaved cohort passes; cost/usage coverage reported; adoption decision recorded (adoption stays OFF unless rule passes) | IN FLIGHT |
| FORGE-02 | DSH version pin + upgrade/plugin-vetting runbook | pinned version, upgrade checklist, plugin allowlist/vetting, rollback | OPEN |
| FORGE-03 | Adoption gate + supervision | effective-GREEN policy executed once under supervision; activation/rollback proven; auto-adoption still OFF | OPEN |
| FORGE-04 | Backup/monitoring for Forge state | DB backups verified (Neon), Mac zip for docs/evidence, digest alerts | OPEN |
| FORGE-05 | Operator surface + SLO for harness | CLI/status/digest covered; failure alerts; runbook | OPEN |
| FORGE-06 | CTO office scheduler + offsite zip | daily pulse (start-session alerts) + scheduled zip; cold-start acceptance | OPEN |
| FORGE-07 | Readiness register + sign-off | written register for Forge delivery, harness, CTO staff; principal sign-off | OPEN |

## Sequence

1. FVCS-01+02 (isolation) → FVCS-03 (limits) → FVCS-04 (soak) → FVCS-05 (backup proof) → register.
2. FORGE-01 result → FORGE-03 gate → FORGE-02/04/05 → CTO FORGE-06 → register.
3. No "production ready" label until the register is signed.

## Notes

- Mac zip is the accepted offsite copy; Neon is the durable primary with its own backups.
- Old ForgeVCS pilot repositories need the DevOps cleanup pass before the soak.
- Every WO lands through its repo's gate; no work order closes on prose alone.
