'use client'

import { useMemo, useState } from 'react'

import { Panel } from '@/components/portal/panel'
import {
  bucketForEvent,
  calendarTimeLabel,
  weekBuckets,
  type CatchUpCalendarEvent,
} from '@/lib/catchup/calendar-adapter'

// ---------------------------------------------------------------------------
// CATCH-UP — calendar pane (week agenda over the normalized projection).
//
// Apple Calendar stays authoritative; this renders a normalized projection.
// The queue and calendar share the same deterministic source data. Prev/Next
// week navigation is purely presentational over the fetched projection.
// ---------------------------------------------------------------------------

const navBtn =
  'inline-flex min-h-8 items-center rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] px-2.5 text-[10px] font-medium uppercase tracking-[0.12em] text-[var(--portal-navy-soft)] transition hover:border-[var(--portal-navy)] hover:text-[var(--portal-navy)] disabled:opacity-35'

const WEEK = 7 * 24 * 60 * 60 * 1000

function eventKindTone(kind: CatchUpCalendarEvent['kind']) {
  if (kind === 'showing') return 'bg-[var(--portal-gold-pale)] text-[var(--portal-navy-soft)]'
  if (kind === 'meeting') return 'bg-[var(--portal-blue-pale)] text-[var(--portal-navy-soft)]'
  return 'bg-black/5 text-black/55'
}

export function CatchUpCalendar({ events }: { events: CatchUpCalendarEvent[] }) {
  const [anchorMs, setAnchorMs] = useState(() => Date.now())

  const buckets = useMemo(() => weekBuckets(anchorMs), [anchorMs])
  const byBucket = useMemo(() => {
    const map = new Map<string, CatchUpCalendarEvent[]>()
    for (const event of events) {
      const key = bucketForEvent(event, buckets)
      if (!key) continue
      const arr = map.get(key) ?? []
      arr.push(event)
      map.set(key, arr)
    }
    for (const arr of map.values()) {
      arr.sort((a, b) => a.startAt.localeCompare(b.startAt))
    }
    return map
  }, [events, buckets])

  const inWindow = events.filter((e) => bucketForEvent(e, buckets)).length

  return (
    <Panel compact heading="Calendar" className="flex h-full min-h-0 flex-col">
      <div className="mb-3 flex items-center justify-between gap-2">
        <div className="text-[11px] font-light text-black/45">
          Week of {buckets[0].dateLabel}
        </div>
        <div className="flex items-center gap-2">
          <button
            type="button"
            className={navBtn}
            onClick={() => setAnchorMs((a) => a - WEEK)}
          >
            ←
          </button>
          <button
            type="button"
            className={navBtn}
            onClick={() => setAnchorMs(() => Date.now())}
          >
            Today
          </button>
          <button
            type="button"
            className={navBtn}
            onClick={() => setAnchorMs((a) => a + WEEK)}
          >
            →
          </button>
        </div>
      </div>

      <div className="min-h-0 flex-1 space-y-2 overflow-y-auto">
        {inWindow === 0 ? (
          <p className="px-2 py-10 text-center text-sm font-light text-black/40">
            No scheduled events this week.
          </p>
        ) : (
          buckets.map((bucket) => {
            const dayEvents = byBucket.get(bucket.key) ?? []
            const isToday = bucket.key === new Date().toISOString().slice(0, 10)
            return (
              <div
                key={bucket.key}
                className={`rounded-[var(--portal-tab-radius)] border px-3 py-2 ${
                  isToday
                    ? 'border-[var(--portal-gold)]/50 bg-[var(--portal-gold-pale)]'
                    : 'border-[var(--portal-panel-border)] bg-white/30'
                }`}
              >
                <div className="flex items-baseline justify-between gap-2">
                  <span className="text-[10px] font-medium uppercase tracking-[0.14em] text-[var(--portal-navy-soft)]">
                    {bucket.dayLabel}
                  </span>
                  <span className="text-[10px] font-light text-black/40">
                    {bucket.dateLabel}
                  </span>
                </div>
                {dayEvents.length === 0 ? (
                  <p className="mt-1 text-xs font-light text-black/35">—</p>
                ) : (
                  <div className="mt-1 space-y-1">
                    {dayEvents.map((event) => (
                      <div
                        key={event.id}
                        className="flex items-start justify-between gap-2 rounded-sm px-1.5 py-1"
                      >
                        <div className="min-w-0">
                          <div className="truncate text-xs font-medium text-[var(--portal-navy)]">
                            {event.title}
                          </div>
                          {event.personName ? (
                            <div className="truncate text-[10px] font-light text-black/45">
                              {event.personName}
                              {event.propertyName ? ` · ${event.propertyName}` : ''}
                            </div>
                          ) : null}
                        </div>
                        <div className="flex shrink-0 items-center gap-1.5">
                          <span className="text-[10px] font-light text-black/55">
                            {calendarTimeLabel(event.startAt)}
                          </span>
                          <span
                            className={`rounded-full px-1.5 py-0.5 text-[8px] font-medium uppercase tracking-[0.08em] ${eventKindTone(event.kind)}`}
                          >
                            {event.kind}
                          </span>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            )
          })
        )}
      </div>
    </Panel>
  )
}
