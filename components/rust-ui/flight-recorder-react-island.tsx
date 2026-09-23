'use client'

import { useEffect, useRef } from 'react'
import { createRoot, type Root } from 'react-dom/client'

import { FlightRecorderPage } from '@/components/portal/tech/flight-recorder-console/FlightRecorderPage'
import {
  adaptFlightRecorderTransaction,
  type FlightRecorderTrace,
} from '@/lib/flight-recorder-adapter'
import type { FlightRecorderTransaction } from '@/lib/flight-recorder-contract'

type Mounted = {
  target: Element
  root: Root
  payload: string
}

function safeUnmount(root: Root) {
  try {
    root.unmount()
  } catch {
    // Yew may already have removed the slot during route teardown.
  }
}

function deferUnmount(root: Root) {
  queueMicrotask(() => safeUnmount(root))
}

function requestRefresh() {
  const bridge = document.getElementById('flight-recorder-island-bridge')
  if (!(bridge instanceof HTMLButtonElement)) return
  bridge.setAttribute('data-intent', 'refresh')
  bridge.click()
}

function readTrace(host: Element): { raw: string; trace: FlightRecorderTrace } | null {
  const payload = host.querySelector('#flight-recorder-payload')
  const raw = payload?.textContent?.trim() ?? ''
  if (!raw) return null

  try {
    const transaction = JSON.parse(raw) as FlightRecorderTransaction
    return { raw, trace: adaptFlightRecorderTransaction(transaction) }
  } catch (error) {
    console.error('[flight-recorder-island] invalid transaction payload', error)
    return null
  }
}

/**
 * Rendering-only adapter for the recorder's specialized React console.
 *
 * Yew owns instance identity, loading, errors and refresh. This adapter only converts the canonical transaction snapshot
 * into the existing console projection and mounts the mature virtualization/SVG renderer into Yew's slot.
 */
export function FlightRecorderReactIsland() {
  const mountedRef = useRef<Mounted | null>(null)

  useEffect(() => {
    const host = document.getElementById('rust-ui')
    if (!host) return

    let frame = 0

    const scan = () => {
      frame = 0
      const target = host.querySelector('#flight-recorder-island')
      const current = mountedRef.current

      if (!target) {
        if (current) {
          deferUnmount(current.root)
          mountedRef.current = null
        }
        return
      }

      const loaded = readTrace(host)
      if (!loaded) return

      if (current && current.target === target) {
        if (current.payload !== loaded.raw) {
          current.payload = loaded.raw
          current.root.render(
            <FlightRecorderPage
              trace={loaded.trace}
              defaultEventId={loaded.trace.events[0]?.id}
            />,
          )
        }
        return
      }

      if (current) deferUnmount(current.root)
      const root = createRoot(target)
      mountedRef.current = { target, root, payload: loaded.raw }
      root.render(
        <FlightRecorderPage
          trace={loaded.trace}
          defaultEventId={loaded.trace.events[0]?.id}
        />,
      )
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
      characterData: true,
    })

    // The console has always said "Auto Refresh · On". Yew now owns a real canonical refresh behind that promise.
    const refresh = window.setInterval(() => {
      if (host.querySelector('#flight-recorder-island')) requestRefresh()
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
