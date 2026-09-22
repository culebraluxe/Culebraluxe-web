import { RustUiHost } from '@/components/rust-ui/host'

// ---------------------------------------------------------------------------
// THE HOMEPAGE, RENDERED BY RUST.
//
// The structure is ported from the components this page used to compose - `hero.tsx`, `services.tsx`, `culture.tsx` -
// into `rust/ui/src/view.rs`, on the same design tokens and the same classes. Its content does not arrive as rows: a
// page is not a list, so it reads blocks (hero, buyers, sellers, culture, about, contact and the listing cards) from
// `/api/rust-ui/public-page`, built from the same two reads this page made.
//
// SECTIONS STILL TO PORT, in the order they appear below the hero: FeaturedProperties, HomeProperties, About, Contact,
// and the site footer. They are not on this page yet; the port is going in from the top so each step can be looked at.
// ---------------------------------------------------------------------------

export default function Page() {
  return (
    <div className="min-h-screen bg-background">
      <RustUiHost
        rowsPath="/api/rust-ui/public-rows"
        pagePath="/api/rust-ui/public-page"
        start="site-home"
      />
    </div>
  )
}

