// ---------------------------------------------------------------------------
// Form signing read seam.
//
// WHY THIS EXISTS: the architecture gate forbids components from importing the
// db layer directly (`no-db-import-from-components` in .dependency-cruiser.js:
// "Components must not touch the db layer directly; go through a service/lib
// seam"). The form editor surface needs the signing records for a document and
// the signer people for a form; this seam owns both reads.
//
// Server-only: the caller is a React Server Component.
// ---------------------------------------------------------------------------

export { listSignatureRequestsByDocument } from '@/db/signature-request'
export { listFormSignerPeople } from '@/db/form-signer'
