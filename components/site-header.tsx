'use client'

import { useEffect, useState } from 'react'
import { cn } from '@/lib/utils'

const NAV_LINKS = [
  { label: 'Buyers', href: '/buyers' },
  { label: 'Sellers', href: '/sellers' },
  { label: 'Guide', href: '/guide' },
  { label: 'About', href: '/about' },
  { label: 'FAQ', href: '/faq' },
  { label: 'Contact', href: '/contact' },
]

export function SiteHeader() {
  const [scrolled, setScrolled] = useState(false)
  const [menuOpen, setMenuOpen] = useState(false)

  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 24)
    onScroll()
    window.addEventListener('scroll', onScroll, { passive: true })
    return () => window.removeEventListener('scroll', onScroll)
  }, [])

  return (
    <header
      className={cn(
        'fixed inset-x-0 top-0 z-50 transition-all duration-700 ease-[cubic-bezier(0.22,1,0.36,1)]',
        scrolled
          ? 'bg-background/85 py-4 backdrop-blur-md border-b border-border/60'
          : 'bg-transparent py-6',
      )}
    >
      <div className="mx-auto flex max-w-[1600px] items-center justify-between px-6 md:px-12">
        <a
          href="/"
          className={cn(
            'font-serif text-xl font-normal tracking-[0.35em] uppercase transition-colors duration-500',
            scrolled ? 'text-foreground' : 'text-background',
          )}
        >
          CulebraLuxe
        </a>

        <nav className="hidden items-center gap-10 md:flex" aria-label="Primary">
          {NAV_LINKS.map((link) => (
            <a
              key={link.href}
              href={link.href}
              className={cn(
                'group relative text-xs font-light uppercase tracking-[0.22em] transition-colors duration-500',
                scrolled ? 'text-foreground/70 hover:text-foreground' : 'text-background/80 hover:text-background',
              )}
            >
              {link.label}
              <span
                className={cn(
                  'absolute -bottom-1.5 left-0 h-px w-0 transition-all duration-500 ease-out group-hover:w-full',
                  scrolled ? 'bg-accent' : 'bg-background',
                )}
              />
            </a>
          ))}
        </nav>

        <button
          type="button"
          onClick={() => setMenuOpen((v) => !v)}
          className={cn(
            'flex flex-col items-end gap-1.5 md:hidden',
            scrolled ? 'text-foreground' : 'text-background',
          )}
          aria-label={menuOpen ? 'Close menu' : 'Open menu'}
          aria-expanded={menuOpen}
        >
          <span
            className={cn(
              'block h-px w-6 bg-current transition-all duration-300',
              menuOpen && 'translate-y-[7px] rotate-45',
            )}
          />
          <span
            className={cn('block h-px w-6 bg-current transition-all duration-300', menuOpen && 'opacity-0')}
          />
          <span
            className={cn(
              'block h-px w-6 bg-current transition-all duration-300',
              menuOpen && '-translate-y-[7px] -rotate-45',
            )}
          />
        </button>
      </div>

      {/* Mobile menu */}
      <div
        className={cn(
          'overflow-hidden bg-background/95 backdrop-blur-md transition-all duration-500 ease-out md:hidden',
          menuOpen ? 'max-h-96 border-t border-border/60' : 'max-h-0',
        )}
      >
        <nav className="flex flex-col px-6 py-4" aria-label="Mobile">
          {NAV_LINKS.map((link) => (
            <a
              key={link.href}
              href={link.href}
              onClick={() => setMenuOpen(false)}
              className="border-b border-border/40 py-4 text-sm font-light uppercase tracking-[0.22em] text-foreground/80 last:border-0"
            >
              {link.label}
            </a>
          ))}
        </nav>
      </div>
    </header>
  )
}
