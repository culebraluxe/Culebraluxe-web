// ---------------------------------------------------------------------------
// SERVER ERROR INSTRUMENTATION — no server failure is allowed to be invisible.
//
// WHY THIS EXISTS: on 2026-09-15 a drag on the Cockpit exposed a failure with no voice. The server
// action never wrote, the browser saw React error #441 ("An error occurred in the Server Components
// render" - the message Next strips in production), and `app_error` stayed empty: nothing captured the
// error, so the only evidence was a digest nobody could read. That is the same defect this repo keeps
// meeting in different clothes - a failure that does not name itself.
//
// `onRequestError` is Next's own hook for this: it fires for server-component renders, server actions
// and route handlers, and hands over the error with its digest and route context. Routing it into the
// durable capture framework means a digest in a browser error can always be matched to a row.
//
// EDGE SAFETY, learned by breaking a deploy: this file is bundled for BOTH runtimes, and a top-level
// import of the database client pulled `node:fs`/`node:crypto`/`node:util/types` into the Edge
// middleware bundle, which Vercel rejects outright ("The Edge Function 'middleware' is referencing
// unsupported modules"). So nothing Node-only may be imported at module scope here: the runtime is
// checked first and the imports happen inside that branch.
// ---------------------------------------------------------------------------

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
  // The Edge runtime cannot reach the database. Records are written where they can be.
  if (process.env.NEXT_RUNTIME !== 'nodejs') return

  const route = [
    context?.routeType,
    context?.routerKind,
    request?.method,
    request?.path ?? context?.routePath,
  ]
    .filter(Boolean)
    .join(' ')

  const [client, seam] = await Promise.all([
    import('@/db/client'),
    import('@/lib/server-error-capture'),
  ])

  // Record with an EXPLICIT executor: the registration seam is not guaranteed in this context, and a
  // capture that silently does nothing is how the last failure stayed quiet.
  await client.sql`
    insert into app_error (kind, operation, message, stack, route, level, meta)
    values (
      ${(error as Error)?.name || 'Error'},
      'next.onRequestError',
      ${String((error as Error)?.message ?? error).slice(0, 500)},
      ${String((error as Error)?.stack ?? '').slice(0, 4000)},
      ${route || null},
      'error',
      ${JSON.stringify({ digest: (error as { digest?: unknown })?.digest ?? null })}::jsonb
    )
  `.catch(() => {
    // A failure to record must not become a second failure.
  })

  const digest = (error as { digest?: unknown })?.digest
  if (typeof digest === 'string' && digest.length > 0) {
    seam.captureServerLog('warn', 'next.onRequestError.digest', `digest=${digest} route=${route}`, {
      route: route || null,
    })
  }
}
