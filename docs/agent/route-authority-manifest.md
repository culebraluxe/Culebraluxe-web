# Route Authority Manifest

GENERATED FILE — do not hand-edit. Regenerate with `node --import tsx scripts/route-authority-manifest.ts --write`.

Every exported HTTP handler under `app/api/**/route.ts` appears exactly once. `decision` is `authority` (a canonical guard was detected in the handler), `session` (the handler consults the portal session but makes no Portal authority decision), `public` (deliberately reachable without a Portal authority or session), `machine` (verified by signature or shared secret), or `unguarded` (no authority decision yet — reason and fixing story required).

| Path | Method | Decision | Authority | Evidence | Reason |
| --- | --- | --- | --- | --- | --- |
| `app/api/auth/[...nextauth]/route.ts` | GET | public |  | exception-registry | Auth.js sign-in, callback, signout and session handlers must be reachable before authentication. |
| `app/api/auth/[...nextauth]/route.ts` | POST | public |  | exception-registry | Auth.js sign-in, callback, signout and session handlers must be reachable before authentication. |
| `app/api/build-info/route.ts` | GET | public |  | exception-registry | Deploy-verification endpoint; exposes version, commit and build time only, no secret. |
| `app/api/integrations/boldsign/webhook/route.ts` | POST | machine |  | exception-registry | Machine caller: BoldSign webhook signature verification. |
| `app/api/integrations/whatsapp/coexistence/complete/route.ts` | POST | authority | portal.read | resolvePortalAccess |  |
| `app/api/integrations/whatsapp/webhook/route.ts` | GET | machine |  | exception-registry | Machine caller: Meta webhook handshake token verification (GET) and HMAC signature (POST). |
| `app/api/integrations/whatsapp/webhook/route.ts` | POST | machine |  | exception-registry | Machine caller: Meta webhook handshake token verification (GET) and HMAC signature (POST). |
| `app/api/media/[id]/route.ts` | GET | session |  | getToken |  |
| `app/api/media/documents/[id]/route.ts` | GET | session |  | getToken |  |
| `app/api/media/upload/route.ts` | POST | authority | listing.write | guardPortalUpload |  |
| `app/api/portal/client-error/route.ts` | POST | public |  | exception-registry | Anonymous by design: the portal error boundary reports failures even when auth or the database are down. |
| `app/api/portal/clients/[personId]/history/route.ts` | GET | authority | portal.read | resolvePortalAccess |  |
| `app/api/portal/clients/[personId]/property-context/route.ts` | GET | authority | portal.read | resolvePortalAccess |  |
| `app/api/portal/clients/[personId]/relationship-channels/route.ts` | GET | authority | portal.read | resolvePortalAccess |  |
| `app/api/portal/clients/[personId]/route.ts` | GET | authority | portal.read | resolvePortalAccess |  |
| `app/api/portal/clients/agents/route.ts` | GET | authority | portal.read | resolvePortalAccess |  |
| `app/api/portal/clients/route.ts` | GET | authority | portal.read | resolvePortalAccess |  |
| `app/api/portal/db-diag/route.ts` | GET | authority | tech.access | resolvePortalAccess |  |
| `app/api/portal/flight-recorder/[instanceId]/route.ts` | GET | authority | tech.access | resolvePortalAccess |  |
| `app/api/portal/form-sidecar/listing/route.ts` | GET | authority | portal.read | runAuthorized |  |
| `app/api/portal/form-sidecar/listing/route.ts` | POST | authority | listing.write | runAuthorized |  |
| `app/api/portal/form-sidecar/listing/select-client/route.ts` | POST | authority | listing.write | runAuthorized |  |
| `app/api/portal/form-sidecar/pns/route.ts` | GET | authority | portal.read | runAuthorized |  |
| `app/api/portal/form-sidecar/pns/route.ts` | POST | authority | deal.write | runAuthorized |  |
| `app/api/portal/issues/route.ts` | GET | authority | portal.read | resolvePortalAccess |  |
| `app/api/portal/move-trace/route.ts` | POST | public |  | exception-registry | Anonymous by design: the board reports drag observations from the browser; stores clipped labels only. |
| `app/api/portal/relationship-evidence-review/actions/route.ts` | POST | authority | crm.write | resolvePortalAccess |  |
| `app/api/portal/relationship-evidence-review/route.ts` | GET | authority | portal.read | resolvePortalAccess |  |
| `app/api/portal/runtime-inspector/[instanceId]/route.ts` | GET | authority | tech.access | resolvePortalAccess |  |
| `app/api/property-media/upload/route.ts` | POST | authority | listing.write | guardPortalUpload |  |
| `app/api/system/agreement-execution-recovery/route.ts` | POST | machine |  | exception-registry | Machine caller: fails closed unless AGREEMENT_EXECUTION_RECOVERY_KEY is set and x-recovery-key matches. |

Handlers: 31
