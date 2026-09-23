import { NextResponse, type NextRequest } from 'next/server'

import { guardPortalRoute } from '@/lib/auth/portal-session'
import { withApiHandler } from '@/lib/error-capture-seam'
import {
  rustApiCreatePropertyVideoUpload,
  rustApiFinalizePropertyVideoUpload,
} from '@/lib/rust-api/client'

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

type UploadSession = {
  uploadId: string
  uploadUrl: string
}

type FinalizeResult = {
  status: string
  attached: boolean
  mediaId: string | null
  muxAssetId: string | null
  muxPlaybackId: string | null
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
    const result = await rustApiCreatePropertyVideoUpload<UploadSession>(
      command.propertyId,
      { corsOrigin: command.origin?.trim() || req.nextUrl.origin },
    )
    return NextResponse.json({
      uploadId: result.value.uploadId,
      uploadUrl: result.value.uploadUrl,
    })
  }

  if (command.action !== 'finalize' || !command.uploadId?.trim()) {
    return NextResponse.json({ error: 'Upload id is required.' }, { status: 400 })
  }

  const result = await rustApiFinalizePropertyVideoUpload<FinalizeResult>(
    command.propertyId,
    command.uploadId,
    {
      role: command.role,
      caption: command.caption?.trim() || null,
    },
  )

  if (!result.value.attached) {
    return NextResponse.json({ status: result.value.status }, { status: 202 })
  }

  return NextResponse.json({ ok: true, value: result.value })
}

export const POST = withApiHandler(
  { label: '/api/property-video/upload', route: '/api/property-video/upload' },
  POSTHandler,
)
