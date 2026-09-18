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

When a new advisory appears, add one row. When the lockfile changes, re-run the scan and update
the rows for the changed packages.

| Package | Version | Advisory | Severity | Classification | Reason |
|---|---|---|---|---|---|
| next | 16.3.0 | GHSA-2xp9-vwfh-vxw4 | CRITICAL | reachable-in-production | Next.js is the production application runtime on Vercel; the advisory affects code reachable from incoming requests. |
| next | 16.3.0 | GHSA-p293-qw3h-jr36 | CRITICAL | reachable-in-production | Next.js is the production application runtime on Vercel; the advisory affects code reachable from incoming requests. |
| brace-expansion | 5.0.6 | GHSA-3jxr-9vmj-r5cp | HIGH | dev-only | Glob expansion used by build and lint tooling; not invoked while serving a request. |
| brace-expansion | 5.0.6 | GHSA-mh99-v99m-4gvg | HIGH | dev-only | Glob expansion used by build and lint tooling; not invoked while serving a request. |
| brace-expansion | 5.0.6 | GHSA-rgw5-rvv9-x895 | HIGH | dev-only | Glob expansion used by build and lint tooling; not invoked while serving a request. |
| browserslist | 4.28.1 | GHSA-73wf-gq98-2v4g | HIGH | dev-only | Browser target resolution runs at build time only. |
| browserslist | 4.28.1 | GHSA-c83g-rgw3-j3cx | HIGH | dev-only | Browser target resolution runs at build time only. |
| fast-uri | 3.1.2 | GHSA-4c8g-83qw-93j6 | HIGH | not-reachable | URI parsing reached through build and validation tooling, not by production request handling. |
| fast-uri | 3.1.2 | GHSA-7p8r-x3mc-p8w7 | HIGH | not-reachable | URI parsing reached through build and validation tooling, not by production request handling. |
| fast-uri | 3.1.2 | GHSA-f65p-4m7j-42xc | HIGH | not-reachable | URI parsing reached through build and validation tooling, not by production request handling. |
| fast-uri | 3.1.2 | GHSA-fph4-wmhf-6fwf | HIGH | not-reachable | URI parsing reached through build and validation tooling, not by production request handling. |
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
