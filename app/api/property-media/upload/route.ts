import { NextResponse } from 'next/server'

import { guardPortalUpload } from '@/lib/auth/portal-session'
import { sql } from '@/db/client'

export const runtime = 'nodejs'

export async function POST(request: Request) {
  try {
    // AUTH-03: authenticated Portal write — resolve the acting user and require
    // listing.write BEFORE any multipart/work. Denied callers get 401/403 and
    // never reach the inserts below.
    const guard = await guardPortalUpload('listing.write')
    if (!guard.ok) {
      return NextResponse.json(
        { error: guard.error },
        { status: guard.status },
      )
    }

    const formData = await request.formData()

    const propertyId = formData.get('propertyId')
    const role = formData.get('role')
    const altText = formData.get('altText')
    const file = formData.get('file')

    if (
      typeof propertyId !== 'string' ||
      propertyId.length === 0
    ) {
      return NextResponse.json(
        { error: 'Property is required.' },
        { status: 400 },
      )
    }

    if (
      role !== 'hero' &&
      role !== 'gallery'
    ) {
      return NextResponse.json(
        { error: 'Invalid media role.' },
        { status: 400 },
      )
    }

    if (!(file instanceof File)) {
      return NextResponse.json(
        { error: 'Image file is required.' },
        { status: 400 },
      )
    }

    if (!file.type.startsWith('image/')) {
      return NextResponse.json(
        { error: 'Only image uploads are supported.' },
        { status: 400 },
      )
    }

    const bytes = new Uint8Array(
      await file.arrayBuffer(),
    )

    const mediaRows = await sql`
      INSERT INTO media (
        file_data,
        filename,
        mime_type,
        file_size,
        alt_text,
        media_type
      )
      VALUES (
        ${bytes},
        ${file.name},
        ${file.type},
        ${file.size},
        ${
          typeof altText === 'string' &&
          altText.trim().length > 0
            ? altText.trim()
            : null
        },
        'image'
      )
      RETURNING id
    `

    const mediaId = String(mediaRows[0].id)

    if (role === 'hero') {
      await sql`
        UPDATE property_media
        SET role = 'gallery'
        WHERE property_id = ${propertyId}
          AND role = 'hero'
      `
    }

    await sql`
      INSERT INTO property_media (
        property_id,
        media_id,
        role,
        sort_order
      )
      VALUES (
        ${propertyId},
        ${mediaId},
        ${role},
        0
      )
    `

    return NextResponse.json({
      ok: true,
      mediaId,
      propertyId,
      role,
    })
  } catch (error) {
    console.error('Property media upload failed:', error)

    return NextResponse.json(
      { error: 'Property media upload failed.' },
      { status: 500 },
    )
  }
}