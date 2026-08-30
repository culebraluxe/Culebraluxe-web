import { checkPdfLogoAsset } from '@/lib/forms/logo-health'

// Temporary deployment-only probe; remove after serverless asset verification.
export const runtime = 'nodejs'
export const dynamic = 'force-dynamic'

export async function GET() {
  try {
    return Response.json(await checkPdfLogoAsset(), {
      headers: { 'Cache-Control': 'no-store' },
    })
  } catch (error) {
    return Response.json(
      {
        ok: false,
        message: error instanceof Error ? error.message : String(error),
      },
      { status: 500, headers: { 'Cache-Control': 'no-store' } },
    )
  }
}
