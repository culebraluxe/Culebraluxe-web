-- ENG-AUTH-GOOGLE-01 — the Google sign-in return path, pinned by tests.
--
-- The human packet is docs/agent/packets/ENG-AUTH-GOOGLE-01.md. THIS row is what the Rust engine reads:
-- rust/forge/src/engine/packet.rs:19-60 loads goal, architect_brief, acceptance_criteria and
-- assay_commands from storyboard_story — the markdown file is for reviewers, the row is for the engine.
-- A row whose columns are empty starts a run with no brief.
--
-- Idempotent on id: re-running refreshes the brief without re-dispatching, because the Ready trigger
-- fires on insert or on a *change* of status (db/migrations/025_agent_work_queue.sql:104).
--
--   ./rust/target/debug/cli db-tool apply db/loads/story_eng_auth_google_01.sql prod      (from the repo root)

begin;

insert into storyboard_story
    (id, workstream, title, priority, status, goal, scope, acceptance_criteria,
     architect_brief, assay_commands, dependencies)
values (
    'ENG-AUTH-GOOGLE-01', 'HARDEN', 'Google sign-in return path, pinned by tests', 'High', 'Ready',

    $goal$rust/server/src/api/google_auth.rs gains the tests its security-relevant surfaces need — the return-address filter, the state-cookie codec, the origin/redirect derivation — and the one real defect those tests expose is fixed.$goal$,

    $scope$In: unit tests inside rust/server/src/api/google_auth.rs (#[cfg(test)] mod tests), plus the smallest fix in that same file for a return address that leaves the site. Out: no route change, no new dependency, no change to rust/server/src/api/ui_auth.rs, no DB, no domain, no migration, no deploy, no other file. safe_next keeps its job (a path on this site, or the default); this story does not redesign where state lives and does not add origin binding.$scope$,

    $ac$1. cargo test -p server google_auth passes with the new cases, and fails if the fix is reverted.
2. safe_next refuses a return address beginning /\ (/\evil.example, and /\ alone) as it already refuses //evil.example and https://evil.example; the default stays /portal/dashboard.
3. percent_decode(&encode(path)) == path for: a space, &, %, a non-ASCII path, and a trailing incomplete escape (/a%); a %zz pair is left alone and never panics. String::from_utf8_lossy keeping a malformed byte lossy is pinned as intended, not as a bug report.
4. origin/redirect_uri are pinned: x-forwarded-host wins over host; a localhost/127.0.0.1 host defaults to http, anything else to https; x-forwarded-proto wins over both; redirect_uri is <origin>/api/auth/callback/google.
5. set_cookie is pinned by relationship, not by a hard-coded flag: HttpOnly and SameSite=Lax are always present, Max-Age is the given value, and Secure is present iff the process is a production target — the test must pass under both, so it must not assume the ambient environment.
6. The diff is tests plus the one fix in safe_next. No other production behaviour changes.$ac$,

    $brief$WHY (measured 2026-09-29, packet docs/agent/packets/ENG-AUTH-GOOGLE-01.md).

The file serves signin/google, callback/google and signout (rust/server/src/api/google_auth.rs:28-33). Everything it does between "Google answered" and "the session cookie is set" is trusted by construction, and two tests (lines 250-269) cover one function and one round trip.

THE DEFECT. rust/server/src/api/google_auth.rs:74-78 — safe_next uses a return address when it starts with / and not //. The filter does not consider \, which every browser reads as /: /\evil.example is //evil.example when the Location header is followed. That is an off-site redirect after a successful sign-in, reachable with no credentials. The order in the callback (lines 215-218) is percent_decode THEN safe_next — that order is what makes the filter see a decoded value, and a test must keep it that way.

CONTEXT REFS (live paths). 36-45 encode; 226-242 percent_decode — the state/next cookie codec, one happy-path round trip tested. 48-67 origin; 69-71 redirect_uri — the redirect URI must equal what is registered in Google Cloud, so its derivation deserves a pinned test rather than a reading. 80-87 set_cookie: HttpOnly, SameSite=Lax, Secure only in a production target, Max-Age from the caller. .github/workflows/gates.yml:336 — CI runs cargo test --workspace --all-targets, so these tests are executed in the gate, not only locally.

RISKS. (1) Safe and Secure are environment reads — set_cookie consults production(); a test asserting Secure unconditionally passes on one machine and fails on another, so assert the relationship. (2) x-forwarded-host is attacker-influenceable in general; Vercel sets it and the redirect URI must match Google's registration, so this story PINS the existing precedence and does not redesign it. (3) Backslash rejection could refuse a legitimate deep link — no route here contains a \, and a \-leading address is off-site in every browser; if a real link needs it, say so rather than widening the filter. (4) The fix is production code: one comparison in one function. The Assay commands are the fence; do not "while here" refactor origin or credentials.$brief$,

    $assay$cargo test --manifest-path rust/Cargo.toml -p server google_auth
cargo check --manifest-path rust/Cargo.toml -p server --all-targets$assay$,

    'None.'
)
on conflict (id) do update set
    workstream          = excluded.workstream,
    title               = excluded.title,
    priority            = excluded.priority,
    goal                = excluded.goal,
    scope               = excluded.scope,
    acceptance_criteria = excluded.acceptance_criteria,
    architect_brief     = excluded.architect_brief,
    assay_commands      = excluded.assay_commands,
    updated_at          = now();

commit;
