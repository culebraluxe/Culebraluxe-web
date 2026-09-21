import { RustUiHost } from '@/components/rust-ui/host'

// ---------------------------------------------------------------------------
// DESIGN LAB, IN THE PRIMARY PATH.
//
// This route used to render the TypeScript design lab — a catalogue of React components, which is why it was the one
// screen kept back. It now serves the Rust lab: the same route, the component vocabulary this crate owns (text field,
// dropdown, switch, tabs, paging, table, badges, empty state), each control wired to the model, with a panel printing
// the model state underneath so a control holding its own value would visibly disagree with it.
//
// The TypeScript lab's own file is untouched and still in the tree: it is the reference for what the Rust lab has to
// cover, and deleting the reference while porting from it is how a port quietly loses things.
// ---------------------------------------------------------------------------

export default function DesignLabPage() {
  return (
    <div className="min-h-screen bg-background">
      <RustUiHost rowsPath="/api/portal/rust-ui/rows" start="rust-lab" />
    </div>
  )
}
