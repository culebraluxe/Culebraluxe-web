import { NextResponse, type NextRequest } from 'next/server'

import { guardPortalRoute } from '@/lib/auth/portal-session'
import { withApiHandler } from '@/lib/error-capture-seam'
import { rustApiAttachPropertyVideo } from '@/lib/rust-api/client'

type CreateCommand = {
  action: 'create'
  propertyId: string
  role: 'video' | 'short'
  caption?: string | null
  origin?: string | null
}

type FinalizeCommand = {
  action: 'finalize'
  propertyId: string
  role: 'video' | 'short'
  caption?: string | null
  uploadId: string
}

type Command = CreateCommand | FinalizeCommand

type MuxUpload = {
  id: string
  url?: string
  status: 'waiting' | 'asset_created' | 'errored' | 'cancelled' | 'timed_out'
  asset_id?: string
  error?: { type?: string; message?: string }
}

type MuxAsset = {
  id: string
  status: 'preparing' | 'ready' | 'errored'
  duration?: number
  aspect_ratio?: string
  playback_ids?: Array<{ id: string; policy: string }>
}

function muxCredentials() {
  const tokenId = process.env.MUX_TOKEN_ID?.trim()
  const tokenSecret = process.env.MUX_TOKEN_SECRET?.trim()
  if (!tokenId || !tokenSecret) return null
  return {
    authorization: 'Basic ' + Buffer.from(tokenId + ':' + tokenSecret).toString('base64'),
  }
}

async function muxFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const credentials = muxCredentials()
  if (!credentials) {
    throw new Error('Mux API is not configured. Set MUX_TOKEN_ID and MUX_TOKEN_SECRET.')
  }
  const response = await fetch('https://api.mux.com/video/v1' + path, {
    ...init,
    headers: {
      authorization: credentials.authorization,
      ...(init?.body ? { 'content-type': 'application/json' } : {}),
      ...(init?.headers ?? {}),
    },
    cache: 'no-store',
  })
  const payload = (await response.json()) as {
    data?: T
    error?: { messages?: string[]; message?: string }
  }
  if (!response.ok || !payload.data) {
    throw new Error(
      payload.error?.messages?.join('; ') ??
        payload.error?.message ??
        ('Mux API request failed with ' + response.status),
    )
  }
  return payload.data
}

async function POSTHandler(req: NextRequest): Promise<Response> {
  const guard = await guardPortalRoute('listing.write')
  if (!guard.ok) {
    return NextResponse.json({ error: guard.error }, { status: guard.status })
  }

  const command = (await req.json()) as Command
  if (!command.propertyId?.trim() || !['video', 'short'].includes(command.role)) {
    return NextResponse.json({ error: 'Property and video role are required.' }, { status: 400 })
  }

  if (command.action === 'create') {
    if (!muxCredentials()) {
      return NextResponse.json(
        { error: 'Mux API is not configured. Add MUX_TOKEN_ID and MUX_TOKEN_SECRET.' },
        { status: 503 },
      )
    }
    const origin = command.origin?.trim() || req.nextUrl.origin
    const upload = await muxFetch<MuxUpload>('/uploads', {
      method: 'POST',
      body: JSON.stringify({
        cors_origin: origin,
        new_asset_settings: {
          playback_policies: ['public'],
          video_quality: 'basic',
        },
      }),
    })
    return NextResponse.json({ uploadId: upload.id, uploadUrl: upload.url })
  }

  if (command.action !== 'finalize' || !command.uploadId?.trim()) {
    return NextResponse.json({ error: 'Upload id is required.' }, { status: 400 })
  }

  const upload = await muxFetch<MuxUpload>('/uploads/' + encodeURIComponent(command.uploadId))
  if (upload.status !== 'asset_created' || !upload.asset_id) {
    if (upload.status === 'errored' || upload.status === 'cancelled' || upload.status === 'timed_out') {
      return NextResponse.json(
        { error: upload.error?.message ?? ('Mux upload ' + upload.status + '.') },
        { status: 409 },
      )
    }
    return NextResponse.json({ status: upload.status }, { status: 202 })
  }

  const asset = await muxFetch<MuxAsset>('/assets/' + encodeURIComponent(upload.asset_id))
  if (asset.status !== 'ready') {
    if (asset.status === 'errored') {
      return NextResponse.json({ error: 'Mux could not prepare this video.' }, { status: 409 })
    }
    return NextResponse.json({ status: asset.status }, { status: 202 })
  }

  const playbackId = asset.playback_ids?.find((item) => item.policy === 'public')?.id
  if (!playbackId) {
    return NextResponse.json({ error: 'Mux asset has no public playback ID.' }, { status: 409 })
  }

  const saved = await rustApiAttachPropertyVideo<{
    ok: true
    mediaId: string
    propertyId: string
    role: string
    muxAssetId: string
    muxPlaybackId: string
  }>(command.propertyId, {
    role: command.role,
    muxAssetId: asset.id,
    muxPlaybackId: playbackId,
    durationSeconds: asset.duration == null ? null : String(asset.duration),
    aspectRatio: asset.aspect_ratio ?? null,
    caption: command.caption?.trim() || null,
  })

  return NextResponse.json({ ok: true, value: saved.value })
}

export const POST = withApiHandler(
  { label: '/api/property-video/upload', route: '/api/property-video/upload' },
  POSTHandler,
)
