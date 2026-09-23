'use client'

import { useEffect, useState } from 'react'
import { createPortal } from 'react-dom'

import { FramerUiLab } from '@/components/portal/tech/framer-ui-lab'
import { TypeScriptUiLab } from '@/components/portal/tech/typescript-ui-lab'

type Targets = {
  react: Element | null
  motion: Element | null
}

/**
 * Bounded renderer adapters for /portal/design-lab.
 *
 * Yew owns the route, tab choice and shell. These portals preserve the existing React component galleries without
 * giving either gallery application ownership or creating a detached React root.
 */
export function UiLabReactIslands() {
  const [targets, setTargets] = useState<Targets>({ react: null, motion: null })

  useEffect(() => {
    const host = document.getElementById('rust-ui')
    if (!host) return

    let frame = 0
    const scan = () => {
      frame = 0
      const next: Targets = {
        react: host.querySelector('#ui-lab-react-island'),
        motion: host.querySelector('#ui-lab-motion-island'),
      }
      setTargets((current) =>
        current.react === next.react && current.motion === next.motion ? current : next,
      )
    }
    const schedule = () => {
      if (frame) return
      frame = window.requestAnimationFrame(scan)
    }

    scan()
    const observer = new MutationObserver(schedule)
    observer.observe(host, { childList: true, subtree: true })

    return () => {
      observer.disconnect()
      if (frame) window.cancelAnimationFrame(frame)
    }
  }, [])

  return (
    <>
      {targets.react ? createPortal(<TypeScriptUiLab />, targets.react) : null}
      {targets.motion ? createPortal(<FramerUiLab />, targets.motion) : null}
    </>
  )
}
