'use client'

import { useMemo, useState, useTransition } from 'react'
import { useRouter } from 'next/navigation'
import type { WbsCategoryId, WbsItem, WbsProject } from '@/services/wbs'
import { WBS_CATEGORIES } from '@/services/wbs'
import {
  completeWbsItemAction,
  createWbsItemAction,
  dismissWbsItemAction,
} from '@/app/portal/wbs/actions'

const nav =
  'inline-flex items-center rounded-full px-3 py-1 text-[10px] font-light uppercase tracking-[0.14em]'

const navTone: Record<string, string> = {
  clients: 'bg-[var(--portal-gold)]/15 text-[var(--portal-gold-muted)]',
  properties: 'bg-[var(--portal-gold)]/15 text-[var(--portal-gold-muted)]',
  media: 'bg-black/5 text-black/50',
  marketing: 'bg-black/5 text-black/50',
  accounting: 'bg-black/5 text-black/50',
  management: 'bg-black/10 text-black/60',
}

function dueLabel(item: WbsItem): string {
  if (!item.dueAt) return 'no due date'
  const d = new Date(item.dueAt)
  if (Number.isNaN(d.getTime())) return item.dueAt
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
}

export function CatchUpBoard({ items, projects }: { items: WbsItem[]; projects: WbsProject[] }) {
  const router = useRouter()
  const [isPending, startTransition] = useTransition()
  const [filter, setFilter] = useState<string | null>(null)
  const [newTitle, setNewTitle] = useState('')
  const [newCategory, setNewCategory] = useState<WbsCategoryId>(WBS_CATEGORIES[0].id)

  function addFollowUp() {
    const title = newTitle.trim()
    if (!title || isPending) return
    startTransition(async () => {
      const res = await createWbsItemAction({ title, category: newCategory })
      if (res.ok) setNewTitle('')
      router.refresh()
    })
  }

  function runTransition(fn: () => Promise<unknown>) {
    startTransition(async () => {
      await fn()
      router.refresh()
    })
  }

  function itemRow(item: WbsItem) {
    return (
      <li key={item.id} className="flex items-center justify-between gap-3 py-2.5">
        <div className="min-w-0">
          <p className="truncate font-serif text-[15px] font-light text-[var(--portal-navy)]">{item.title}</p>
          <p className="text-[10px] font-light uppercase tracking-[0.12em] text-black/40">
            {item.category} · {dueLabel(item)}
            {item.owner ? ` · ${item.owner}` : ''}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          <button type="button" disabled={isPending} onClick={() => runTransition(() => completeWbsItemAction(item.id))} className="inline-flex min-h-7 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-gold)] px-2 text-[9px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy)] disabled:opacity-40">Complete</button>
          <button type="button" disabled={isPending} onClick={() => runTransition(() => dismissWbsItemAction(item.id))} className="inline-flex min-h-7 items-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-2 text-[9px] font-light uppercase tracking-[0.12em] text-black/40 hover:text-[var(--portal-archive)] disabled:opacity-40">Dismiss</button>
        </div>
      </li>
    )
  }

  const counts = useMemo(() => {
    const map = new Map<string, number>()
    for (const item of items) map.set(item.category, (map.get(item.category) ?? 0) + 1)
    return map
  }, [items])

  const visible = filter ? items.filter((i) => i.category === filter) : items

  return (
    <div className="flex flex-col gap-4">
      {/* TOP — status + GROK-style ask */}
      <div className="grid items-stretch gap-3 lg:grid-cols-[1fr_auto]">
        <div className="portal-glass-panel flex flex-col justify-center gap-1 rounded-[var(--portal-panel-radius)] px-4 py-3">
          <p className="text-[10px] font-light uppercase tracking-[0.18em] text-black/40">Catch-Up</p>
          <p className="font-serif text-lg font-light text-[var(--portal-navy)]">
            {items.length} open follow-up{items.length === 1 ? '' : 's'}
            {projects.length > 0 ? ` · ${projects.length} project${projects.length === 1 ? '' : 's'}` : ''}
          </p>
        </div>
        <div className="portal-glass-panel portal-glass-panel-lifted flex min-h-[4.5rem] min-w-[16rem] items-center gap-2 rounded-[var(--portal-panel-radius)] px-3">
          <input type="text" placeholder="Ask WBS what needs doing…" className="h-10 min-w-0 flex-1 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/60 px-3 font-serif text-[15px] font-light text-[var(--portal-navy)] outline-none placeholder:text-black/35" />
          <button type="button" disabled className="inline-flex h-10 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-3 text-[10px] font-medium uppercase tracking-[0.14em] text-white disabled:opacity-45">Go</button>
        </div>
      </div>

      <div className="grid items-start gap-4 lg:grid-cols-[16rem_1fr_minmax(15rem,20rem)]">
        {/* LEFT — projects + area nav */}
        <section className="portal-glass-panel flex flex-col gap-3 rounded-[var(--portal-panel-radius)] p-4">
          <p className="text-[10px] font-light uppercase tracking-[0.14em] text-black/40">Projects</p>
          {projects.length === 0 ? (
            <p className="text-xs font-light text-black/40">No projects yet</p>
          ) : (
            <ul className="space-y-1">
              {projects.map((p) => (
                <li key={p.id} className="text-sm font-light text-[var(--portal-navy)]">{p.name}</li>
              ))}
            </ul>
          )}
          <p className="mt-2 text-[10px] font-light uppercase tracking-[0.14em] text-black/40">Areas</p>
          <div className="flex flex-wrap gap-1.5">
            {WBS_CATEGORIES.map((c) => (
              <button
                key={c.id}
                type="button"
                onClick={() => setFilter(filter === c.id ? null : c.id)}
                className={`${nav} ${navTone[c.id] ?? ''} ${filter === c.id ? 'ring-1 ring-[var(--portal-gold)]' : ''}`}
              >
                {c.label}
                {counts.get(c.id) ? <span className="ml-1 opacity-60">{counts.get(c.id)}</span> : null}
              </button>
            ))}
          </div>
        </section>

        {/* CENTER — follow-up list */}
        <section className="portal-glass-panel flex flex-col gap-2 rounded-[var(--portal-panel-radius)] p-4">
          <p className="text-[10px] font-light uppercase tracking-[0.14em] text-black/40">
            {filter ? `Follow-ups · ${WBS_CATEGORIES.find((c) => c.id === filter)?.label ?? filter}` : 'Follow-ups'}
          </p>
          <div className="flex items-center gap-2">
            <input
              type="text"
              value={newTitle}
              onChange={(e) => setNewTitle(e.target.value)}
              onKeyDown={(e) => { if (e.key === 'Enter') addFollowUp() }}
              placeholder="New follow-up…"
              className="h-9 min-w-0 flex-1 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-2.5 text-[13px] font-light text-black/70 outline-none focus:border-[var(--portal-navy)]"
            />
            <select
              value={newCategory}
              onChange={(e) => setNewCategory(e.target.value as WbsCategoryId)}
              className="h-9 rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white px-1.5 text-[11px] font-light text-black/60 outline-none"
            >
              {WBS_CATEGORIES.map((c) => (
                <option key={c.id} value={c.id}>{c.label}</option>
              ))}
            </select>
            <button
              type="button"
              disabled={!newTitle.trim() || isPending}
              onClick={addFollowUp}
              className="inline-flex h-9 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-3 text-[10px] font-medium uppercase tracking-[0.14em] text-white disabled:opacity-40"
            >
              Add
            </button>
          </div>

          {visible.length === 0 ? (
            <p className="text-sm font-light text-black/40">Nothing due here — all clear.</p>
          ) : (
            <div className="space-y-3">
              {projects
                .filter((p) => visible.some((i) => i.projectId === p.id))
                .map((project) => (
                  <div key={project.id}>
                    <p className="mb-1 text-[11px] font-medium uppercase tracking-[0.12em] text-[var(--portal-gold-muted)]">
                      {project.name}
                    </p>
                    <ul className="divide-y divide-[var(--portal-panel-border)]">
                      {visible.filter((i) => i.projectId === project.id).map(itemRow)}
                    </ul>
                  </div>
                ))}
              {visible.some((i) => !i.projectId) ? (
                <div>
                  <p className="mb-1 text-[11px] font-medium uppercase tracking-[0.12em] text-black/40">Ad-hoc</p>
                  <ul className="divide-y divide-[var(--portal-panel-border)]">
                    {visible.filter((i) => !i.projectId).map(itemRow)}
                  </ul>
                </div>
              ) : null}
            </div>
          )}
        </section>


        {/* RIGHT — calendar region (hosts the full calendar widget) */}
        <section className="portal-glass-panel portal-glass-panel-lifted flex min-h-[60vh] flex-col rounded-[var(--portal-panel-radius)] p-4">
          <p className="text-[10px] font-light uppercase tracking-[0.14em] text-black/40">Calendar</p>
          <div className="mt-3 space-y-2">
            {[...items]
              .filter((i) => i.dueAt)
              .sort((a, b) => String(a.dueAt).localeCompare(String(b.dueAt ?? '')))
              .slice(0, 8)
              .map((item) => (
                <div key={item.id} className="rounded-[var(--portal-tab-radius)] bg-white/55 px-3 py-2">
                  <p className="text-[13px] font-light leading-5 text-black/70">{item.title}</p>
                  <p className="text-[10px] font-light text-black/40">{dueLabel(item)}</p>
                </div>
              ))}
          </div>
        </section>
      </div>
    </div>
  )
}

