import { NextResponse } from "next/server"
import { sql } from "@/db/client"

export async function POST(request: Request) {
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