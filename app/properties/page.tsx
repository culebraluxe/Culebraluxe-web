import { RustUiHost } from '@/components/rust-ui/host'

// ---------------------------------------------------------------------------
// THE PUBLIC PROPERTY INDEX — the page the site exists for, rendered by Rust.
//
// There was no route here: the public read model was reachable only through `/properties/[slug]`, and nothing linked to
// an index. The screen has been in the Rust table (`site-properties`) the whole time with an empty `path`, which is
// what said so; this is the route that fills it in.
//
// The feed already existed — `site-properties` answers from the public listing read model in
// `/api/rust-ui/public-rows`, with `publicOnly: true`, so this page can only ever show inventory the site already
// publishes to anonymous visitors.
// ---------------------------------------------------------------------------

export default function PropertiesPage() {
  return (
    <div className="min-h-screen bg-background">
      <RustUiHost rowsPath="/api/rust-ui/public-rows" start="site-properties" />
    </div>
  )
}
