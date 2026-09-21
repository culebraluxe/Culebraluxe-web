import { RustUiHost } from '@/components/rust-ui/host'

// ---------------------------------------------------------------------------
// CONVERTED TO RUST (screen: decision-analysis).
//
// What this route rendered now lives in rust/ui/src/view.rs, fed by the rows route. The route itself is unchanged,
// which is what keeps every link and bookmark working.
// ---------------------------------------------------------------------------

export default function Page() {

  return (
    <div className="min-h-screen bg-background">
      <RustUiHost rowsPath="/api/portal/rust-ui/rows" start="decision-analysis" />
    </div>
  )
}
