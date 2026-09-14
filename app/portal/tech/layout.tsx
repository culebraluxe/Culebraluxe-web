import type { Metadata } from 'next'

// ---------------------------------------------------------------------------
// TECH tree page NAMES.
//
// Nothing in this subtree declared a title, so every engineering screen's browser tab fell back to
// the public site's title ("CulebraLuxe — Culebra Caribbean Estates"). A cockpit tab was labelled
// like the marketing site — the same class of complaint as a screen that keeps the temporary name
// it was built under instead of the name it replaced: when a screen is swapped in, the OLD one
// dies and the NAME stays.
//
// With this template a page that declares its own title reads "Flight Recorder · TECH", and the
// TECH root — the cockpit — reads simply "Cockpit".
// ---------------------------------------------------------------------------

export const metadata: Metadata = {
  title: {
    default: 'Cockpit',
    template: '%s · TECH',
  },
}

export default function TechLayout({ children }: { children: React.ReactNode }) {
  return children
}
