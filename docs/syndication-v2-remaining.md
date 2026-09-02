# Syndication V2 remaining (apply on feat/marketing-syndication)

Architect already landed: `facebookReadiness`, photo hints, types.externalUrl, expire helper, dashboard Meta strip, TTL event label.

Finish these exact edits.

## 1. last_error / ok (required)

`lib/syndication/adapters.ts` Facebook return:

```
return { ...result, ok: status !== 'failed', externalId: graphPostId, externalUrl: graphUrl }
```

Compute `graphUrl` from Page feed id (`https://www.facebook.com/${id}` when id contains `_`).

`db/syndication.ts` insert last_error:

```
${result.status === 'failed' || !result.ok ? result.message : null}
```

and persist `input.externalUrl ?? result.externalUrl ?? null`.

Test: live Graph 400 => `result.ok === false`.

## 2. Catalog optional in missingEnv

`facebookTransportPlan`:

```
const missing = missingEnv(['META_ACCESS_TOKEN', 'META_PAGE_ID'])
```

Add `facebookPostUrl()` helper at bottom of `facebook.ts` if not present.

## 3. Workbench Meta strip + photo hint

`SyndicationWorkbench` props add `facebook?: FacebookReadiness`.
Render the same present/absent line as the dashboard (no secrets).
Show `pack.photoHint` as a PackField labeled Photos.

`app/portal/marketing/syndication/page.tsx` should pass `facebook={facebookReadiness()}` and keep `await expireStalePlacements()` from `@/db/syndication-expire`.

## 4. Tests

- failed live => ok false
- clasificados photoHint matches `/api/media/m1`
- missingEnv empty with token+page and no catalog

Do not add HubSpot or OAuth.
