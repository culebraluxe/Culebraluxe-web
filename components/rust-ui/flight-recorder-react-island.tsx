'use client'

import { useEffect, useState } from 'react'
import { createPortal } from 'react-dom'

import { FlightRecorderPage } from '@/components/portal/tech/flight-recorder-console/FlightRecorderPage'
import {
  adaptFlightRecorderTransaction,
  type FlightRecorderTrace,
} from '@/lib/flight-recorder-adapter'
import type { FlightRecorderTransaction } from '@/lib/flight-recorder-contract'

type Mounted = {
  target: Element
  payload: string
  trace: FlightRecorderTrace
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
 * Yew owns instance identity, loading, errors and refresh. The console is rendered with a React portal rather than a
 * second React root: the DOM still lands in Yew's slot, but the component remains inside Next's React tree and therefore
 * retains App Router context for useRouter/usePathname/useSearchParams.
 */
export function FlightRecorderReactIsland() {
  const [mounted, setMounted] = useState<Mounted | null>(null)

  useEffect(() => {
    const host = document.getElementById('rust-ui')
    if (!host) return

    let frame = 0

    const scan = () => {
      frame = 0
      const target = host.querySelector('#flight-recorder-island')
      if (!target) {
        setMounted((current) => (current === null ? current : null))
        return
      }

      const loaded = readTrace(host)
      if (!loaded) {
        setMounted((current) => (current === null ? current : null))
        return
      }

      setMounted((current) => {
        if (current?.target === target && current.payload === loaded.raw) return current
        return { target, payload: loaded.raw, trace: loaded.trace }
      })
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

    // The console has always said "Auto Refresh · On". Yew owns the canonical refresh behind that promise.
    const refresh = window.setInterval(() => {
      if (host.querySelector('#flight-recorder-island')) requestRefresh()
    }, 30_000)

    return () => {
      observer.disconnect()
      window.clearInterval(refresh)
      if (frame) window.cancelAnimationFrame(frame)
    }
  }, [])

  if (!mounted) return null

  return createPortal(
    <FlightRecorderPage
      trace={mounted.trace}
      defaultEventId={mounted.trace.events[0]?.id}
    />,
    mounted.target,
  )
}
