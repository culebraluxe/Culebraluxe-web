import { NextResponse } from "next/server"

import { guardPortalUpload } from "@/lib/auth/portal-session"
import { sql } from "@/db/client"

// AUTH-03: media uploads are authenticated Portal writes. Resolve the acting
// user and require listing.write BEFORE any multipart/work — an unauthenticated
// or unauthorized caller must never reach the insert. Fail closed on any
// unexpected guard failure (500) rather than proceeding.
export async function POST(request: Request) {
  let guard
  try {
    guard = await guardPortalUpload("listing.write")
  } catch {
    return NextResponse.json(
      { error: "Media upload failed." },
      { status: 500 }
    )
  }
  if (!guard.ok) {
    return NextResponse.json({ error: guard.error }, { status: guard.status })
  }

  const formData = await request.formData()

  const file = formData.get("file")

  if (!(file instanceof File)) {
    return NextResponse.json(
      { error: "No file uploaded" },
      { status: 400 }
    )
  }

  const bytes = new Uint8Array(await file.arrayBuffer())

  const result = await sql`
    INSERT INTO media (
      file_data,
      filename,
      mime_type,
      file_size
    )
    VALUES (
      ${bytes},
      ${file.name},
      ${file.type || "application/octet-stream"},
      ${file.size}
    )
    RETURNING id, filename, mime_type, file_size
  `

  return NextResponse.json(result[0])
}
