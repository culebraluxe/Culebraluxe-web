"use client";

import Image from "next/image";
import { useState } from "react";
import { cn } from "@/lib/utils";
import { FavoritesLink } from "@/components/property/favorites-link";

// Temporary visual-review switch. Set to false to restore the text wordmark.
const USE_IMAGE_LOGO = true;

// Public top-nav uses the SAME shared capsule as the portal header
// (.top-nav-capsule in globals.css) so the two cannot drift. The public nav has
// more items, so it uses the tighter-padding modifier; mobile uses the
// full-width modifier. Material, border, radius, colors, and states are shared.
const capsuleClass = "top-nav-capsule top-nav-capsule--tight";
const mobileCapsuleClass = "top-nav-capsule top-nav-capsule--full";

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
              className={capsuleClass}
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
              className={mobileCapsuleClass}
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
