"use client"

import { useState } from "react"

// PORTAL-13 — small client button that copies a pre-formatted engineering packet
// to the clipboard. The text is formatted server-side (pure formatters in
// lib/storyboard-data.ts); this component only performs the clipboard write.
export function CopyButton({ text, label }: { text: string; label: string }) {
  const [copied, setCopied] = useState(false)
  return (
    <button
      type="button"
      onClick={async () => {
        try {
          await navigator.clipboard.writeText(text)
          setCopied(true)
          setTimeout(() => setCopied(false), 1400)
        } catch {
          setCopied(false)
        }
      }}
      className="inline-flex min-h-9 items-center gap-2 rounded-md border border-[var(--portal-border)] px-3 text-[11px] font-light uppercase tracking-[0.14em] text-[var(--portal-navy)] transition hover:border-[var(--portal-gold)]/60 hover:text-[var(--portal-archive)]"
    >
      {copied ? "Copied" : label}
    </button>
  )
}
