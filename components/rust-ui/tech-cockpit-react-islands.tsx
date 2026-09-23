'use client'

import { useEffect, useRef } from 'react'
import { createRoot, type Root } from 'react-dom/client'

import { moveStoryBucketAction } from '@/app/portal/tech/actions'
import {
  StoryKanbanBoard,
  type StoryKanbanCard,
  type StoryKanbanColumn,
} from '@/components/portal/tech/story-kanban-board'

type SorterPayload = {
  cards: StoryKanbanCard[]
  columns: StoryKanbanColumn[]
}

type MountedIsland = {
  target: Element
  root: Root
  payloadRaw: string
}

function safeUnmount(root: Root) {
  try {
    root.unmount()
  } catch {
    // The outer portal may already have removed the slot during route teardown.
  }
}

function deferUnmount(root: Root) {
  queueMicrotask(() => safeUnmount(root))
}

function dispatchTechIntent(intent: Record<string, string>) {
  const bridge = document.getElementById('tech-cockpit-island-bridge')
  if (!(bridge instanceof HTMLButtonElement)) return
  bridge.setAttribute('data-intent', JSON.stringify(intent))
  bridge.click()
}

function SorterIsland({ payload }: { payload: SorterPayload }) {
  return (
    <StoryKanbanBoard
      cards={payload.cards}
      columns={payload.columns}
      onMove={async (cardId, from, to) => {
        const result = await moveStoryBucketAction(cardId, from, to)
        // Match the proven Cockpit behavior: successful drags stay local until the periodic canonical refresh.
        // That avoids the vendor-store reinitialization that used to bounce cards backwards after a drop.
        if (!result.ok) dispatchTechIntent({ kind: 'refresh' })
        return result.ok
          ? { ...result, note: result.note ?? cardId + ' -> ' + to }
          : result
      }}
      onResync={() => dispatchTechIntent({ kind: 'refresh' })}
    />
  )
}

export function TechCockpitReactIslands() {
  const mountedRef = useRef<MountedIsland | null>(null)

  useEffect(() => {
    const host = document.getElementById('rust-ui')
    if (!host) return

    let frame = 0
    const scan = () => {
      frame = 0
      const target = host.querySelector('#tech-sorter-island')
      const current = mountedRef.current

      if (!target) {
        if (current) {
          deferUnmount(current.root)
          mountedRef.current = null
        }
        return
      }

      const payloadRaw = target.getAttribute('data-tech-widget')
      if (!payloadRaw) return

      let payload: SorterPayload
      try {
        payload = JSON.parse(payloadRaw) as SorterPayload
      } catch (error) {
        console.error('[tech-cockpit-island] invalid sorter payload', error)
        return
      }

      if (current && current.target === target) {
        if (current.payloadRaw !== payloadRaw) {
          current.payloadRaw = payloadRaw
          current.root.render(<SorterIsland payload={payload} />)
        }
        return
      }

      if (current) deferUnmount(current.root)
      const root = createRoot(target)
      mountedRef.current = { target, root, payloadRaw }
      root.render(<SorterIsland payload={payload} />)
    }

    const scheduleScan = () => {
      if (frame) return
      frame = window.requestAnimationFrame(scan)
    }

    scan()
    const observer = new MutationObserver(scheduleScan)
    observer.observe(host, {
      subtree: true,
      childList: true,
      attributes: true,
      attributeFilter: ['data-tech-widget'],
    })

    // The engine moves even when the operator does not. Keep the mature Cockpit's 30-second cadence.
    const refresh = window.setInterval(() => {
      if (host.querySelector('#tech-sorter-island')) {
        dispatchTechIntent({ kind: 'refresh' })
      }
    }, 30_000)

    return () => {
      observer.disconnect()
      window.clearInterval(refresh)
      if (frame) window.cancelAnimationFrame(frame)
      const current = mountedRef.current
      mountedRef.current = null
      if (current) deferUnmount(current.root)
    }
  }, [])

  return null
}
