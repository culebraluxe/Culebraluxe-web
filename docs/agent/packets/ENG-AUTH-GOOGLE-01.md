# ENG-AUTH-GOOGLE-01 — the Google sign-in return path, pinned by tests

Lane: builder. Small, bounded, one file. This is the smallest slice of the test-safety sweep
(`docs/agent/TEST-SAFETY-SWEEP-2026-09-29.md`, item 5): the sign-in route is a security surface with
two tests on it.

## Goal (one line)

`web/src/api/google_auth.rs` gains the tests its security-relevant surfaces need — the return-address
filter, the state-cookie codec, and the origin/redirect derivation — and the one real defect those tests
expose is fixed.

## Scope

- **In:** unit tests inside `web/src/api/google_auth.rs` (`#[cfg(test)] mod tests`), plus the
  smallest fix in that same file for a return address that leaves the site.
- **Out:** no route change, no new dependency, no change to `web/src/api/ui_auth.rs`, no DB, no
  domain, no migration, no deploy, no other file. `safe_next` keeps its job (a path on this site or the
  default); this story does not redesign where `state` lives and does not add origin binding.

## Why (the gap, measured)

The file serves `signin/google`, `callback/google` and `signout` (`web/src/api/google_auth.rs:28-33`).
Everything it does between "Google answered" and "the session cookie is set" is trusted by construction,
and two tests (lines 250-269) cover one function and one round trip.

## Context refs (live paths — verified 2026-09-29)

- `web/src/api/google_auth.rs:74-78` — `safe_next`: a return address is used when it starts with
  `/` and not `//`. The filter does not consider `\`, which every browser reads as `/`: `/\evil.example`
  is `//evil.example` when the Location header is followed. That is an off-site redirect after a
  successful sign-in, reachable with no credentials.
- `web/src/api/google_auth.rs:36-45` — `encode`; `web/src/api/google_auth.rs:226-242` —
  `percent_decode`. The pair is the state/next cookie codec. One happy-path round trip is tested.
- `web/src/api/google_auth.rs:48-67` — `origin`, `web/src/api/google_auth.rs:69-71` —
  `redirect_uri`. The redirect URI must equal what is registered in Google Cloud, so its derivation
  deserves a pinned test rather than a reading.
- `web/src/api/google_auth.rs:80-87` — `set_cookie`: `HttpOnly`, `SameSite=Lax`, `Secure` only in
  a production target, `Max-Age` from the caller.
- `web/src/api/google_auth.rs:215-218` — the order in the callback: `percent_decode` **then**
  `safe_next`. The order is what makes the filter see a decoded value; a test asserts it stays that way.
- `.github/workflows/gates.yml:336` — CI runs `cargo test --workspace --all-targets`, so these tests are
  actually executed in the gate, not only locally.

## Acceptance criteria

1. `cargo test -p web google_auth` passes with the new cases, and fails if the fix is reverted.
2. `safe_next` refuses a return address beginning `/\` (`/\evil.example`, and `/\` alone) as it already
   refuses `//evil.example` and `https://evil.example`; the default stays `/portal/dashboard`.
3. `percent_decode(&encode(path)) == path` for: a space, `&`, `%`, a non-ASCII path, and a trailing
   incomplete escape (`/a%`); a `%zz` pair is left alone and never panics. `String::from_utf8_lossy`
   keeping a malformed byte lossy is pinned as intended, not as a bug report.
4. `origin`/`redirect_uri` are pinned: `x-forwarded-host` wins over `host`; a `localhost`/`127.0.0.1`
   host defaults to `http`, anything else to `https`; `x-forwarded-proto` wins over both;
   `redirect_uri` is `<origin>/api/auth/callback/google`.
5. `set_cookie` is pinned by relationship, not by a hard-coded flag: `HttpOnly` and `SameSite=Lax` are
   always present, `Max-Age` is the given value, and `Secure` is present **iff** the process is a
   production target — the test must pass under both, so it must not assume the ambient environment.
6. The diff is tests plus the one fix in `safe_next`. No other production behaviour changes.

## Skills

ripwire, semgrep

## Loop

intent: grow
loop: 1/3

## Test mode

SCOPED

## Assay commands

```sh
cargo test --manifest-path Cargo.toml -p web google_auth
cargo check --manifest-path Cargo.toml -p web --all-targets
```

## Risks (what could invalidate this)

1. **`Safe` and `Secure` are environment reads.** `set_cookie` consults `production()`; a test that
   asserts `Secure` unconditionally is a test that passes on one machine and fails on another. Assert
   the relationship (criterion 5).
2. **`x-forwarded-host` is attacker-influenceable in general.** Vercel sets it, and the redirect URI
   must match Google's registration, so this story PINS the existing precedence and does not redesign
   it. Changing it would break the registered callback — a separate story if it is ever wanted.
3. **Backslash rejection could refuse a legitimate deep link.** No route in this application contains a
   `\`, and a `\`-leading address is off-site in every browser. If a real link needs it, that is a new
   fact about the app and the fix is wrong — say so rather than widening the filter.
4. **The fix is in production code.** It is one comparison in one function; the Assay commands above are
   the fence. Do not "while here" refactor `origin` or `credentials`.

## Release obligations

- migrationRequired: false
- derivedRefreshRequired: false
- deploymentRequired: true (the fix ships in the Rust server; no schema, no env, no data)
