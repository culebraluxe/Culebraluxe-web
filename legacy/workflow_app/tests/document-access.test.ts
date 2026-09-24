import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

async function read(rel: string): Promise<string> {
  return readFile(new URL(`../../../${rel}`, import.meta.url), 'utf8')
}

test('property PDFs cross the Rust Vault service boundary', async () => {
  const edge = await read('app/api/media/documents/[id]/route.ts')
  const routes = await read('rust/server/src/api/routes.rs')
  const vault = await read('rust/server/src/vault/mod.rs')
  const repository = await read('rust/core/db/src/vault.rs')

  assert.ok(edge.includes('/v1/vault/public-listing-documents'))
  assert.ok(edge.includes('/v1/vault/document-bytes'))
  assert.ok(!edge.includes('@/legacy/db/'))
  assert.ok(!edge.includes('decideDocumentAccess'))
  assert.ok(routes.includes('resolve_public_guest_context(&state, &headers)'))
  assert.ok(routes.includes('.public_listing_document_bytes(&id.to_string(), &context)'))
  assert.ok(vault.includes('"vault.publicListingDocument.read"'))
  assert.ok(repository.includes("pm.role = 'document'"))
  assert.ok(repository.includes('p.is_published is distinct from true'))
  assert.ok(repository.includes('p.is_active_listing is distinct from true'))
  assert.ok(repository.includes('p.archived_at is not null'))
  assert.ok(repository.includes('td.signed_media_id = m.id'))
  assert.ok(repository.includes('td.signed_audit_media_id = m.id'))
})
