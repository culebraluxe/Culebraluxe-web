'use client'

import Link from 'next/link'
import { Mail } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'

import { Panel } from '@/components/portal/panel'

// ---------------------------------------------------------------------------
// CATCH-UP — derived attention queue (server-side paged, ENG-34).
//
// Rows summarize WHY a person is here + the quickest useful action. Clicking
// the person opens them in CORE Clients. A compact EMAIL action appears when a
// usable email exists. Never duplicates the Client Card or Contact History.
// ---------------------------------------------------------------------------

type QueueItem = {
  personId: string
  displayName: string
  role: string
  status: string
  email: string | null
  phone: string | null
  lastMeaningfulContactAt: string | null
  activeDealProperty: string | null
  nextEventLabel: string | null
  reasonCode: string
  reasonLabel: string
  priority: number
}

type QueueResponse = {
  items: QueueItem[]
  total: number
  page: number
  pageSize: number
}

const PAGE_SIZE = 50

const navBtn =
  'inline-flex min-h-8 items-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-2.5 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] disabled:opacity-35'

const emailBtn =
  'inline-flex min-h-8 items-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-2.5 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)]'

function roleLabel(role: string) {
  if (role === 'both') return 'Buyer & Seller'
  return role.charAt(0).toUpperCase() + role.slice(1)
}

function statusTone(status: string) {
  if (status === 'active') return 'bg-[var(--portal-success-pale)] text-[var(--portal-success)]'
  if (status === 'new') return 'bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)]'
  return 'bg-black/5 text-black/55'
}

function lastContactLabel(iso: string | null): string {
  if (!iso) return 'No meaningful contact'
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return 'Unknown'
  return `Last contact · ${d.toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
  })}`
}

export function CatchUpQueue() {
  const [search, setSearch] = useState('')
  const [debounced, setDebounced] = useState('')
  const [page, setPage] = useState(1)
  const [data, setData] = useState<QueueResponse | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    const id = setTimeout(() => setDebounced(search), 250)
    return () => clearTimeout(id)
  }, [search])

  const load = useCallback(async (q: string, p: number) => {
    setLoading(true)
    try {
      const params = new URLSearchParams({
        search: q,
        page: String(p),
        pageSize: String(PAGE_SIZE),
      })
      const res = await fetch(`/api/portal/catch-up?${params.toString()}`)
      if (!res.ok) throw new Error(`HTTP ${res.status}`)
      const json = (await res.json()) as QueueResponse
      setData(json)
    } catch {
      setData(null)
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    setPage(1)
    void load(debounced, 1)
  }, [debounced, load])

  const items = data?.items ?? []
  const total = data?.total ?? 0
  const totalPages = Math.max(1, Math.ceil(total / PAGE_SIZE))

  return (
    <Panel compact heading="Catch-Up Queue" className="flex h-full min-h-0 flex-col">
      <div className="mb-3">
        <input
          type="search"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search people…"
          className="w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/50 px-2.5 py-1.5 text-sm font-light outline-none placeholder:text-black/35 focus:border-[var(--portal-navy)]"
        />
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        {loading && !data ? (
          <p className="px-4 py-10 text-center text-sm font-light text-black/40">Loading…</p>
        ) : items.length === 0 ? (
          <p className="px-4 py-10 text-center text-sm font-light text-black/40">
            {search ? 'No matching attention right now.' : 'No one needs attention right now.'}
          </p>
        ) : (
          <div className="space-y-2">
            {items.map((item) => (
              <div
                key={item.personId}
                className="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/40 px-3 py-2.5"
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <Link
                      href={`/portal/clients/${item.personId}`}
                      className="block truncate font-serif text-[15px] font-light text-[var(--portal-navy)] transition hover:text-[var(--portal-navy-soft)]"
                    >
                      {item.displayName}
                    </Link>
                    <div className="mt-0.5 flex flex-wrap items-center gap-1.5">
                      <span className="text-[10px] font-light uppercase tracking-[0.12em] text-black/40">
                        {roleLabel(item.role)}
                      </span>
                      <span className={`rounded-full px-1.5 py-0.5 text-[8px] font-medium uppercase tracking-[0.08em] ${statusTone(item.status)}`}>
                        {item.status}
                      </span>
                    </div>
                  </div>
                  {item.email ? (
                    <a
                      href={`mailto:${item.email}`}
                      className={emailBtn}
                      title={`Email ${item.displayName}`}
                    >
                      <Mail className="mr-1 h-3.5 w-3.5" aria-hidden /> Email
                    </a>
                  ) : null}
                </div>

                <div className="mt-1.5 text-xs font-medium leading-5 text-[var(--portal-navy)]">
                  {item.reasonLabel}
                </div>
                <div className="mt-1 text-[10px] font-light text-black/40">
                  {lastContactLabel(item.lastMeaningfulContactAt)}
                  {item.activeDealProperty ? ` · ${item.activeDealProperty}` : ''}
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="flex shrink-0 items-center justify-between gap-2 border-t border-[var(--portal-panel-border)] px-1 pt-2">
        <button
          type="button"
          disabled={page <= 1}
          onClick={() => {
            const next = page - 1
            setPage(next)
            void load(debounced, next)
          }}
          className={navBtn}
        >
          ← Prev
        </button>
        <span className="text-[10px] font-light uppercase tracking-[0.12em] text-black/40">
          Page {page} / {totalPages}
        </span>
        <button
          type="button"
          disabled={page >= totalPages}
          onClick={() => {
            const next = page + 1
            setPage(next)
            void load(debounced, next)
          }}
          className={navBtn}
        >
          Next →
        </button>
      </div>
    </Panel>
  )
}

