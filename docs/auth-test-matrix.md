# AUTH Test Matrix

Deterministic expectations for the authentication/authorization runtime. No
persistent production writes during tests. Run the pure secret verification now
via `node scripts/verify-break-glass-secret.mjs`; the remainder is exercised
once Auth.js + provider are installed.

## Normal login

| ID | Scenario | Expected |
|----|----------|----------|
| A | unauthenticated | rejected (`UnauthenticatedError`) |
| B | authenticated but unknown provider subject | `unmapped` — no account creation, no email match |
| C | mapped inactive app_user | `inactive` — rejected |
| D | mapped active agent | authorities = {portal.read, crm.write, listing.write, deal.read, deal.write} |
| E | mapped viewer | authorities = {portal.read, deal.read} |
| F | external client | authenticated; does NOT gain portal.read |
| G | owner | exactly the explicit owner authorities (no wildcard) |

## Break-glass root

| ID | Scenario | Expected |
|----|----------|----------|
| H | disabled break-glass | fails (`disabled`) |
| I | wrong secret | generic failure, no enumeration |
| J | configured user nonexistent | fails (`unavailable`) |
| K | configured user inactive | fails (`unavailable`) |
| L | configured user not internal | fails (`not-owner`) |
| M | configured user lacks owner role | fails (`not-owner`) |
| N | valid root | resolves the same canonical ActingUser model |
| O | root absent an authority | that authority is still denied (no wildcard) |

## Identity

| ID | Scenario | Expected |
|----|----------|----------|
| P | provider email changes | subject mapping unaffected (subject is key) |
| Q | same provider subject mapped twice | structurally impossible (UNIQUE) |
| R | one app_user, multiple identities | allowed (no unique app_user_id) |

## Secret primitive (runnable now)

`node scripts/verify-break-glass-secret.mjs` verifies hash/verify symmetry,
wrong-secret rejection, malformed-hash rejection, and deterministic hashing per
salt — mirroring `lib/auth/break-glass-secret.ts`.
