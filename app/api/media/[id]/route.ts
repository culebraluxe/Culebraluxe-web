import { sql } from "@/db/client"

export async function GET(
  request: Request,
  context: { params: Promise<{ id: string }> }
) {
  const { id } = await context.params

  const result = await sql`
    SELECT file_data, mime_type
    FROM media
    WHERE id = ${id}
    LIMIT 1
  `

  if (result.length === 0) {
    return new Response("Not found", { status: 404 })
  }

  const row = result[0]

  return new Response(row.file_data, {
    headers: {
      "Content-Type": row.mime_type,
      "Cache-Control": "public, max-age=3600",
    },
  })
}