# Dependency Triage Ledger

Every advisory that `pnpm scan:deps` (osv-scanner 2.6.0 over `pnpm-lock.yaml`) reports is
triaged here exactly once, keyed on `package@version` plus advisory id. The point is that the
next run does not re-open a question this run already answered: an unreachable advisory stays
recorded as unreachable, with the reason that made it so.

Regenerate the raw findings with:

```sh
pnpm scan:deps
osv-scanner scan source -L pnpm-lock.yaml --all-packages --format json
```

## Classification vocabulary

- `reachable-in-production` — the vulnerable code path can execute while serving a production
  request, so the advisory is a live risk and drives remediation.
- `dev-only` — the package is exercised only by the build, lint or test toolchain; it never runs
  in the deployed request path.
- `not-reachable` — the package is present in the production dependency closure but the
  vulnerable code path is not invoked by our usage.
- `retired-unused` — the package belonged to the retired TypeScript stack (Next.js, React and
  their companions). Nothing in this repository or in the deploy loads it: there is no
  `next build`, no Node runtime in production, and no script that imports it. It remains in
  `pnpm-lock.yaml` only because nobody has removed the dependency, so an advisory against it
  describes code that cannot run here. Added 2026-09-28, when the ledger was found describing
  `next` as "the production application runtime" a full port after that stopped being true.

**A CRITICAL THAT WE BELIEVE IS REACHABLE FAILS THE BUILD.** `gates.yml` refuses a run in which a
reported advisory has a ledger row classifying it `reachable-in-production` at CRITICAL severity:
that is the ledger saying "yes, this one can hurt us", and a green pipeline must not be able to say
that. Everything else — including a critical recorded as `retired-unused`, `dev-only` or
`not-reachable`, with its reason — passes. That is the difference between triage and silence.

When a new advisory appears, add one row. When the lockfile changes, re-run the scan and update
the rows for the changed packages.

| Package | Version | Advisory | Severity | Classification | Reason |
|---|---|---|---|---|---|
| next | 16.3.0 | GHSA-2xp9-vwfh-vxw4 | CRITICAL | retired-unused | The Next.js application was retired: there is no `next build`, no Node runtime in the deploy (the site is Yew/wasm served by the Rust server), and no live script that imports it. Corrected 2026-09-28 — the row said "reachable-in-production" for a full port after that stopped being true. |
| next | 16.3.0 | GHSA-p293-qw3h-jr36 | CRITICAL | retired-unused | Same as above: the advisory sits in the retired TypeScript runtime's dependency, not in anything this repository builds or serves. |
| brace-expansion | 5.0.6 | GHSA-3jxr-9vmj-r5cp | HIGH | dev-only | Glob expansion used by build and lint tooling; not invoked while serving a request. |
| brace-expansion | 5.0.6 | GHSA-mh99-v99m-4gvg | HIGH | dev-only | Glob expansion used by build and lint tooling; not invoked while serving a request. |
| brace-expansion | 5.0.6 | GHSA-rgw5-rvv9-x895 | HIGH | dev-only | Glob expansion used by build and lint tooling; not invoked while serving a request. |
| browserslist | 4.28.1 | GHSA-73wf-gq98-2v4g | HIGH | dev-only | Browser target resolution runs at build time only. |
| browserslist | 4.28.1 | GHSA-c83g-rgw3-j3cx | HIGH | dev-only | Browser target resolution runs at build time only. |
| fast-uri | 3.1.2 | GHSA-4c8g-83qw-93j6 | HIGH | not-reachable | URI parsing reached through build and validation tooling, not by production request handling. |
| fast-uri | 3.1.2 | GHSA-7p8r-x3mc-p8w7 | HIGH | not-reachable | URI parsing reached through build and validation tooling, not by production request handling. |
| fast-uri | 3.1.2 | GHSA-f65p-4m7j-42xc | HIGH | not-reachable | URI parsing reached through build and validation tooling, not by production request handling. |
| brace-expansion | 5.0.6 | GHSA-6j4f-fj2g-mc7p | HIGH | dev-only | Stack-exhaustion DoS in the glob expander, reached only through eslint → minimatch (devDependencies). Not invoked while serving a request; fixed upstream in 5.0.10. |
| brace-expansion | 5.0.6 | GHSA-q2hr-2g5m-vwhr | MODERATE | dev-only | Quadratic-time rewrite of nested braces, same eslint → minimatch chain; a denial of service against the developer's own shell, not the deploy. Fixed upstream in 5.0.12. |
| brace-expansion | 5.0.6 | GHSA-qhr7-859c-m2p7 | HIGH | dev-only | Recursion on nested brace groups, same eslint → minimatch chain as the two rows above. Fixed upstream in 5.0.11. |
| fast-uri | 3.1.2 | GHSA-hrr3-gc8f-f4qj | MODERATE | not-reachable | Host-case normalization through percent-encoded octets, reached via ajv ← @modelcontextprotocol/sdk ← shadcn, the scaffolding CLI. Same chain and same reasoning as the three fast-uri rows above: validation tooling, not production request handling. Fixed on the 2.x line in 2.4.7. |
| hono | 4.12.25 | GHSA-hxh3-vqpv-xpqv | MODERATE | not-reachable | Unescaped strings in hono's JSX boundary components, reached only through @hono/node-server ← @modelcontextprotocol/sdk ← shadcn. That renderer is exercised by the CLI, never by the deployed site — Rust/Yew serves it and there is no Node runtime in production. Fixed upstream in 4.13.7. |
| ip-address | 10.2.0 | GHSA-h3mg-xc3c-68pw | MODERATE | not-reachable | Parse diagnostic proportional to input with no length bound, reached two ways: express-rate-limit ← @modelcontextprotocol/sdk ← shadcn, and socks ← imapflow. Nothing in this repository imports imapflow, and no Node process runs in the deploy, so the address parser never sees attacker-supplied input. Fixed upstream in 10.7.1. |
| ip-address | 10.2.0 | GHSA-j6r3-76f7-8jcv | MODERATE | not-reachable | isInSubnet()/isHostInSubnet() comparing addresses of different families as if they shared one, same two chains as the row above; our usage never compares across families. Fixed upstream in 10.7.1. |
| next | 16.3.0 | GHSA-vcvr-r3jv-pc5j | CRITICAL | retired-unused | Remote code execution in next/og ImageResponse. The Next.js application was retired and its tree deleted from this branch: no next build runs, no Node runtime is deployed (the site is Yew/wasm served by the Rust server), and nothing imports next. The package sits in dependencies only because nobody removed it — see the note below. Fixed upstream in 16.3.6. |

## The 2026-10-02 rows: what they are saying

Eight rows were added on 2026-10-02, when `static gates` went red reporting eight untriaged advisories. Four of them
(three `brace-expansion` and one `next`) tell the story this ledger has already told: build tooling, and a framework
that was retired. The other four (`fast-uri`, `hono`, two `ip-address`) arrive through one manifest smell worth
naming out loud:

`shadcn` is a component **scaffolding CLI**, and it is declared in `dependencies` rather than `devDependencies`. It
drags `@modelcontextprotocol/sdk` — and with it `ajv`, `hono` and `express-rate-limit` — into the production
dependency closure of a repository that deploys no Node runtime at all. `imapflow` and `socks` are the same shape:
declared in `dependencies`, imported by nothing in the tree (`grep -rn imapflow` outside `node_modules` finds
nothing).

No advisory is hidden by these rows — each one is recorded with its severity and the chain that reaches it. But the
remediation is manifest hygiene rather than more triage: move `shadcn` and `imapflow` (and `socks` with it) to
`devDependencies`, or delete them, and five of these eight rows stop being reported at all. Until then these rows
stand as the reason each finding cannot reach a request.

| fast-uri | 3.1.2 | GHSA-fph4-wmhf-6fwf | HIGH | not-reachable | URI parsing reached through build and validation tooling, not by production request handling. |
| fast-uri | 3.1.2 | GHSA-qw65-cvwx-89v3 | HIGH | retired-unused | Reached only as `ajv@8.20.0 -> fast-uri`. Nothing under `scripts/`, `agent-runtime/` or `workflow_app/` imports ajv or fast-uri, and the deployed request path is Rust — no JavaScript runs in production. Added 2026-09-28, when the strengthened gate reached this step for the first time (CI had been failing at `lint` before it). |
| ip-address | 10.2.0 | GHSA-2vr4-cq9g-pvrc | MODERATE | retired-unused | Reached only as `express-rate-limit@8.5.2 -> ip-address` and `socks@2.8.9 -> ip-address`. Neither express nor socks is imported anywhere in this repository, and no Node runtime is deployed. |
| ip-address | 10.2.0 | GHSA-rpw4-54j3-4h4q | MODERATE | retired-unused | Same chain as the row above. |

| fast-uri | 3.1.2 | GHSA-jqff-g426-hqxp | HIGH | not-reachable | URI parsing reached through build and validation tooling, not by production request handling. |
| fast-uri | 3.1.2 | GHSA-v2hh-gcrm-f6hx | HIGH | not-reachable | URI parsing reached through build and validation tooling, not by production request handling. |
| ip-address | 10.2.0 | GHSA-mwp4-54f8-5fhr | HIGH | not-reachable | IP parsing reached only through transitive tooling; not on a production request path. |
| js-yaml | 4.2.0 | GHSA-2883-xcg3-v3hh | HIGH | not-reachable | YAML parsing used by build and configuration tooling, never on untrusted production input. |
| js-yaml | 4.2.0 | GHSA-52cp-r559-cp3m | HIGH | not-reachable | YAML parsing used by build and configuration tooling, never on untrusted production input. |
| js-yaml | 4.2.0 | GHSA-5p4m-2wfm-xmqj | HIGH | not-reachable | YAML parsing used by build and configuration tooling, never on untrusted production input. |
| nanoid | 3.3.11 | GHSA-28wg-ghj8-5hjv | HIGH | dev-only | ID generation reached through PostCSS and build tooling; production identifiers use other generators. |
| nanoid | 3.3.11 | GHSA-2v37-7h3g-55p8 | HIGH | dev-only | ID generation reached through PostCSS and build tooling; production identifiers use other generators. |
| nanoid | 3.3.16 | GHSA-2v37-7h3g-55p8 | HIGH | dev-only | ID generation reached through PostCSS and build tooling; production identifiers use other generators. |
| nanoid | 3.3.11 | GHSA-xwg4-73v4-xw9w | HIGH | dev-only | ID generation reached through PostCSS and build tooling; production identifiers use other generators. |
| postcss | 8.5.6 | GHSA-6g55-p6wh-862q | HIGH | dev-only | PostCSS processes only first-party CSS at build time. |
| postcss | 8.5.6 | GHSA-r28c-9q8g-f849 | HIGH | dev-only | PostCSS processes only first-party CSS at build time. |
| sharp | 0.35.3 | GHSA-rgj7-g3m4-5g8c | HIGH | reachable-in-production | sharp performs production image optimization for the Next.js image pipeline, which processes remote image input. |
| @hono/node-server | 1.19.14 | GHSA-frvp-7c67-39w9 | MODERATE | not-reachable | Hono server adapter arrives only as a transitive tooling dependency; Next.js serves all production HTTP, so no Hono request path executes. |
| baseline-browser-mapping | 2.9.19 | GHSA-w5vr-8v7q-w6rv | MODERATE | dev-only | Build-time browser target data consumed by Browserslist while bundling; absent from the request path. |
| hono | 4.12.25 | GHSA-54fx-42gc-7vw4 | MODERATE | not-reachable | Hono is a transitive tooling dependency; no Hono server is mounted in production. |
| hono | 4.12.25 | GHSA-8j4g-w8fx-2239 | MODERATE | not-reachable | Hono is a transitive tooling dependency; no Hono server is mounted in production. |
| hono | 4.12.25 | GHSA-crvj-82cr-hjcx | MODERATE | not-reachable | Hono is a transitive tooling dependency; no Hono server is mounted in production. |
| hono | 4.12.25 | GHSA-f23p-vx2j-j53r | MODERATE | not-reachable | Hono is a transitive tooling dependency; no Hono server is mounted in production. |
| hono | 4.12.25 | GHSA-g6gw-c38x-mqfc | MODERATE | not-reachable | Hono is a transitive tooling dependency; no Hono server is mounted in production. |
| hono | 4.12.25 | GHSA-gqvv-2mrq-wpjv | MODERATE | not-reachable | Hono is a transitive tooling dependency; no Hono server is mounted in production. |
| hono | 4.12.25 | GHSA-hvrm-45r6-mjfj | MODERATE | not-reachable | Hono is a transitive tooling dependency; no Hono server is mounted in production. |
| hono | 4.12.25 | GHSA-w62v-xxxg-mg59 | MODERATE | not-reachable | Hono is a transitive tooling dependency; no Hono server is mounted in production. |
| hono | 4.12.25 | GHSA-xgm2-5f3f-mvvc | MODERATE | not-reachable | Hono is a transitive tooling dependency; no Hono server is mounted in production. |
| ip-address | 10.2.0 | GHSA-22jq-vg5j-6vgg | MODERATE | not-reachable | IP parsing reached only through transitive tooling; not on a production request path. |
| ip-address | 10.2.0 | GHSA-4xrf-jv44-h6hh | MODERATE | not-reachable | IP parsing reached only through transitive tooling; not on a production request path. |
| postcss | 8.5.19 | GHSA-fxqj-rqcc-2cmp | MODERATE | dev-only | PostCSS processes only first-party CSS at build time. |
| postcss | 8.5.6 | GHSA-fxqj-rqcc-2cmp | MODERATE | dev-only | PostCSS processes only first-party CSS at build time. |
| postcss | 8.5.6 | GHSA-qx2v-qp2m-jg93 | MODERATE | dev-only | PostCSS processes only first-party CSS at build time. |
| qs | 6.15.2 | GHSA-4mjr-xmp4-gh2g | MODERATE | not-reachable | Query-string parsing reached through transitive tooling; production query parsing is handled by Next.js. |
| qs | 6.15.2 | GHSA-x5fp-wj9c-mxmx | MODERATE | not-reachable | Query-string parsing reached through transitive tooling; production query parsing is handled by Next.js. |
| body-parser | 2.2.2 | GHSA-v422-hmwv-36x6 | LOW | not-reachable | Express body parsing reached only through transitive tooling; no Express application runs in production. |
| hono | 4.12.25 | GHSA-79qm-7rj5-m7r9 | LOW | not-reachable | Hono is a transitive tooling dependency; no Hono server is mounted in production. |
| postcss-selector-parser | 7.1.1 | GHSA-w9m9-85wc-3x92 | LOW | dev-only | Selector parsing runs inside the PostCSS build pipeline. |
