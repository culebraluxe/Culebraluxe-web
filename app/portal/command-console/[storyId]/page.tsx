import { RustUiHost } from '@/components/rust-ui/host'

// ---------------------------------------------------------------------------
// CONVERTED TO RUST (screen: console-story).
//
// What this route rendered now lives in rust/ui/src/view.rs, fed by the rows route. The route itself is unchanged,
// which is what keeps every link and bookmark working, and the storyId in the URL reaches the screen as its scope.
// ---------------------------------------------------------------------------

export default async function Page({ params }: { params: Promise<Record<'storyId', string>> }) {
  const { storyId } = await params

  return (
    <div className="min-h-screen bg-background">
      <RustUiHost rowsPath="/api/portal/rust-ui/rows" start="console-story" scope={storyId} />
    </div>
  )
}
