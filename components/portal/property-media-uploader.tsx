'use client'

import { FormEvent, useState } from 'react'

type PropertyOption = {
  id: string
  name: string
  slug: string | null
  status: string
}

type PropertyMediaUploaderProps = {
  properties: PropertyOption[]
}

export function PropertyMediaUploader({
  properties,
}: PropertyMediaUploaderProps) {
  const [propertyId, setPropertyId] = useState(
    properties[0]?.id ?? '',
  )

  const [role, setRole] = useState<'hero' | 'gallery'>(
    'hero',
  )

  const [altText, setAltText] = useState('')
  const [file, setFile] = useState<File | null>(null)

  const [uploading, setUploading] = useState(false)
  const [message, setMessage] = useState<string | null>(
    null,
  )

  async function handleSubmit(
    event: FormEvent<HTMLFormElement>,
  ) {
    event.preventDefault()

    if (!propertyId) {
      setMessage('Choose a property.')
      return
    }

    if (!file) {
      setMessage('Choose an image.')
      return
    }

    setUploading(true)
    setMessage(null)

    try {
      const formData = new FormData()

      formData.append('propertyId', propertyId)
      formData.append('role', role)
      formData.append('altText', altText)
      formData.append('file', file)

      const response = await fetch(
        '/api/property-media/upload',
        {
          method: 'POST',
          body: formData,
        },
      )

      const result = await response.json()

      if (!response.ok) {
        throw new Error(
          result.error ?? 'Upload failed.',
        )
      }

      setMessage(
        `${file.name} uploaded and assigned successfully.`,
      )

      setFile(null)
      setAltText('')

      const input = document.getElementById(
        'property-media-file',
      ) as HTMLInputElement | null

      if (input) {
        input.value = ''
      }
    } catch (error) {
      setMessage(
        error instanceof Error
          ? error.message
          : 'Upload failed.',
      )
    } finally {
      setUploading(false)
    }
  }

  if (properties.length === 0) {
    return (
      <div className="border border-[var(--portal-border)] bg-[var(--portal-surface)] p-8">
        <p className="text-sm text-[var(--portal-muted)]">
          No properties are available.
        </p>
      </div>
    )
  }

  return (
    <form
      onSubmit={handleSubmit}
      className="border border-[var(--portal-border)] bg-[var(--portal-surface)] p-6 md:p-8"
    >
      <div className="grid gap-7">
        <div>
          <label
            htmlFor="property"
            className="mb-2 block text-xs uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]"
          >
            Property
          </label>

          <select
            id="property"
            value={propertyId}
            onChange={(event) =>
              setPropertyId(event.target.value)
            }
            className="h-12 w-full border border-[var(--portal-border)] bg-white px-4 text-sm text-[var(--portal-text)] outline-none"
          >
            {properties.map((property) => (
              <option
                key={property.id}
                value={property.id}
              >
                {property.name} — {property.status}
              </option>
            ))}
          </select>
        </div>

        <div>
          <label
            htmlFor="role"
            className="mb-2 block text-xs uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]"
          >
            Image role
          </label>

          <select
            id="role"
            value={role}
            onChange={(event) =>
              setRole(
                event.target.value as
                  | 'hero'
                  | 'gallery',
              )
            }
            className="h-12 w-full border border-[var(--portal-border)] bg-white px-4 text-sm text-[var(--portal-text)] outline-none"
          >
            <option value="hero">Hero</option>
            <option value="gallery">Gallery</option>
          </select>
        </div>

        <div>
          <label
            htmlFor="alt-text"
            className="mb-2 block text-xs uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]"
          >
            Alt text
          </label>

          <input
            id="alt-text"
            type="text"
            value={altText}
            onChange={(event) =>
              setAltText(event.target.value)
            }
            placeholder="Oceanfront villa overlooking Culebra"
            className="h-12 w-full border border-[var(--portal-border)] bg-white px-4 text-sm text-[var(--portal-text)] outline-none"
          />
        </div>

        <div>
          <label
            htmlFor="property-media-file"
            className="mb-2 block text-xs uppercase tracking-[0.18em] text-[var(--portal-blue-gray)]"
          >
            Image
          </label>

          <input
            id="property-media-file"
            type="file"
            accept="image/*"
            onChange={(event) =>
              setFile(
                event.target.files?.[0] ?? null,
              )
            }
            className="block w-full border border-dashed border-[var(--portal-border)] bg-[var(--portal-bg)] p-5 text-sm text-[var(--portal-muted)]"
          />

          {file && (
            <p className="mt-3 text-xs text-[var(--portal-muted)]">
              {file.name} ·{' '}
              {(file.size / 1024 / 1024).toFixed(2)} MB
            </p>
          )}
        </div>

        <div className="flex flex-col gap-4 border-t border-[var(--portal-border)] pt-6 sm:flex-row sm:items-center sm:justify-between">
          <p className="min-h-5 text-sm text-[var(--portal-muted)]">
            {message}
          </p>

          <button
            type="submit"
            disabled={uploading}
            className="inline-flex h-11 items-center justify-center bg-[var(--portal-navy)] px-7 text-xs uppercase tracking-[0.18em] text-white transition-opacity disabled:opacity-50"
          >
            {uploading
              ? 'Uploading...'
              : 'Upload & Assign'}
          </button>
        </div>
      </div>
    </form>
  )
}