import { resolveRustApiBaseUrl } from '@/lib/rust-api/contract'

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

    const ready =
      response.ok &&
      body.ok === true &&
      typeof body.databaseTarget === 'string' &&
      body.databaseTarget.length > 0

    return Response.json(
      {
        ok: ready,
        service: 'culebraluxe-rust',
        databaseTarget: body.databaseTarget ?? null,
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
