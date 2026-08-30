import { readFile } from 'node:fs/promises'
import path from 'node:path'
import { PDFDocument } from 'pdf-lib'

export const runtime = 'nodejs'
export const dynamic = 'force-dynamic'

export async function GET() {
  try {
    const filePath = path.join(process.cwd(), 'public', 'brand', 'CLLOGO.png')
    const raw = await readFile(filePath)
    const doc = await PDFDocument.create()
    const image = await doc.embedPng(raw)
    return Response.json({
      ok: true,
      cwd: process.cwd(),
      filePath,
      bytes: raw.length,
      width: image.width,
      height: image.height,
    }, { headers: { 'Cache-Control': 'no-store' } })
  } catch (error) {
    return Response.json({
      ok: false,
      cwd: process.cwd(),
      message: error instanceof Error ? error.message : String(error),
    }, { status: 500, headers: { 'Cache-Control': 'no-store' } })
  }
}
