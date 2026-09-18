// ---------------------------------------------------------------------------
// DOCUMENT RESPONSE — the length a document download actually has.
//
// The route used to declare `Content-Length` from `media.file_size`. When that
// column disagrees with the stored bytes the header lies, and a stream whose
// declared length is short is truncated (or the client waits for bytes that
// never arrive). The length must come from the bytes being sent, and only from
// them.
//
// This lives beside the route instead of inside it because a Next `route.ts`
// may export only HTTP methods and segment config — a named helper export there
// fails the build.
// ---------------------------------------------------------------------------

/** Build a safe Content-Disposition header, preserving the original filename for UTF-8. */
export function contentDisposition(filename: string, download: boolean): string {
  const disposition = download ? 'attachment' : 'inline'
  const safeFilename = filename
    .replace(/[\r\n]/g, '')
    .replace(/[^\x20-\x7E]/g, '_')
    .replace(/["\\]/g, '_')

  return `${disposition}; filename="${safeFilename}"; filename*=UTF-8''${encodeURIComponent(filename)}`
}

export type DocumentResponseInput = {
  fileData: Uint8Array
  filename: string | null
  mimeType: string | null
  download: boolean
  /**
   * The stored `media.file_size`, carried for the caller's benefit ONLY. It is
   * deliberately NOT the source of the Content-Length header — it is in the
   * signature so a caller cannot quietly reintroduce it as one.
   */
  declaredFileSize?: string | number | null
}

/**
 * The document response. `Content-Length` is the byte length of the body being
 * returned (`byteLength`, not `.length`: a `Uint8Array` view and a `Buffer` can
 * disagree on element count), so a mismatched `file_size` cannot truncate the
 * stream. Every other header is exactly what the route produced before.
 */
export function buildDocumentResponse(input: DocumentResponseInput): Response {
  const headers = new Headers({
    'Content-Type': String(input.mimeType ?? 'application/octet-stream'),
    'Content-Disposition': contentDisposition(
      String(input.filename ?? 'document'),
      input.download,
    ),
    'Cache-Control': 'private, max-age=0, must-revalidate',
    'X-Content-Type-Options': 'nosniff',
    // THE BYTES SENT, not the row's claim about them.
    'Content-Length': String(input.fileData.byteLength),
  })

  return new Response(input.fileData, { headers })
}
