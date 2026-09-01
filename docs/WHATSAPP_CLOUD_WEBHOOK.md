# WhatsApp Cloud API webhook

CulebraLuxe receives Meta WhatsApp Cloud API events at:

```text
https://www.culebraluxe.com/api/integrations/whatsapp/webhook
```

The route is a provider edge only. It verifies Meta's signature, lowers each
message into the existing source-neutral realtime intake contract, and sends it
through `integration_inbox`, canonical phone identity resolution, and the
`interaction.record` command. It does not create a parallel WhatsApp table or
change the Apple intake path.

## Retention

The adapter is metadata-only. It does not store message bodies, captions, or the
raw Meta webhook. It stores stable provider message identity, direction,
timestamp, participant phone identity, message type, bounded media descriptors,
and canonical convergence status. This matches the existing communications
retention policy.

## Vercel environment

Configure these values for Production (and Preview only when testing against
the DEV database):

```text
WHATSAPP_VERIFY_TOKEN       Long random value created by the operator
WHATSAPP_APP_SECRET         Meta app secret
WHATSAPP_PHONE_NUMBER_ID    Meta WhatsApp business phone-number ID
WHATSAPP_OWNED_PHONE_E164   Owned business line, for example +17875551212
```

`DATABASE_URL_DEV` and `DATABASE_URL_PROD` remain the existing application
database settings. The webhook uses the application database gateway and fails
closed on missing or contradictory environment routing.

Never commit any of these values.

## Signed production fixture

The repository includes a Mac-compatible script that creates the Meta-shaped
JSON, signs its exact raw bytes, and posts it to the production webhook. It
automatically reads the gitignored `.env.local` file and defaults to sender
`+1 617-251-6169`:

```sh
pnpm whatsapp:webhook:test
```

The script prompts for any missing configuration. A different gitignored env
file can be selected explicitly:

```sh
pnpm whatsapp:webhook:test --env-file .env.production.local
```

To test with the fictional `617-555-0169` sender instead:

```sh
pnpm whatsapp:webhook:test --from 6175550169
```

The fixture is a real write to the selected webhook environment. A fictional
sender normally produces an unresolved intake item; a real sender resolves only
when its normalized phone identity already belongs to a canonical person.

The verification handshake can be checked separately:

```sh
pnpm whatsapp:webhook:test --env-file .env.production.local --handshake
```

## Meta configuration

1. Set the callback URL to the endpoint above.
2. Set Meta's verify token to the exact `WHATSAPP_VERIFY_TOKEN` value.
3. Subscribe to `messages` and `smb_message_echoes`.
4. Use coexistence when the same number must remain active in the WhatsApp
   Business mobile application.
5. Send one inbound message and one phone-app reply.

Expected database outcome:

- one idempotent `integration_inbox` receipt per Meta message ID;
- a canonical `interaction` with channel `whatsapp` when the external E.164
  phone already resolves to a non-archived `person_identity`;
- `resolution_required` when no canonical person owns that phone;
- no `whatsapp_events` table and no direct person/contact mutation.

Transient processing failures return a retryable HTTP error to Meta. Replayed
deliveries are safe through both the inbox source key and canonical interaction
source key.
