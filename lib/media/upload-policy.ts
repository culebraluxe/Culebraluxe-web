// HARDEN-06 — bounded, baby-V1 media upload policy (shared by upload routes).
//
// No enterprise DLP: just deterministic bounds so an authenticated uploader
// cannot dump unbounded / arbitrary content into canonical media storage.
//   - bounded file size
//   - allowed MIME families
//   - filename/path sanitization (no path separators, no control chars)

export const MAX_MEDIA_UPLOAD_BYTES = 50 * 1024 * 1024 // 50 MB

const ALLOWED_MIME_PREFIXES = [
  'image/',
  'video/',
  'audio/',
  'text/',
  'application/pdf',
  'application/msword',
  'application/rtf',
  'application/vnd.',
  'application/x-www-form-urlencoded',
]

export function isAllowedMediaMime(mime: string): boolean {
  const lower = (mime || '').toLowerCase().trim()
  return ALLOWED_MIME_PREFIXES.some((prefix) => lower.startsWith(prefix))
}

/** Strip path separators, control chars, and over-long names. Never empty. */
export function sanitizeUploadFilename(name: string): string {
  const base = (name || '').split(/[\\/]/).pop()?.trim() ?? ''
  const cleaned = base.replace(/[\x00-\x1f\x7f]/g, '').replace(/\.{2,}/g, '.')
  return cleaned.slice(0, 255) || 'upload'
}

export type UploadValidation =
  | { ok: true }
  | { ok: false; error: string }

export function validateMediaUpload(file: {
  size: number
  type: string
  name: string
}): UploadValidation {
  if (!file.size || file.size <= 0) {
    return { ok: false, error: 'Empty file.' }
  }
  if (file.size > MAX_MEDIA_UPLOAD_BYTES) {
    return { ok: false, error: 'File is too large (max 50 MB).' }
  }
  if (!isAllowedMediaMime(file.type)) {
    return { ok: false, error: 'File type is not allowed.' }
  }
  return { ok: true }
}
