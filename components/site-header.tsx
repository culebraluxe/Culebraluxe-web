"use client";

import Image from "next/image";
import { useState } from "react";
import { cn } from "@/lib/utils";
import { FavoritesLink } from "@/components/property/favorites-link";

// Temporary visual-review switch. Set to false to restore the text wordmark.
const USE_IMAGE_LOGO = true;

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
              className="font-serif text-xl font-medium uppercase tracking-[0.35em] text-[#f8f5ec] transition-colors duration-500 hover:text-[#d8c39a]"
            >
              CulebraLuxe
            </a>
          )}

          <nav
            className="hidden h-7 items-center gap-0 md:flex"
            aria-label="Primary"
          >
            {NAV_LINKS.map((link) => (
              <a
                key={link.href}
                href={link.href}
                className="group relative flex h-12 items-center px-1 text-[18px] font-medium uppercase tracking-[0.08em] text-[#f8f5ec]/85 transition-colors duration-500 hover:text-brand-gold focus-visible:text-brand-gold focus-visible:outline-none lg:px-2"
              >
                {link.label}
                <span className="absolute -bottom-1.5 left-0 h-px w-0 bg-brand-gold transition-all duration-500 ease-out group-hover:w-full" />
              </a>
            ))}
            <FavoritesLink className="group relative flex h-12 items-center gap-1.5 px-1 text-[18px] font-medium uppercase tracking-[0.08em] text-[#f8f5ec]/85 transition-colors duration-500 hover:text-brand-gold focus-visible:text-brand-gold focus-visible:outline-none lg:px-2" />
          </nav>

          <button
            type="button"
            onClick={() => setMenuOpen((v) => !v)}
            className="flex flex-col items-end gap-1.5 text-[#f8f5ec] md:hidden"
            aria-label={menuOpen ? "Close menu" : "Open menu"}
            aria-expanded={menuOpen}
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
          className={cn(
            "overflow-hidden bg-brand-navy backdrop-blur-md transition-all duration-500 ease-out md:hidden",
            menuOpen ? "max-h-96 border-t border-brand-gold/25" : "max-h-0",
          )}
        >
          <nav className="flex flex-col px-6 py-4" aria-label="Mobile">
            {NAV_LINKS.map((link) => (
              <a
                key={link.href}
                href={link.href}
                onClick={() => setMenuOpen(false)}
                className="border-b border-[#f8f5ec]/10 py-4 text-sm font-medium uppercase tracking-[0.22em] text-[#f8f5ec]/80 transition-colors hover:text-[#d8c39a] last:border-0"
              >
                {link.label}
              </a>
            ))}
            <FavoritesLink
              onNavigate={() => setMenuOpen(false)}
              className="flex items-center gap-2 border-b border-[#f8f5ec]/10 py-4 text-sm font-medium uppercase tracking-[0.22em] text-[#f8f5ec]/80 transition-colors hover:text-[#d8c39a] last:border-0"
            />
          </nav>
        </div>
      </header>

      <div className="h-[77px]" aria-hidden="true" />
    </>
  );
}
