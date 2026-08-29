# WhatsApp Cloud API → Neon (receive-only)

Drop-in webhook for a Next.js App Router app on Vercel.
Meta POSTs events here. You keep chatting in the WhatsApp Business phone app.
This does **not** send messages and does **not** include an inbox UI.

Callback URL to paste in Meta:

```
https://YOUR_PRODUCTION_DOMAIN/api/whatsapp/webhook
```

## What you copy into your repo

```
src/app/api/whatsapp/webhook/route.ts
src/lib/whatsapp/phone.ts
src/lib/whatsapp/verify.ts
src/lib/whatsapp/parse.ts
src/lib/whatsapp/db.ts
src/lib/whatsapp/types.ts
sql/001_whatsapp_events.sql
```

Install (if you do not already have it):

```bash
npm i @neondatabase/serverless
```

## Env (Vercel → Settings → Environment Variables)

```
DATABASE_URL                  Neon pooled connection string
WHATSAPP_VERIFY_TOKEN         Any long random string (you invent this)
WHATSAPP_APP_SECRET           Meta app → Settings → Basic → App secret
WHATSAPP_PHONE_NUMBER_ID      Optional. Ignore events from other numbers.
CONTACTS_TABLE                Optional. Default: contacts
CONTACTS_PHONE_COLUMN         Optional. Default: phone
```

`CONTACTS_PHONE_COLUMN` should already store E.164 (`+17875551212`).
If your column is named `phone_e164`, set that.

## SQL

Run `sql/001_whatsapp_events.sql` against Neon (SQL editor or `psql $DATABASE_URL -f ...`).

It creates `whatsapp_events` and adds last-touch columns on `contacts` **only if that table exists**.
If your people table is not named `contacts`, edit the last section of the SQL before running.

## One-time Meta setup (you, not this code)

1. Convert the phone number to WhatsApp Business. Backup first.
2. business.facebook.com → Business portfolio.
3. developers.facebook.com → App type Business → add WhatsApp product.
4. Accept WhatsApp Business Platform terms.
5. Onboard the **same** number with **coexistence** (keep the phone app).
6. WhatsApp → Configuration → Webhook:
   - Callback URL = `https://YOUR_DOMAIN/api/whatsapp/webhook`
   - Verify token = same value as `WHATSAPP_VERIFY_TOKEN`
   - Verify
7. Subscribe fields: `messages`, `smb_message_echoes`. Optionally `history` at first connect.
8. Send a test inbound and a test outbound from the phone. Check `whatsapp_events` in Neon.

Open the Business app at least once every 13 days or coexistence can drop.

## Local handshake test

```bash
curl -sS "http://localhost:3000/api/whatsapp/webhook?hub.mode=subscribe&hub.verify_token=YOUR_TOKEN&hub.challenge=1158201444"
# expect: 1158201444
```

Do not use unofficial WhatsApp Web libraries (Baileys, whatsapp-web.js) with this.
