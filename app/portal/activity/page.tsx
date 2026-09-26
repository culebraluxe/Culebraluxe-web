import { RustUi } from '@/components/rust-ui/rust-ui'

// ---------------------------------------------------------------------------
// /portal/activity — YEW OWNS THIS SCREEN NOW, AS A REAL SCREEN.
//
// The generic row list that used to render here is gone: the feed is its own Yew component
// (`rust/ui/src/yew_views/portal_activity.rs`) inside the portal's own Yew shell, with the layout, the channel labels,
// the entry count, the empty state and the person/deal links of `components/portal/activity-feed.tsx`. The payload comes
// from `/api/portal/rust-ui/page`, which serves the read model's fields rather than a column list.
// ---------------------------------------------------------------------------

export default function Page() {
  return <RustUi screen="activity" />
}
