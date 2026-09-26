import { RustUi } from '@/components/rust-ui/rust-ui'

// ---------------------------------------------------------------------------
// CONVERTED TO YEW (screen: runtime-record).
//
// What this route rendered now lives in rust/ui/src/view.rs, fed by the rows route. The route itself is unchanged,
// which is what keeps every link and bookmark working, and the instanceId in the URL reaches the screen as its scope.
// ---------------------------------------------------------------------------

export default async function Page({ params }: { params: Promise<Record<'instanceId', string>> }) {
  const { instanceId } = await params

  return (
    <RustUi screen="runtime-record" scope={instanceId} />
  )
}
