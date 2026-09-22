import { RustUiHost } from '@/components/rust-ui/host'

// ---------------------------------------------------------------------------
// FLIPPED TO RUST (screen: site-property-detail, surface Site).
//
// The route is unchanged; the screen is not. It was a TypeScript page fetching its own data for a TypeScript
// component. It is now the Rust host, and `rust/ui/src/view.rs` paints the screen.
//
// THIS PAGE IS AN EDITORIAL PAGE AND A RECORD PAGE AT ONCE, which is the whole of why it is wired the way it is.
// It is served by the PAGE feed (`pagePath`), because a property record is not a row: it is a hero, a gallery, a
// facts card, four tabs of description, documents and video, and the two rails beside it. Flattening it into rows
// is what made the child page a fact table. And it is about ONE record, so its slug travels as `scope` — Rust puts
// it on the effect, the host puts it on the query string, and the route refuses to guess when it is missing.
//
// `pagePath` WAS MISSING HERE, and the page was broken by its absence: opening a property raised the host's own
// error — `screen "site-property-detail" asked for its page but no page feed is configured for this host` — because
// a host that is not told where pages come from has to refuse to invent a path. Every editorial route needs it.
//
// It qualified because its TypeScript body has no interaction to lose - no state, form, dialog, filter or paging
// control - so there is nothing the Rust body can fail to reproduce. Screens whose TypeScript carries behaviour stay
// in TypeScript until the Rust body has its own controls. scripts/ui-flip-readiness.mjs prints the split at any time.
// ---------------------------------------------------------------------------

export default async function Page({ params }: { params: Promise<Record<'slug', string>> }) {
  const { slug } = await params

  return (
    <div className="min-h-screen bg-background">
      <RustUiHost
        rowsPath="/api/rust-ui/public-rows"
        pagePath="/api/rust-ui/public-page"
        start="site-property-detail"
        scope={slug}
      />
    </div>
  )
}
