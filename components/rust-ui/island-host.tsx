'use client'

import { Fragment, useEffect, useState, type ReactNode } from 'react'
import { createPortal } from 'react-dom'

// ---------------------------------------------------------------------------
// THE ISLAND HOST — the Next half of `<Island>` (rust/ui/src/app/island.rs; contract in
// docs/agent/UI-SCREEN-ARCHITECTURE.md §9).
//
// Yew renders an empty node and queues MOUNT / UPDATE / UNMOUNT operations on `window.__culebraIslandOps`. This host
// drains the queue (when it starts, and whenever Rust calls `window.__culebraIslandFlush`) and renders each widget into
// its node with a React PORTAL, so the widget stays inside Next's tree and keeps App Router context. A widget's events
// go back to its screen through `emit`, as JSON.
//
// No DOM scanning, no MutationObserver, no bridge buttons: the node, its kind and its props are handed over directly.
// ---------------------------------------------------------------------------

/** Draws one kind of widget from its props, sending events to its screen with `emit`. */
export type IslandRenderer = (props: unknown, emit: (event: Record<string, unknown>) => void) => ReactNode

type IslandOp =
  | { op: 'mount'; id: number; node: Element; kind: string; props: string; emit: (json: string) => void }
  | { op: 'update'; id: number; props: string }
  | { op: 'unmount'; id: number }

type Mounted = { node: Element; kind: string; props: unknown; emit: (json: string) => void }

type IslandWindow = Window & {
  __culebraIslandOps?: IslandOp[]
  __culebraIslandFlush?: () => void
}

const reported = new Set<string>()

/** A widget failure is reported like any other portal client failure, so it is never silent. */
function report(message: string) {
  void fetch('/api/portal/client-error', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ message: `[island] ${message}`, path: window.location.pathname }),
    keepalive: true,
  }).catch((cause: unknown) => {
    console.error('[island] the failure report could not be sent', cause)
  })
}

function parse(kind: string, json: string): unknown {
  try {
    return JSON.parse(json) as unknown
  } catch (cause) {
    report(`${kind}: props are not JSON (${cause instanceof Error ? cause.message : String(cause)})`)
    return null
  }
}

function apply(current: Map<number, Mounted>, ops: IslandOp[]): Map<number, Mounted> {
  const next = new Map(current)
  for (const op of ops) {
    if (op.op === 'mount') {
      next.set(op.id, { node: op.node, kind: op.kind, props: parse(op.kind, op.props), emit: op.emit })
    } else if (op.op === 'update') {
      const mounted = next.get(op.id)
      if (mounted) next.set(op.id, { ...mounted, props: parse(mounted.kind, op.props) })
    } else {
      next.delete(op.id)
    }
  }
  return next
}

export function IslandHost({ renderers }: { renderers: Record<string, IslandRenderer> }) {
  const [mounted, setMounted] = useState<Map<number, Mounted>>(() => new Map())

  useEffect(() => {
    const host = window as IslandWindow
    const flush = () => {
      const ops = host.__culebraIslandOps ?? []
      host.__culebraIslandOps = []
      if (ops.length > 0) setMounted((current) => apply(current, ops))
    }
    host.__culebraIslandFlush = flush
    flush()
    return () => {
      if (host.__culebraIslandFlush === flush) delete host.__culebraIslandFlush
    }
  }, [])

  return (
    <>
      {[...mounted].map(([id, widget]) => {
        const render = renderers[widget.kind]
        if (!render) {
          if (!reported.has(widget.kind)) {
            reported.add(widget.kind)
            report(`no renderer for "${widget.kind}"`)
          }
          return null
        }
        const emit = (event: Record<string, unknown>) => widget.emit(JSON.stringify(event))
        return createPortal(<Fragment>{render(widget.props, emit)}</Fragment>, widget.node, String(id))
      })}
    </>
  )
}
