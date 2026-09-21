import { RustUiHost } from '@/components/rust-ui/host'

// ---------------------------------------------------------------------------
// ACTIVITY — the first portal screen flipped to Rust.
//
// A real flip, not a preview: the route a user already has now renders the Rust screen. It was chosen because it is
// the safest kind of screen to move — a read-only list. Its TypeScript body
// (`components/portal/activity-feed.tsx`) has no state, no form, no dialog and no paging control (131 lines, zero
// interactive hooks), so there is nothing the Rust body can fail to reproduce. The feed is the same
// `getActivityFeed(50)` the old page server-fetched, now read through the rows route.
//
// Screens that carry interaction are deliberately NOT flipped: the issue queue's two-pane filter and paging and the
// projects workspace's tree, Gantt and calendar have no Rust equivalent yet, so a flip would remove them. The rule and
// the per-screen list are in docs/layers/UI.md.
// ---------------------------------------------------------------------------

export default function ActivityPage() {
  return (
    <div className="min-h-screen bg-background">
      <RustUiHost rowsPath="/api/portal/rust-ui/rows" start="activity" />
    </div>
  )
}
