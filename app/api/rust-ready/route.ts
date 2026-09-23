import { resolveInternalApiKey, resolveRustApiBaseUrl } from '@/lib/rust-api/contract'

export const dynamic = 'force-dynamic'

type RustReady = {
  ok?: boolean
  databaseTarget?: string
}

export async function GET() {
  const base = resolveRustApiBaseUrl(process.env.RUST_API_BASE_URL, process.env.NODE_ENV)
  if (!base) {
    return Response.json(
      { ok: false, error: 'rust_api_binding_missing' },
      { status: 503, headers: { 'cache-control': 'no-store' } },
    )
  }

  try {
    const response = await fetch(`${base}/readyz`, {
      cache: 'no-store',
      signal: AbortSignal.timeout(10_000),
    })
    const body = (await response.json()) as RustReady

    const databaseReady =
      response.ok &&
      body.ok === true &&
      typeof body.databaseTarget === 'string' &&
      body.databaseTarget.length > 0

    const internalKey = resolveInternalApiKey(
      process.env.CULEBRA_INTERNAL_API_KEY,
      process.env.AUTH_SECRET,
    )
    if (!databaseReady || !internalKey) {
      return Response.json(
        {
          ok: false,
          service: 'culebraluxe-rust',
          databaseTarget: body.databaseTarget ?? null,
          bridgeAuth: false,
          error: !databaseReady ? 'rust_database_not_ready' : 'rust_bridge_key_missing',
        },
        { status: 503, headers: { 'cache-control': 'no-store' } },
      )
    }

    // Exercise the same internal-key gate used by every authenticated portal request.
    // /readyz alone cannot detect a frontend/Rust secret mismatch.
    const bridgeResponse = await fetch(`${base}/v1/diagnostics/db`, {
      cache: 'no-store',
      signal: AbortSignal.timeout(10_000),
      headers: {
        accept: 'application/json',
        'x-culebra-internal-key': internalKey,
      },
    })
    const bridgeAuth = bridgeResponse.ok
    const ready = databaseReady && bridgeAuth

    return Response.json(
      {
        ok: ready,
        service: 'culebraluxe-rust',
        databaseTarget: body.databaseTarget ?? null,
        bridgeAuth,
        error: bridgeAuth ? null : 'rust_bridge_auth_failed',
      },
      {
        status: ready ? 200 : 503,
        headers: { 'cache-control': 'no-store' },
      },
    )
  } catch {
    return Response.json(
      { ok: false, error: 'rust_api_unreachable' },
      { status: 503, headers: { 'cache-control': 'no-store' } },
    )
  }
}
