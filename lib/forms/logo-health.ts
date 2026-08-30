import { readFile } from 'node:fs/promises'

export async function checkPdfLogoAsset(): Promise<{
  ok: boolean
  bytes: number
  png: boolean
}> {
  const raw = await readFile(
    new URL('../../public/brand/CLLOGO.png', import.meta.url),
  )
  return {
    ok: true,
    bytes: raw.length,
    png:
      raw.length >= 8 &&
      raw[0] === 0x89 &&
      raw[1] === 0x50 &&
      raw[2] === 0x4e &&
      raw[3] === 0x47,
  }
}
