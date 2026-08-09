export function SiteFooter() {
  const year = new Date().getFullYear()

  return (
    <footer className="border-t border-border px-6 py-16 md:px-12">
      <div className="mx-auto max-w-[1600px]">
        <div className="flex flex-col gap-12 md:flex-row md:items-end md:justify-between">
          <div>
            <p className="font-serif text-lg font-normal uppercase tracking-[0.35em] text-foreground">
              CulebraLuxe
            </p>
            <p className="mt-4 max-w-xs text-sm font-light leading-relaxed text-muted-foreground">
              Architectural estates and beachfront residences on the island of Culebra,
              Puerto Rico.
            </p>
          </div>

          <nav
            className="flex flex-wrap gap-x-8 gap-y-3"
            aria-label="Footer"
          >
            {[
              { label: 'Buyers', href: '#buyers' },
              { label: 'Sellers', href: '#sellers' },
              { label: 'Culture', href: '#culture' },
              { label: 'About Us', href: '#about' },
              { label: 'Contact', href: '#contact' },
            ].map((link) => (
              <a
                key={link.href}
                href={link.href}
                className="text-xs font-light uppercase tracking-[0.2em] text-muted-foreground transition-colors hover:text-foreground"
              >
                {link.label}
              </a>
            ))}
          </nav>
        </div>

        <div className="mt-14 flex flex-col gap-3 border-t border-border pt-8 text-xs font-light uppercase tracking-[0.16em] text-muted-foreground md:flex-row md:justify-between">
          <p>&copy; {year} CulebraLuxe. All rights reserved.</p>
          <p>Culebra &middot; Puerto Rico</p>
        </div>
      </div>
    </footer>
  )
}
