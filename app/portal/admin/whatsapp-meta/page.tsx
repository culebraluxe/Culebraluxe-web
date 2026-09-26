import { RustUi } from '@/components/rust-ui/rust-ui'

// ---------------------------------------------------------------------------
// /portal/admin/whatsapp-meta — YEW OWNS THIS SCREEN NOW.
//
// The screen is `rust/ui/src/yew_views/portal_support_whatsapp_meta.rs`. The Meta query still happens SERVER-SIDE, in the
// bridge, with the same endpoint, the same version and the same no-store behaviour as the pre-cutover page; the access token
// is read and used there and is not a field of the payload, so it cannot reach this bundle.
//
// Read-only diagnosis. Nothing on this path sends a WhatsApp message.
// ---------------------------------------------------------------------------

export default function Page() {
  return <RustUi screen="whatsapp-meta" />
}
