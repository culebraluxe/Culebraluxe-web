"use client"

import { useEffect, useRef, useState } from "react"

export function PdfPreview({
  formId,
  fieldValues,
  sections,
}: {
  formId: string
  fieldValues: Record<string, string>
  sections: Record<string, string>
}) {
  const payload = JSON.stringify({ fieldValues, sections })
  const activeUrl = useRef<string | null>(null)
  const [previewUrl, setPreviewUrl] = useState<string | null>(null)
  const [rendering, setRendering] = useState(true)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    const controller = new AbortController()
    const timer = window.setTimeout(() => {
      setRendering(true)
      setError(null)
      void fetch(`/portal/forms/${formId}/preview`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: payload,
        signal: controller.signal,
      })
        .then(async (response) => {
          if (!response.ok) throw new Error("Preview could not be rendered.")
          const blob = await response.blob()
          if (blob.type !== "application/pdf") {
            throw new Error("Preview did not return a PDF.")
          }
          const nextUrl = URL.createObjectURL(blob)
          if (activeUrl.current) URL.revokeObjectURL(activeUrl.current)
          activeUrl.current = nextUrl
          setPreviewUrl(`${nextUrl}#toolbar=0&navpanes=0&view=FitH`)
        })
        .catch((caught: unknown) => {
          if (controller.signal.aborted) return
          setError(
            caught instanceof Error
              ? caught.message
              : "Preview could not be rendered.",
          )
        })
        .finally(() => {
          if (!controller.signal.aborted) setRendering(false)
        })
    }, 250)

    return () => {
      window.clearTimeout(timer)
      controller.abort()
    }
  }, [formId, payload])

  useEffect(
    () => () => {
      if (activeUrl.current) URL.revokeObjectURL(activeUrl.current)
    },
    [],
  )

  return (
    <div className="relative h-full min-h-[34rem] bg-[var(--portal-blue-pale)]/55 p-3 lg:p-4">
      {previewUrl ? (
        <iframe
          title="Exact PDF preview"
          src={previewUrl}
          className="h-full min-h-[32rem] w-full rounded-sm bg-white shadow-[0_12px_36px_rgba(24,43,64,0.14)] ring-1 ring-black/[0.06]"
        />
      ) : null}
      {!previewUrl && !error ? (
        <div className="flex h-full min-h-[32rem] items-center justify-center bg-white text-sm font-light text-black/45">
          Building exact PDF preview…
        </div>
      ) : null}
      {!previewUrl && error ? (
        <div className="flex h-full min-h-[32rem] items-center justify-center bg-white px-8 text-center text-sm font-light text-[var(--portal-archive)]">
          {error}
        </div>
      ) : null}
      {previewUrl && error ? (
        <span className="absolute bottom-6 left-6 right-6 rounded-sm bg-[var(--portal-archive)]/95 px-3 py-2 text-center text-xs text-white shadow-sm">
          {error} Showing the last successful render.
        </span>
      ) : null}
      {rendering && previewUrl ? (
        <span className="absolute right-6 top-6 rounded-full bg-[var(--portal-navy)]/85 px-3 py-1 text-[10px] uppercase tracking-[0.12em] text-white shadow-sm">
          Updating PDF…
        </span>
      ) : null}
    </div>
  )
}
