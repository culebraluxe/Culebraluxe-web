# Composition patch (apply in CIL-04, do not paste blindly)

In `services/composition.ts`:

1. Import `CouncilService`, `CouncilRepository`.
2. Add `council?: CouncilRepository` to `CoreServiceRepositories`.
3. Add `council?: CouncilService` to `CoreServiceComposition`.
4. Register when present:

```ts
const council = repositories.council
  ? registry.register(new CouncilService(repositories.council, serviceInfrastructure))
  : undefined
```

5. Return `council` on the composition object.

Production kernel always passes `createCouncilRepository()`.

Entitlement policy must know `council.read` and `council.write`. Copy the
pattern used for `showing.read` / `showing.write`. Fail closed.
