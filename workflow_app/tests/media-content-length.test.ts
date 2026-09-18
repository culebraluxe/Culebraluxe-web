import assert from 'node:assert/strict'
import test from 'node:test'

import { buildDocumentResponse } from '../../app/api/media/documents/[id]/document-response'

// ---------------------------------------------------------------------------
// ENG-FORGE-TWO-UNIT-DOGFOOD-02 — UNIT A.
//
// The download declared Content-Length from `media.file_size`. When that column
// disagrees with the stored bytes the header lies and the stream is truncated.
// These lock the length to the bytes actually sent.
// ---------------------------------------------------------------------------

const bytes = (count: number): Uint8Array =>
  new Uint8Array(Array.from({ length: count }, (_, i) => i % 256))

test('content-length-is-derived-from-bytes-sent', () => {
  const fileData = bytes(100)
  const response = buildDocumentResponse({
    fileData,
    filename: 'lease.pdf',
    mimeType: 'application/pdf',
    download: true,
    declaredFileSize: 100,
  })

  assert.equal(response.headers.get('content-length'), '100')
  assert.equal(response.headers.get('content-type'), 'application/pdf')
  assert.equal(response.headers.get('cache-control'), 'private, max-age=0, must-revalidate')
  assert.equal(response.headers.get('x-content-type-options'), 'nosniff')
  assert.match(response.headers.get('content-disposition') ?? '', /^attachment;/)

  // A null file_size must not drop the header — the bytes still declare it.
  const noDeclared = buildDocumentResponse({
    fileData,
    filename: 'lease.pdf',
    mimeType: 'application/pdf',
    download: false,
    declaredFileSize: null,
  })
  assert.equal(noDeclared.headers.get('content-length'), '100')
  assert.match(noDeclared.headers.get('content-disposition') ?? '', /^inline;/)
})

test('file-size-mismatch-still-streams-complete', async () => {
  const fileData = bytes(100)
  const response = buildDocumentResponse({
    fileData,
    filename: 'lease.pdf',
    mimeType: 'application/pdf',
    download: false,
    // The row says 3; the bytes say 100. The bytes win, and the body is whole.
    declaredFileSize: 3,
  })

  assert.equal(response.headers.get('content-length'), '100', 'the header follows the bytes')

  const body = new Uint8Array(await response.arrayBuffer())
  assert.equal(body.byteLength, 100, 'the whole body streams')
  assert.deepEqual(body, fileData, 'the bytes are the bytes that were passed')
})
