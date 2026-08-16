"use client"

import { useState } from "react"

type UploadResult = {
  id: string
  filename: string
  mime_type: string
  file_size: string
  error?: string
}

export default function MediaTestPage() {
  const [file, setFile] = useState<File | null>(null)
  const [result, setResult] = useState<UploadResult | null>(null)
  const [loading, setLoading] = useState(false)

  async function upload() {
    if (!file) return

    setLoading(true)
    setResult(null)

    const formData = new FormData()
    formData.append("file", file)

    const response = await fetch("/api/media/upload", {
      method: "POST",
      body: formData,
    })

    const data = await response.json()

    setResult(data)
    setLoading(false)
  }

  return (
    <div className="p-8">
      <h1 className="mb-6 text-2xl font-semibold">Media Upload Test</h1>

      <div className="flex max-w-xl flex-col gap-4">
        <input
          type="file"
          accept="image/*"
          onChange={(event) => {
            setFile(event.target.files?.[0] ?? null)
          }}
        />

        <button
          onClick={upload}
          disabled={!file || loading}
          className="w-fit rounded bg-[var(--portal-navy)] px-4 py-2 text-white disabled:opacity-50"
        >
          {loading ? "Uploading..." : "Upload Image"}
        </button>

        {result && (
          <>
            <pre className="overflow-auto rounded border p-4 text-sm">
              {JSON.stringify(result, null, 2)}
            </pre>

            {result.id && (
              <div>
                <p className="mb-2 font-medium">Image read back from Neon:</p>

                <img
                  src={`/api/media/${result.id}`}
                  alt={result.filename}
                  className="max-w-xl rounded border"
                />
              </div>
            )}
          </>
        )}
      </div>
    </div>
  )
}