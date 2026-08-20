'use client'

import { useEffect, useRef, useState, useTransition } from 'react'

import { searchPeopleAction } from '@/app/portal/actions'
import type { PersonSearchResult } from '@/db/people'

// Bounded existing-person selector. This is operator selection, NOT identity
// resolution: it searches already-existing canonical people and lets the
// operator explicitly choose one. No fuzzy matching, ranking, suggestion, or
// person creation semantics.

function describe(person: PersonSearchResult): string {
  const parts = [person.email, person.phone, person.location].filter(
    Boolean,
  ) as string[]
  return parts.join(' · ')
}

export function PersonSelector({
  onSelect,
  selectedLabel,
  placeholder = 'Search existing people…',
}: {
  onSelect: (personId: string, label: string) => void
  selectedLabel?: string | null
  placeholder?: string
}) {
  const [query, setQuery] = useState('')
  const [results, setResults] = useState<PersonSearchResult[]>([])
  const [open, setOpen] = useState(false)
  const [searching, setSearching] = useState(false)
  const [isPending, startTransition] = useTransition()
  const containerRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    function onPointerDown(event: MouseEvent) {
      if (
        containerRef.current &&
        !containerRef.current.contains(event.target as Node)
      ) {
        setOpen(false)
      }
    }
    document.addEventListener('mousedown', onPointerDown)
    return () => document.removeEventListener('mousedown', onPointerDown)
  }, [])

  useEffect(() => {
    if (query.trim().length < 2) {
      setResults([])
      setOpen(false)
      return
    }
    setSearching(true)
    const handle = setTimeout(() => {
      startTransition(async () => {
        const found = await searchPeopleAction(query)
        setResults(found)
        setOpen(found.length > 0)
        setSearching(false)
      })
    }, 250)
    return () => clearTimeout(handle)
  }, [query])

  function choose(person: PersonSearchResult) {
    onSelect(person.id, person.displayName)
    setQuery('')
    setResults([])
    setOpen(false)
  }

  return (
    <div ref={containerRef} className="relative">
      {selectedLabel ? (
        <button
          type="button"
          onClick={() => {
            onSelect('', '')
            setQuery('')
          }}
          className="inline-flex min-h-11 items-center gap-2 rounded-sm border border-[var(--portal-border)] bg-[var(--portal-blue-pale)] px-3 text-sm font-light text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)]"
        >
          <span>{selectedLabel}</span>
          <span aria-hidden className="text-xs text-black/40">
            ✕
          </span>
        </button>
      ) : (
        <input
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onFocus={() => {
            if (results.length > 0) setOpen(true)
          }}
          placeholder={placeholder}
          autoComplete="off"
          aria-label={placeholder}
          className="block min-h-11 w-full rounded-sm border border-[var(--portal-border)] bg-white px-3 text-sm font-light text-black/70 outline-none focus:border-[var(--portal-navy-soft)]"
        />
      )}

      {open && (
        <div className="absolute z-20 mt-1 w-full overflow-hidden rounded-sm border border-[var(--portal-border)] bg-white shadow-[0_14px_36px_rgba(3,15,35,0.12)]">
          {results.length > 0 ? (
            results.map((person) => (
              <button
                key={person.id}
                type="button"
                onClick={() => choose(person)}
                className="flex w-full items-start justify-between gap-3 border-b border-[var(--portal-border)] px-4 py-3 text-left last:border-b-0 hover:bg-[var(--portal-blue-pale)]"
              >
                <span className="min-w-0">
                  <span className="block truncate text-sm font-medium text-[var(--portal-navy)]">
                    {person.displayName}
                  </span>
                  {describe(person) && (
                    <span className="mt-0.5 block truncate text-xs font-light text-black/45">
                      {describe(person)}
                    </span>
                  )}
                </span>
                <span className="shrink-0 text-[10px] font-light uppercase tracking-[0.12em] text-black/40">
                  {person.status}
                </span>
              </button>
            ))
          ) : (
            <div className="px-4 py-3 text-sm font-light text-black/40">
              {searching || isPending
                ? 'Searching…'
                : 'No matching people found.'}
            </div>
          )}
        </div>
      )}
    </div>
  )
}
