// ---------------------------------------------------------------------------
// SERVER ERROR INSTRUMENTATION — no server failure is allowed to be invisible.
//
// WHY THIS EXISTS: on 2026-09-15 a drag on the Cockpit exposed a failure with no voice. The server
// action never wrote, the browser saw React error #441 ("An error occurred in the Server Components
// render" — the message Next strips in production), and `app_error` stayed empty: nothing captured the
// error, so the only evidence was a digest nobody could read. That is the same defect this repo keeps
// meeting in different clothes - a failure that does not name itself.
//
// `onRequestError` is Next's own hook for exactly this: it fires for server-component renders, server
// actions and route handlers, and it hands over the error with its digest, the request and the route
// context. Routing it into the durable capture framework means a digest in a browser error can always be
// matched to a row that says what actually happened.
//
// `@/db/client` is imported for its side effect: it registers the app_error executor. Without it the
// capture seam throws "app_error executor is not configured" and the record is lost - which is how this
// failure stayed quiet for so long.
// ---------------------------------------------------------------------------

import '@/db/client'

import { captureServerError, captureServerLog } from '@/lib/server-error-capture'

type RequestLike = {
  path?: string
  method?: string
}

type ContextLike = {
  routerKind?: string
  routePath?: string
  routeType?: string
  renderSource?: string
}

export async function onRequestError(
  error: unknown,
  request: RequestLike,
  context: ContextLike,
): Promise<void> {
  const digest = (error as { digest?: unknown })?.digest
  const route = [context?.routeType, context?.routerKind, request?.method, request?.path ?? context?.routePath]
    .filter(Boolean)
    .join(' ')

  captureServerError('next.onRequestError', error, { route: route || null })
  if (typeof digest === 'string' && digest.length > 0) {
    // The digest is what a browser error shows instead of the message; recording it makes the two ends
    // of the same failure findable from either side.
    captureServerLog('warn', 'next.onRequestError.digest', `digest=${digest} route=${route}`, {
      route: route || null,
    })
  }
}
