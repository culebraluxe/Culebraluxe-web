# Copy into your existing Vercel / Next.js app

From the unzipped folder:

```
src/app/api/whatsapp/webhook/route.ts   →  app/api/whatsapp/webhook/route.ts
src/lib/whatsapp/*                      →  lib/whatsapp/*   (or src/lib/whatsapp/*)
sql/001_whatsapp_events.sql             →  run on Neon
```

If your app does **not** use the `@/` alias, change the imports in `route.ts` from:

```ts
import { persistMessages } from "@/lib/whatsapp/db";
```

to a relative path:

```ts
import { persistMessages } from "../../../../lib/whatsapp/db";
```

(adjust to match where you put `lib/whatsapp`).

`route.ts` must stay at `app/api/whatsapp/webhook/route.ts` so the public URL is `/api/whatsapp/webhook`.

If `contacts` has no `phone_e164` column, the extra `or phone_e164 = ...` clause is harmless on Postgres only if the column exists. Remove that line in `db.ts` if the column is absent.
