'use client'

import { useMemo, useState } from 'react'
import type { LineStation, LineStory, RouteIntent, UniverseBucket } from './types'
import { loadEngineeringLine } from './fixture'

const RECORDER = '/portal/tech/flight-recorder'

function recorderHref(instanceId?: string) {
  return instanceId
    ? `${RECORDER}/${encodeURIComponent(instanceId)}`
    : RECORDER
}

export function EngineeringLinePage() {
  const seed = loadEngineeringLine()
  const [stories, setStories] = useState(seed.stories)
  const [selectedId, setSelectedId] = useState(
    () => stories.find((s) => s.onBench)?.id ?? stories[0]?.id ?? null,
  )

  const selected = stories.find((s) => s.id === selectedId) ?? null
  const bench = stories.filter((s) => s.onBench)
  const lineOf = (station: LineStation) =>
    stories.filter((s) => s.line?.station === station)

  const universe = useMemo(() => {
    const take = (bucket: UniverseBucket, n = 3) => {
      const rows = stories.filter((s) => s.bucket === bucket)
      return { rows: rows.slice(0, n), total: rows.length }
    }
    return {
      open: take('open'),
      backlog: take('backlog'),
      closed: take('closed'),
      next: take('next'),
    }
  }, [stories])

  function patch(id: string, next: Partial<LineStory>) {
    setStories((prev) => prev.map((s) => (s.id === id ? { ...s, ...next } : s)))
  }

  function sendToBench(id: string) {
    patch(id, { onBench: true })
    setSelectedId(id)
  }

  function removeFromBench(id: string) {
    patch(id, { onBench: false })
  }

  function pushToLine(story: LineStory) {
    if (!story.depsClear || story.waitsFor.length) return
    patch(story.id, {
      onBench: false,
      line: { station: 'ready' },
    })
  }

  function returnToBench(id: string) {
    patch(id, { onBench: true, line: undefined })
    setSelectedId(id)
  }

  return (
    <div
      className="min-h-screen px-3 py-4 sm:px-5"
      style={{
        background:
          'radial-gradient(1200px 600px at 20% -10%, rgba(41,68,95,0.55), transparent 60%), linear-gradient(180deg, var(--portal-navy-deep-2) 0%, var(--portal-navy-deep) 100%)',
      }}
    >
      <header className="mb-4 flex flex-wrap items-end justify-between gap-3">
        <div>
          <p className="text-[10px] font-light uppercase tracking-[0.28em] text-[var(--portal-feature-eyebrow)]">
            TECH / Engineering
          </p>
          <h1 className="mt-1 font-serif text-2xl font-light text-white">
            Engineering Cockpit
          </h1>
          <p className="mt-1 text-xs font-light text-[var(--portal-on-navy)]/70">
            Universe · Bench · Line · Recorder — one screen
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          {seed.roles.map((r) => (
            <span
              key={r}
              className="rounded-full border border-white/15 px-2.5 py-0.5 text-[10px] font-light text-white/70"
            >
              {r}
            </span>
          ))}
          <a
            href={RECORDER}
            className="rounded-md border border-[var(--portal-gold)]/50 bg-[var(--portal-gold)]/15 px-3 py-1 text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-gold-soft)]"
          >
            Flight Recorder
          </a>
        </div>
      </header>

      {/* BENCH */}
      <SectionEyebrow
        label="BENCH — scope today's work"
        count={bench.length}
      />
      <div className="mb-5 grid gap-3 lg:grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)]">
        <Panel>
          <PanelHead title="Active Work Queue" note="Explicitly selected today" />
          <div className="max-h-72 space-y-1 overflow-y-auto p-2">
            {bench.length === 0 ? (
              <Empty>Universe → Send to Bench.</Empty>
            ) : (
              bench.map((s) => (
                <button
                  key={s.id}
                  type="button"
                  onClick={() => setSelectedId(s.id)}
                  className={`flex w-full items-center gap-2 rounded-md border px-2.5 py-1.5 text-left ${
                    s.id === selectedId
                      ? 'border-[var(--portal-gold)]/60 bg-[var(--portal-gold-pale)]'
                      : 'border-white/10 bg-white/[0.03] hover:border-[var(--portal-gold)]/30'
                  }`}
                >
                  <span className="w-[9.5rem] shrink-0 truncate font-mono text-[11px] text-[var(--portal-on-navy)]">
                    {s.id}
                  </span>
                  <span className="min-w-0 flex-1 truncate text-[12px] font-light text-white/75">
                    {s.title}
                  </span>
                  <span className="hidden text-[9px] uppercase tracking-[0.1em] text-white/40 sm:block">
                    {s.workstream}
                  </span>
                  <span
                    role="presentation"
                    onClick={(e) => {
                      e.stopPropagation()
                      removeFromBench(s.id)
                    }}
                    className="text-[9px] uppercase tracking-[0.12em] text-white/40 hover:text-[var(--portal-gold-soft)]"
                  >
                    Remove
                  </span>
                </button>
              ))
            )}
          </div>
        </Panel>

        <Panel>
          {selected ? (
            <ScopeSheet
              story={selected}
              onRoute={(routeIntent) => patch(selected.id, { routeIntent })}
              onPush={() => pushToLine(selected)}
              onHold={() =>
                patch(selected.id, {
                  onBench: false,
                  line: { station: 'hold', reason: 'Held on the bench' },
                })
              }
            />
          ) : (
            <Empty>Select a bench story.</Empty>
          )}
        </Panel>
      </div>

      {/* LINE */}
      <SectionEyebrow label="LINE — Forge is running" />
      <p className="mb-2 text-[10px] font-light uppercase tracking-[0.14em] text-white/35">
        PROD · execution_environment visible
      </p>
      <div className="mb-5 grid gap-2 md:grid-cols-2 xl:grid-cols-4">
        <LineCol title="READY" items={lineOf('ready')} accent="ready">
          {(s) => (
            <Ticket story={s}>
              <span className="text-[10px] text-white/50">{s.routeIntent}</span>
            </Ticket>
          )}
        </LineCol>
        <LineCol title="RUNNING" items={lineOf('running')} accent="run">
          {(s) => (
            <Ticket story={s}>
              <div className="text-[11px] font-light text-white/70">
                {s.line?.node} · widgets {s.line?.widgets} · attempt {s.line?.attempt}
              </div>
              {s.line?.children?.length ? (
                <div className="mt-1 font-mono text-[10px] text-white/45">
                  {s.line.children.map((c) => (
                    <div key={c.sha}>
                      {c.label} #{c.sha}
                    </div>
                  ))}
                </div>
              ) : null}
              <a
                href={recorderHref(s.line?.instanceId)}
                className="mt-1 inline-block text-[10px] text-[var(--portal-gold-soft)] hover:underline"
              >
                Open Recorder
              </a>
            </Ticket>
          )}
        </LineCol>
        <LineCol title="HOLD" items={lineOf('hold')} accent="hold">
          {(s) => (
            <Ticket story={s}>
              <p className="text-[12px] font-light text-white/80">
                “{s.line?.reason}”
              </p>
              <div className="mt-2 flex gap-2">
                <a
                  href={recorderHref(s.line?.instanceId)}
                  className="text-[10px] text-[var(--portal-gold-soft)] hover:underline"
                >
                  Open Recorder
                </a>
                <button
                  type="button"
                  onClick={() => returnToBench(s.id)}
                  className="text-[10px] uppercase tracking-[0.1em] text-white/50 hover:text-white"
                >
                  Return to Bench
                </button>
              </div>
            </Ticket>
          )}
        </LineCol>
        <LineCol title="DONE" items={lineOf('done')} accent="done">
          {(s) => (
            <Ticket story={s}>
              <div className="font-mono text-[11px] text-emerald-200/80">
                {s.line?.sha} · QA {s.line?.qa}
              </div>
            </Ticket>
          )}
        </LineCol>
      </div>

      {/* UNIVERSE */}
      <SectionEyebrow label="UNIVERSE — what exists" />
      <div className="grid gap-2 md:grid-cols-2">
        <UniversePane
          title="Open"
          count={universe.open.total}
          badge="HOLD / IN FLIGHT"
          rows={universe.open.rows}
          onSend={sendToBench}
        />
        <UniversePane
          title="Backlog"
          count={universe.backlog.total}
          badge="PLANNED"
          rows={universe.backlog.rows}
          onSend={sendToBench}
        />
        <UniversePane
          title="Closed"
          count={universe.closed.total}
          badge="COMPLETE"
          rows={universe.closed.rows}
          onSend={sendToBench}
        />
        <UniversePane
          title="Next Version"
          count={universe.next.total}
          badge="DEFERRED"
          rows={universe.next.rows}
          onSend={sendToBench}
        />
      </div>
    </div>
  )
}

function SectionEyebrow({ label, count }: { label: string; count?: number }) {
  return (
    <div className="mb-2 flex items-baseline gap-2">
      <h2 className="font-serif text-lg font-light text-[var(--portal-gold-soft)]">
        {label}
      </h2>
      {count != null ? (
        <span className="rounded-full border border-white/10 px-2 text-[11px] tabular-nums text-white/55">
          {count}
        </span>
      ) : null}
    </div>
  )
}

function Panel({ children }: { children: React.ReactNode }) {
  return (
    <section className="overflow-hidden rounded-[calc(var(--portal-panel-radius)-6px)] border border-white/10 bg-white/[0.03]">
      {children}
    </section>
  )
}

function PanelHead({ title, note }: { title: string; note: string }) {
  return (
    <div className="border-b border-white/10 px-4 py-3">
      <div className="text-[9px] uppercase tracking-[0.18em] text-[var(--portal-on-navy)]/60">
        {note}
      </div>
      <h3 className="mt-0.5 font-serif text-lg font-semibold text-white">{title}</h3>
    </div>
  )
}

function Empty({ children }: { children: React.ReactNode }) {
  return (
    <p className="px-4 py-10 text-center text-xs font-light italic text-white/40">
      {children}
    </p>
  )
}

function ScopeSheet({
  story,
  onRoute,
  onPush,
  onHold,
}: {
  story: LineStory
  onRoute: (r: RouteIntent) => void
  onPush: () => void
  onHold: () => void
}) {
  const blocked = !story.depsClear || story.waitsFor.length > 0
  return (
    <div className="p-4">
      <div className="text-[9px] uppercase tracking-[0.16em] text-white/40">Scope sheet</div>
      <h3 className="mt-1 font-serif text-lg text-white">
        {story.id}
      </h3>
      <p className="text-sm font-light text-white/70">{story.title}</p>
      <p className="mt-3 text-[13px] font-light leading-5 text-white/75">{story.intent || '—'}</p>
      <dl className="mt-3 space-y-2 text-[12px] font-light text-white/70">
        <div>
          <dt className="uppercase tracking-[0.12em] text-white/40">Allowed</dt>
          <dd className="font-mono text-[11px]">{story.allowedScope || '—'}</dd>
        </div>
        <div>
          <dt className="uppercase tracking-[0.12em] text-white/40">Prohibited</dt>
          <dd className="font-mono text-[11px]">{story.prohibitedScope || '—'}</dd>
        </div>
      </dl>
      <div className="mt-3 flex gap-2">
        {(['SOLO', 'SMITH', 'SPLIT'] as const).map((r) => (
          <button
            key={r}
            type="button"
            onClick={() => onRoute(r)}
            className={`rounded-md border px-2.5 py-1 text-[10px] uppercase tracking-[0.12em] ${
              story.routeIntent === r
                ? 'border-[var(--portal-gold)] bg-[var(--portal-gold)]/20 text-[var(--portal-gold-soft)]'
                : 'border-white/15 text-white/55'
            }`}
          >
            {r}
          </button>
        ))}
      </div>
      <p className="mt-2 text-[10px] font-light text-white/40">
        Lead still decides on the machine; this is launch intent.
      </p>
      <div className="mt-3 flex flex-wrap gap-2">
        <button
          type="button"
          disabled={blocked}
          onClick={onPush}
          className="rounded-md border border-[var(--portal-gold)]/50 bg-[var(--portal-gold)]/20 px-3 py-1.5 text-[10px] uppercase tracking-[0.12em] text-[var(--portal-gold-soft)] disabled:cursor-not-allowed disabled:opacity-40"
        >
          Push to Line
        </button>
        <button
          type="button"
          onClick={onHold}
          className="rounded-md border border-white/15 px-3 py-1.5 text-[10px] uppercase tracking-[0.12em] text-white/70"
        >
          Hold
        </button>
      </div>
      {blocked ? (
        <p className="mt-3 rounded-md border border-amber-400/30 bg-amber-500/10 px-3 py-2 text-[12px] text-amber-100/90">
          Cannot push — waits on {story.waitsFor.join(', ') || 'uncleared deps'}
        </p>
      ) : null}
    </div>
  )
}

function LineCol({
  title,
  items,
  accent,
  children,
}: {
  title: string
  items: LineStory[]
  accent: 'ready' | 'run' | 'hold' | 'done'
  children: (s: LineStory) => React.ReactNode
}) {
  const ring =
    accent === 'run'
      ? 'border-[var(--portal-gold)]/35'
      : accent === 'hold'
        ? 'border-rose-400/30'
        : accent === 'done'
          ? 'border-emerald-400/25'
          : 'border-white/10'
  return (
    <section className={`rounded-lg border bg-white/[0.03] ${ring}`}>
      <div className="flex items-center justify-between border-b border-white/10 px-3 py-2">
        <h3 className="text-[11px] font-medium uppercase tracking-[0.16em] text-white/70">
          {title}
        </h3>
        <span className="tabular-nums text-[11px] text-white/45">{items.length}</span>
      </div>
      <div className="space-y-2 p-2">
        {items.length === 0 ? (
          <p className="px-1 py-6 text-center text-[11px] italic text-white/30">Empty</p>
        ) : (
          items.map((s) => <div key={s.id}>{children(s)}</div>)
        )}
      </div>
    </section>
  )
}

function Ticket({ story, children }: { story: LineStory; children?: React.ReactNode }) {
  return (
    <article className="rounded-md border border-white/10 bg-black/20 px-2.5 py-2">
      <div className="font-mono text-[10px] text-[var(--portal-on-navy)]">{story.id}</div>
      <div className="text-[12px] font-light leading-4 text-white/80">{story.title}</div>
      <div className="mt-1">{children}</div>
    </article>
  )
}

function UniversePane({
  title,
  count,
  badge,
  rows,
  onSend,
}: {
  title: string
  count: number
  badge: string
  rows: LineStory[]
  onSend: (id: string) => void
}) {
  return (
    <section className="rounded-lg border border-white/10 bg-white/[0.03] p-3">
      <div className="mb-2 flex items-baseline justify-between gap-2">
        <h3 className="font-serif text-base text-white">
          {title}{' '}
          <span className="text-sm font-light text-white/45">({count})</span>
        </h3>
        <span className="text-[9px] uppercase tracking-[0.14em] text-emerald-300/70">
          {badge}
        </span>
      </div>
      <ul className="space-y-1">
        {rows.map((s) => (
          <li key={s.id} className="flex items-center gap-2 text-[12px]">
            <button
              type="button"
              onClick={() => onSend(s.id)}
              className="min-w-0 flex-1 truncate text-left"
            >
              <span className="font-mono text-[11px] text-[var(--portal-on-navy)]">
                {s.id}
              </span>{' '}
              <span className="font-light text-white/70">{s.title}</span>
            </button>
            <button
              type="button"
              onClick={() => onSend(s.id)}
              className="shrink-0 text-[9px] uppercase tracking-[0.1em] text-white/35 hover:text-[var(--portal-gold-soft)]"
            >
              Bench
            </button>
          </li>
        ))}
      </ul>
      {count > rows.length ? (
        <p className="mt-2 text-[11px] italic text-white/35">
          … and {count - rows.length} more
        </p>
      ) : null}
    </section>
  )
}
