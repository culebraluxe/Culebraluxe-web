import { RustUiHost } from '@/components/rust-ui/host'

// ---------------------------------------------------------------------------
// CONVERTED TO RUST (screen: site-services).
//
// What this route rendered now lives in rust/ui/src/view.rs. The route itself is unchanged, which is what keeps every
// link and bookmark working.
//
// THE PAGE ARRIVES AS BLOCKS, NOT ROWS. This screen is `components/services.tsx` — a buyers section and a sellers
// section with eyebrows, body copy, a ruled list, calls to action and a portrait image — and a page rendered from rows
// is a heading and three lines of text with none of that. `pagePath` is what makes the fetch happen at all: the screen
// asks Rust for its page, and the host answers from `/api/rust-ui/public-page`, which reads the same Neon marketing
// slots the live page read.
// ---------------------------------------------------------------------------

export default function Page() {
  return (
    <div className="min-h-screen bg-background">
      <RustUiHost
        rowsPath="/api/rust-ui/public-rows"
        pagePath="/api/rust-ui/public-page"
        start="site-services"
      />
    </div>
  )
}
