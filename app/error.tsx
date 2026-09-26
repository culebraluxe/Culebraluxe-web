'use client'

import { RustUi } from '@/components/rust-ui/rust-ui'

// ---------------------------------------------------------------------------
// THE ROUTE ERROR BOUNDARY. IT IS STILL A CLIENT COMPONENT BECAUSE NEXT REQUIRES ONE.
//
// Next attaches a boundary only to a client component at this path, and it hands that component the thrown `error` and a
// `reset()`. That is the framework's contract and it is the one thing on this page that cannot move.
//
// EVERYTHING ELSE IS RUST. The header, the message and the footer come from `yew_app::mount_error_in`, chosen by
// `data-rust-app="site-error"` on the container this component mounts — the same one-owner rule as every other page,
// only with an application that has no router, because the URL it renders at is the one that failed.
//
// WHY NEITHER PROP IS USED:
//   * `reset` would have to cross from React into the Rust view, which is the seam this port removes. The Rust view's
//     "Try again" is a link back to this URL, which reloads and retries. The trade is stated where it is made, in
//     `rust/ui/src/yew_app.rs` (the `ErrorView` component), and it is a full reload rather than an in-place re-render.
//   * `error` is not rendered on purpose: a stack trace is for the operator, not the reader, and it is already captured
//     durably by the error framework rather than being printed at a visitor.
export default function Error(_props: {
  error: Error & { digest?: string }
  reset: () => void
}) {
  return <RustUi error />
}
