"use client";

import Image from "next/image";
import { useState } from "react";
import { cn } from "@/lib/utils";
import { FavoritesLink } from "@/components/property/favorites-link";

// Temporary visual-review switch. Set to false to restore the text wordmark.
const USE_IMAGE_LOGO = true;

// Public top-nav glass capsules — a restrained glass pill adapted to the
// public-site navy/gold/ivory language. Labels are fully visible at rest
// (strong ivory), shift to gold on hover/focus, and Portal reads as the
// translucent gold entry point. Readable over both solid navy and imagery.
const capsuleClass =
  "inline-flex min-h-11 items-center justify-center gap-1.5 rounded-full border border-brand-ivory/25 bg-brand-ivory/[0.08] px-4 text-[13px] font-medium uppercase tracking-[0.08em] text-brand-ivory shadow-[inset_0_1px_0_rgba(255,255,255,0.1),0_1px_2px_rgba(3,15,35,0.14)] backdrop-blur-md transition-colors duration-300 hover:border-brand-ivory/40 hover:bg-brand-ivory/[0.16] hover:text-brand-gold focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-gold/70 focus-visible:ring-offset-2 focus-visible:ring-offset-brand-navy";

const portalCapsuleClass =
  "inline-flex min-h-11 items-center justify-center gap-1.5 rounded-full border border-brand-gold/50 bg-brand-gold/[0.14] px-4 text-[13px] font-medium uppercase tracking-[0.08em] text-brand-gold shadow-[inset_0_1px_0_rgba(255,255,255,0.16),0_1px_2px_rgba(3,15,35,0.14)] backdrop-blur-md transition-colors duration-300 hover:border-brand-gold/70 hover:bg-brand-gold/[0.22] hover:text-brand-gold focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-gold/70 focus-visible:ring-offset-2 focus-visible:ring-offset-brand-navy";

const mobileCapsuleClass =
  "flex min-h-11 items-center justify-center gap-2 rounded-full border border-brand-ivory/25 bg-brand-ivory/[0.08] px-4 py-2 text-sm font-medium uppercase tracking-[0.22em] text-brand-ivory shadow-[inset_0_1px_0_rgba(255,255,255,0.1),0_1px_2px_rgba(3,15,35,0.14)] backdrop-blur-md transition-colors duration-300 hover:border-brand-ivory/40 hover:bg-brand-ivory/[0.16] hover:text-brand-gold focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-gold/70 focus-visible:ring-offset-2 focus-visible:ring-offset-brand-navy";

const mobilePortalCapsuleClass =
  "flex min-h-11 items-center justify-center gap-2 rounded-full border border-brand-gold/50 bg-brand-gold/[0.14] px-4 py-2 text-sm font-medium uppercase tracking-[0.22em] text-brand-gold shadow-[inset_0_1px_0_rgba(255,255,255,0.16),0_1px_2px_rgba(3,15,35,0.14)] backdrop-blur-md transition-colors duration-300 hover:border-brand-gold/70 hover:bg-brand-gold/[0.22] hover:text-brand-gold focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand-gold/70 focus-visible:ring-offset-2 focus-visible:ring-offset-brand-navy";

const NAV_LINKS = [
  { label: "Buyers", href: "/buyers" },
  { label: "Sellers", href: "/sellers" },
  { label: "Services", href: "/services" },
  { label: "Guide", href: "/guide" },
  { label: "About", href: "/about" },
  { label: "FAQ", href: "/faq" },
  { label: "Contact", href: "/contact" },
];

export function SiteHeader() {
  const [menuOpen, setMenuOpen] = useState(false);

  return (
    <>
      <header className="fixed inset-x-0 top-0 z-50 border-b border-brand-gold/15 bg-brand-navy py-6">
        <div className="mx-auto flex max-w-[1600px] items-center justify-between px-6 md:px-12">
          {USE_IMAGE_LOGO ? (
            <a
              href="/"
              aria-label="CulebraLuxe home"
            className="flex h-7 w-[250px] flex-none items-center"
            >
              <Image
                src="/images/culebraluxe-header-logo-test.png"
                alt="CulebraLuxe"
              width={2050}
              height={300}
              priority
              className="h-9 max-h-9 w-auto max-w-full flex-none object-contain"
              />
            </a>
          ) : (
            /* Previous text logo retained for easy rollback. */
            <a
              href="/"
              className="font-serif text-xl font-medium uppercase tracking-[0.35em] text-brand-ivory transition-colors duration-500 hover:text-[#d8c39a]"
            >
              CulebraLuxe
            </a>
          )}

          <nav
            className="hidden items-center gap-1 lg:flex"
            aria-label="Primary"
          >
            {NAV_LINKS.map((link) => (
              <a
                key={link.href}
                href={link.href}
                className={capsuleClass}
              >
                {link.label}
              </a>
            ))}
            <FavoritesLink className={capsuleClass} />
            <a
              href="/portal/dashboard"
              className={portalCapsuleClass}
            >
              Portal
            </a>
          </nav>

          <button
            type="button"
            onClick={() => setMenuOpen((v) => !v)}
            className="flex flex-col items-end gap-1.5 text-brand-ivory lg:hidden"
            aria-label={menuOpen ? "Close menu" : "Open menu"}
            aria-expanded={menuOpen}
            aria-controls="site-mobile-menu"
          >
            <span
              className={cn(
                "block h-px w-6 bg-current transition-all duration-300",
                menuOpen && "translate-y-[7px] rotate-45",
              )}
            />
            <span
              className={cn(
                "block h-px w-6 bg-current transition-all duration-300",
                menuOpen && "opacity-0",
              )}
            />
            <span
              className={cn(
                "block h-px w-6 bg-current transition-all duration-300",
                menuOpen && "-translate-y-[7px] -rotate-45",
              )}
            />
          </button>
        </div>

        {/* Mobile menu */}
        <div
          id="site-mobile-menu"
          className={cn(
            "max-h-0 overflow-hidden bg-brand-navy backdrop-blur-md transition-all duration-500 ease-out lg:hidden",
            menuOpen
              ? "max-h-[75svh] overflow-y-auto border-t border-brand-gold/25"
              : "",
          )}
        >
          <nav className="flex flex-col gap-2 px-4 py-4" aria-label="Mobile">
            {NAV_LINKS.map((link) => (
              <a
                key={link.href}
                href={link.href}
                onClick={() => setMenuOpen(false)}
                className={mobileCapsuleClass}
              >
                {link.label}
              </a>
            ))}
            <FavoritesLink
              onNavigate={() => setMenuOpen(false)}
              className={mobileCapsuleClass}
            />
            <a
              href="/portal/dashboard"
              onClick={() => setMenuOpen(false)}
              className={mobilePortalCapsuleClass}
            >
              Portal
            </a>
          </nav>
        </div>
      </header>

      <div className="h-[76px] lg:h-[92px]" aria-hidden="true" />
    </>
  );
}
