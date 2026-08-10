'use client'

import { useState } from 'react'
import { Reveal } from '@/components/reveal'

type QA = { q: string; a: string }

export function FaqAccordion({ items }: { items: QA[] }) {
  const [open, setOpen] = useState<number | null>(0)

  return (
    <ul className="mx-auto max-w-3xl">
      {items.map((item, i) => {
        const isOpen = open === i
        return (
          <Reveal key={item.q} delay={i * 60}>
            <li className="border-b border-border">
              <button
                type="button"
                onClick={() => setOpen(isOpen ? null : i)}
                aria-expanded={isOpen}
                className="flex w-full items-start justify-between gap-8 py-7 text-left"
              >
                <span className="font-serif text-lg font-light leading-snug text-foreground md:text-xl">
                  {item.q}
                </span>
                <span
                  className="relative mt-2 inline-block h-4 w-4 shrink-0"
                  aria-hidden="true"
                >
                  <span className="absolute left-0 top-1/2 h-px w-4 -translate-y-1/2 bg-accent" />
                  <span
                    className={`absolute left-1/2 top-0 h-4 w-px -translate-x-1/2 bg-accent transition-opacity duration-300 ${
                      isOpen ? 'opacity-0' : 'opacity-100'
                    }`}
                  />
                </span>
              </button>
              <div
                className={`grid transition-all duration-400 ease-out ${
                  isOpen ? 'grid-rows-[1fr] pb-7 opacity-100' : 'grid-rows-[0fr] opacity-0'
                }`}
              >
                <div className="overflow-hidden">
                  <p className="max-w-2xl text-sm font-light leading-relaxed text-muted-foreground">
                    {item.a}
                  </p>
                </div>
              </div>
            </li>
          </Reveal>
        )
      })}
    </ul>
  )
}
