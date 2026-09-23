'use client'

import MuxPlayer from '@mux/mux-player-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { createRoot, type Root } from 'react-dom/client'

type VideoItem = {
  id: string
  role: 'video' | 'short'
  caption: string | null
  muxAssetId: string | null
  playbackId: string
  durationSeconds: string | null
  aspectRatio: string | null
}

type VideoPayload = {
  propertyId: string
  propertyName: string
  videos: VideoItem[]
}

type MountedIsland = {
  target: Element
  root: Root
  payloadRaw: string
}

function aspectRatio(value: string | null, role: 'video' | 'short') {
  if (!value) return role === 'short' ? '9 / 16' : '16 / 9'
  return value.replace(':', ' / ')
}

function durationLabel(value: string | null) {
  if (!value) return '—'
  const seconds = Number(value)
  if (!Number.isFinite(seconds)) return value
  const whole = Math.round(seconds)
  const minutes = Math.floor(whole / 60)
  const remainder = whole % 60
  return minutes > 0 ? minutes + ':' + String(remainder).padStart(2, '0') : remainder + ' sec'
}

function sleep(ms: number) {
  return new Promise((resolve) => window.setTimeout(resolve, ms))
}

function VideoWorkspace({ payload }: { payload: VideoPayload }) {
  const [activeIndex, setActiveIndex] = useState(0)
  const [uploadOpen, setUploadOpen] = useState(false)
  const [role, setRole] = useState<'video' | 'short'>('video')
  const [caption, setCaption] = useState('')
  const [file, setFile] = useState<File | null>(null)
  const [uploading, setUploading] = useState(false)
  const [message, setMessage] = useState<string | null>(null)
  const fileRef = useRef<HTMLInputElement | null>(null)

  const videos = payload.videos
  const index = videos.length === 0 ? 0 : Math.min(activeIndex, videos.length - 1)
  const active = videos[index] ?? null
  const previous = videos.length === 0 ? 0 : (index + videos.length - 1) % videos.length
  const next = videos.length === 0 ? 0 : (index + 1) % videos.length

  useEffect(() => {
    setActiveIndex(0)
  }, [payload.propertyId, payload.videos.length])

  const poster = useMemo(
    () => (active ? 'https://image.mux.com/' + active.playbackId + '/thumbnail.jpg?time=1&width=1600' : null),
    [active],
  )

  async function uploadVideo() {
    if (!file || uploading) return
    setUploading(true)
    setMessage('Creating secure Mux upload…')

    try {
      const createResponse = await fetch('/api/property-video/upload', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({
          action: 'create',
          propertyId: payload.propertyId,
          role,
          caption: caption.trim() || null,
          origin: window.location.origin,
        }),
      })
      const createBody = (await createResponse.json()) as {
        uploadId?: string
        uploadUrl?: string
        error?: string
      }
      if (!createResponse.ok || !createBody.uploadId || !createBody.uploadUrl) {
        throw new Error(createBody.error ?? 'Mux upload could not be created.')
      }

      setMessage('Uploading directly to Mux…')
      const uploadResponse = await fetch(createBody.uploadUrl, {
        method: 'PUT',
        body: file,
      })
      if (!uploadResponse.ok) {
        throw new Error('Mux file upload failed with ' + uploadResponse.status + '.')
      }

      setMessage('Mux is preparing the video…')
      for (let attempt = 0; attempt < 120; attempt += 1) {
        const finalizeResponse = await fetch('/api/property-video/upload', {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({
            action: 'finalize',
            propertyId: payload.propertyId,
            role,
            caption: caption.trim() || null,
            uploadId: createBody.uploadId,
          }),
        })
        const finalizeBody = (await finalizeResponse.json()) as {
          ok?: boolean
          status?: string
          error?: string
        }

        if (finalizeResponse.ok && finalizeBody.ok) {
          setMessage('Video uploaded and linked to ' + payload.propertyName + '.')
          setFile(null)
          setCaption('')
          setUploadOpen(false)
          if (fileRef.current) fileRef.current.value = ''
          document.getElementById('opps-video-refresh')?.click()
          return
        }

        if (finalizeResponse.status !== 202) {
          throw new Error(finalizeBody.error ?? 'Mux video could not be finalized.')
        }
        await sleep(2000)
      }

      throw new Error('Mux is still preparing the video. Refresh the tab in a few moments.')
    } catch (error) {
      setMessage(error instanceof Error ? error.message : 'Video upload failed.')
    } finally {
      setUploading(false)
    }
  }

  return (
    <div className="space-y-4 p-4">
      {active ? (
        <>
          <section className="overflow-hidden rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-[var(--portal-navy)]">
            <div className="relative mx-auto w-full max-w-[980px]" style={{ aspectRatio: aspectRatio(active.aspectRatio, active.role) }}>
              <MuxPlayer
                key={active.playbackId}
                playbackId={active.playbackId}
                poster={poster ?? undefined}
                metadata={{ video_title: active.caption ?? payload.propertyName }}
                style={{
                  position: 'absolute',
                  inset: 0,
                  display: 'block',
                  width: '100%',
                  height: '100%',
                  maxWidth: '100%',
                  maxHeight: '100%',
                }}
              />
              {videos.length > 1 ? (
                <>
                  <button
                    type="button"
                    onClick={() => setActiveIndex(previous)}
                    aria-label="Previous video"
                    className="absolute left-3 top-1/2 z-20 grid h-11 w-11 -translate-y-1/2 place-items-center rounded-full border border-white/35 bg-black/30 text-xl text-white backdrop-blur-md hover:bg-black/55"
                  >
                    ‹
                  </button>
                  <button
                    type="button"
                    onClick={() => setActiveIndex(next)}
                    aria-label="Next video"
                    className="absolute right-3 top-1/2 z-20 grid h-11 w-11 -translate-y-1/2 place-items-center rounded-full border border-white/35 bg-black/30 text-xl text-white backdrop-blur-md hover:bg-black/55"
                  >
                    ›
                  </button>
                </>
              ) : null}
            </div>
          </section>

          <div className="grid gap-3 sm:grid-cols-3">
            <div className="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35 p-3">
              <div className="text-[9px] font-semibold uppercase tracking-[0.13em] text-black/35">Type</div>
              <div className="mt-1 text-[13px] font-medium text-[var(--portal-navy)]">
                {active.role === 'short' ? 'Short film' : 'Property film'}
              </div>
            </div>
            <div className="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35 p-3">
              <div className="text-[9px] font-semibold uppercase tracking-[0.13em] text-black/35">Duration</div>
              <div className="mt-1 text-[13px] font-medium text-[var(--portal-navy)]">{durationLabel(active.durationSeconds)}</div>
            </div>
            <div className="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35 p-3">
              <div className="text-[9px] font-semibold uppercase tracking-[0.13em] text-black/35">Source</div>
              <div className="mt-1 text-[13px] font-medium text-[var(--portal-navy)]">Mux</div>
            </div>
          </div>

          {active.caption ? (
            <p className="text-[13px] font-light leading-relaxed text-black/60">{active.caption}</p>
          ) : null}

          {videos.length > 1 ? (
            <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
              {videos.map((video, videoIndex) => (
                <button
                  key={video.id}
                  type="button"
                  onClick={() => setActiveIndex(videoIndex)}
                  className={[
                    'overflow-hidden rounded-[var(--portal-tab-radius)] border text-left transition',
                    videoIndex === index
                      ? 'border-[var(--portal-gold)] bg-white/70'
                      : 'border-[var(--portal-panel-border)] bg-white/35 hover:bg-white/55',
                  ].join(' ')}
                >
                  <img
                    src={'https://image.mux.com/' + video.playbackId + '/thumbnail.jpg?time=1&width=500'}
                    alt=""
                    className="aspect-video w-full object-cover"
                  />
                  <div className="truncate px-2 py-2 text-[11px] font-medium text-[var(--portal-navy)]">
                    {video.caption ?? (video.role === 'short' ? 'Short film' : 'Property film')}
                  </div>
                </button>
              ))}
            </div>
          ) : null}
        </>
      ) : (
        <section className="grid min-h-[360px] place-items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] p-8 text-center">
          <div>
            <div className="font-serif text-2xl font-light text-white">No Mux video linked</div>
            <p className="mt-2 text-[12px] font-light text-white/55">
              Upload the first Property film directly to Mux below.
            </p>
          </div>
        </section>
      )}

      {uploadOpen ? (
        <section className="rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/35 p-4">
          <div className="text-[10px] font-semibold uppercase tracking-[0.14em] text-[var(--portal-gold-muted)]">Add video</div>
          <div className="mt-3 grid gap-3 sm:grid-cols-2">
            <label className="text-[10px] font-semibold uppercase tracking-[0.11em] text-[var(--portal-blue-gray)]">
              Video role
              <select
                value={role}
                onChange={(event) => setRole(event.target.value as 'video' | 'short')}
                disabled={uploading}
                className="mt-1.5 h-10 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 text-[13px] font-light"
              >
                <option value="video">Property film</option>
                <option value="short">Short film</option>
              </select>
            </label>
            <label className="text-[10px] font-semibold uppercase tracking-[0.11em] text-[var(--portal-blue-gray)]">
              Caption
              <input
                value={caption}
                onChange={(event) => setCaption(event.target.value)}
                disabled={uploading}
                placeholder={payload.propertyName + ' property film'}
                className="mt-1.5 h-10 w-full rounded-[var(--portal-tab-radius)] border border-[var(--portal-panel-border)] bg-white/70 px-3 text-[13px] font-light"
              />
            </label>
          </div>
          <input
            ref={fileRef}
            type="file"
            accept="video/*"
            disabled={uploading}
            onChange={(event) => setFile(event.target.files?.[0] ?? null)}
            className="mt-4 block w-full rounded-[var(--portal-tab-radius)] border border-dashed border-[var(--portal-panel-border)] bg-white/45 p-4 text-[12px] font-light text-black/55"
          />
          <div className="mt-3 flex flex-wrap items-center justify-between gap-3">
            <div className="min-w-0 text-[11px] font-light text-black/45">
              {message ?? (file ? file.name + ' · ' + (file.size / 1024 / 1024).toFixed(1) + ' MB' : 'Video uploads directly to Mux; Neon stores only Mux identity and metadata.')}
            </div>
            <button
              type="button"
              onClick={() => void uploadVideo()}
              disabled={!file || uploading}
              className="inline-flex h-10 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-semibold uppercase tracking-[0.12em] text-white disabled:opacity-40"
            >
              {uploading ? 'Uploading…' : 'Upload to Mux'}
            </button>
          </div>
        </section>
      ) : null}

      <div className="flex items-center justify-between gap-3 border-t border-[var(--portal-panel-border)] pt-4">
        <p className="text-[11px] font-light text-black/40">
          Mux handles ingest, encoding and playback; CulebraLuxe keeps the Property → Media relationship.
        </p>
        <button
          type="button"
          onClick={() => {
            setUploadOpen((value) => !value)
            setMessage(null)
          }}
          disabled={uploading}
          className="inline-flex h-10 items-center rounded-[var(--portal-tab-radius)] bg-[var(--portal-navy)] px-4 text-[10px] font-semibold uppercase tracking-[0.12em] text-white disabled:opacity-40"
        >
          {uploadOpen ? 'Close uploader' : '+ Add new video'}
        </button>
      </div>
    </div>
  )
}

export function OpsVideoReactIsland() {
  const mountedRef = useRef<MountedIsland | null>(null)

  useEffect(() => {
    const host = document.getElementById('rust-ui')
    if (!host) return

    let frame = 0
    const scan = () => {
      frame = 0
      const target = host.querySelector('#opps-video-island')
      const current = mountedRef.current

      if (!target) {
        if (current) {
          try {
            current.root.unmount()
          } catch {
            // Yew may already have detached the island node during a repaint.
          }
          mountedRef.current = null
        }
        return
      }

      const payloadRaw = target.getAttribute('data-video-widget')
      if (!payloadRaw) return

      let payload: VideoPayload
      try {
        payload = JSON.parse(payloadRaw) as VideoPayload
      } catch (error) {
        console.error('[opps-video-island] invalid payload', error)
        return
      }

      if (current && current.target === target) {
        if (current.payloadRaw !== payloadRaw) {
          current.payloadRaw = payloadRaw
          current.root.render(<VideoWorkspace payload={payload} />)
        }
        return
      }

      if (current) {
        try {
          current.root.unmount()
        } catch {
          // Yew may already have detached the island node during a repaint.
        }
      }
      const root = createRoot(target)
      mountedRef.current = { target, root, payloadRaw }
      root.render(<VideoWorkspace payload={payload} />)
    }

    const schedule = () => {
      if (frame) return
      frame = window.requestAnimationFrame(scan)
    }

    scan()
    const observer = new MutationObserver(schedule)
    observer.observe(host, {
      subtree: true,
      childList: true,
      attributes: true,
      attributeFilter: ['data-video-widget'],
    })

    return () => {
      observer.disconnect()
      if (frame) window.cancelAnimationFrame(frame)
      const current = mountedRef.current
      mountedRef.current = null
      if (current) {
        queueMicrotask(() => {
          try {
            current.root.unmount()
          } catch {
            // Yew may already have detached the island node during a repaint.
          }
        })
      }
    }
  }, [])

  return null
}
