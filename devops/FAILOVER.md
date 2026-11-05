# Failover — production to the DR app

**Production is `culebraluxe-prod` (Ashburn, two machines). The standby is `culebraluxe-dr` (Chicago, one machine
that sleeps).** Both run the same image: `scripts/deploy-fly.sh` releases to production, waits for its smoke to
pass, and only then updates the standby. The domains point at production's dedicated IPv6; a failover is a DNS
change, not a deploy.

| | app | region | machines | image | dedicated IPv6 |
|---|---|---|---|---|---|
| Production | `culebraluxe-prod` | `iad` | 2 | from `devops/fly.toml` | `2a09:8280:1::1ea:991e:0` |
| Standby | `culebraluxe-dr` | `ord` | 1 (sleeping) | from `devops/fly.dr.toml` | `2a09:8280:1::1e4:ead1:0` |

Both apps read the **same Neon production database**; failing over does not move data. What changes is which machine
answers the domain.

## When to fail over

Production is unhealthy and not coming back quickly: both iad machines failing their `/healthz` check, an Ashburn
regional outage, or a Fly incident scoped to `iad`. A single bad release is **not** a failover — that is a rollback
(the previous complete release image, see `devops/DR-FLY.md`).

## Steps

**1. Wake the standby and prove it is healthy.** This is read-only and safe to do before deciding.

```sh
fly status --app culebraluxe-dr
curl -s https://culebraluxe-dr.fly.dev/healthz        # the first request wakes the machine (allow ~30s)
curl -s https://culebraluxe-dr.fly.dev/api/build-info # the release it carries; it should match production's
```

If `/api/build-info` reports a different `changeset` than production's last healthy release, the standby is behind:
`scripts/deploy-fly.sh --resume <that image tag>` with `FLY_APP=culebraluxe-dr` is how to bring it level *before*
switching.

**2. Keep the standby awake for the duration.** A sleeping DR machine is fine for a cold standby and wrong during a
failover — every request would pay a wake, and nothing keeps it warm:

```sh
fly machine update <machine-id> --app culebraluxe-dr --auto-stop=off
# undo afterwards:  fly machine update <machine-id> --app culebraluxe-dr --auto-stop=suspend
```

**3. Point the domains at the standby (Claude + Lisa).** In the Cloudflare zone `culebraluxe.com`, change the
`AAAA` record that carries the origin for the apex and for `www` to the standby's address:

```
from  2a09:8280:1::1ea:991e:0     (culebraluxe-prod, iad)
to    2a09:8280:1::1e4:ead1:0     (culebraluxe-dr, ord)
```

Keep the proxy (orange cloud), SSL `Full (strict)` and "Always Use HTTPS" exactly as they are — Cloudflare still
terminates TLS, and Fly still holds valid certificates for both names.

**4. Verify.**

```sh
curl -sI https://www.culebraluxe.com/ | grep -iE '^(HTTP/|server|cf-ray)'
curl -s https://www.culebraluxe.com/api/build-info     # databaseTarget must be "prod"
scripts/smoke-site.sh --base https://www.culebraluxe.com
```

The origin behind Cloudflare is the only thing that changed, so `server: cloudflare` and a `cf-ray` header are still
what you should see; `/api/build-info` naming the standby's changeset is what proves the switch.

## Fail back

Production is healthy again when both iad machines pass their check and `https://culebraluxe-prod.fly.dev/healthz`
answers 200. Then reverse step 3: the `AAAA` origin goes back to `2a09:8280:1::1ea:991e:0`, and step 2 is reverted so
the standby goes back to sleep. Verify with step 4 again.

## What does NOT need doing

- **No deploy.** The standby already carries the release that passed its smoke; failing over is a DNS change.
- **No data move.** Both apps use the production Neon branch.
- **No certificate work.** Both apps hold Fly certificates for `culebraluxe.com` and `www`, issued through the
  `_acme-challenge` records already in Cloudflare.
