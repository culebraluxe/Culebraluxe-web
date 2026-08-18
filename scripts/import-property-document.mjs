#!/usr/bin/env node

import { readFile, stat } from 'node:fs/promises'
import path from 'node:path'

import { neon } from '@neondatabase/serverless'

const CASA_LUAR_PROPERTY_ID = '40000000-0000-4000-8000-000000000001'
const CASA_LUAR_SLUG = 'casa-luar'
const DEFAULT_TITLE = 'Casa Luar Property Appraisal'

function usage() {
  return `Usage:
  node --env-file=.env.local scripts/import-property-document.mjs \\
    --property-id ${CASA_LUAR_PROPERTY_ID} \\
    --file "/absolute/path/to/appraisal.pdf" \\
    --title "${DEFAULT_TITLE}"`
}

function parseArguments(argv) {
  const options = {
    propertyId: null,
    file: null,
    title: DEFAULT_TITLE,
  }

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index]

    if (argument === '--property-id') {
      options.propertyId = argv[++index] ?? null
    } else if (argument === '--file') {
      options.file = argv[++index] ?? null
    } else if (argument === '--title') {
      options.title = argv[++index] ?? null
    } else if (argument === '--help' || argument === '-h') {
      console.log(usage())
      process.exit(0)
    } else {
      throw new Error(`Unknown argument: ${argument}\n\n${usage()}`)
    }
  }

  if (!options.propertyId || !options.file || !options.title) {
    throw new Error(`--property-id, --file, and --title are required.\n\n${usage()}`)
  }

  if (options.propertyId !== CASA_LUAR_PROPERTY_ID) {
    throw new Error('This one-off importer is restricted to Casa Luar.')
  }

  if (!path.isAbsolute(options.file)) {
    throw new Error('--file must be an absolute path.')
  }

  if (path.extname(options.file).toLowerCase() !== '.pdf') {
    throw new Error('Only PDF files are accepted by this importer.')
  }

  return options
}

function getDatabaseUrl() {
  const appEnv = process.env.APP_ENV ?? 'development'
  const databaseUrl =
    appEnv === 'production'
      ? process.env.DATABASE_URL_PROD
      : process.env.DATABASE_URL_DEV

  if (!databaseUrl) {
    throw new Error(
      `Database URL is not configured for APP_ENV="${appEnv}". ` +
        'Run with --env-file=.env.local or provide the matching environment variable.',
    )
  }

  return databaseUrl
}

async function importDocument(options) {
  const fileStats = await stat(options.file)

  if (!fileStats.isFile()) {
    throw new Error('The supplied PDF path is not a file.')
  }

  const fileData = new Uint8Array(await readFile(options.file))
  const pdfSignature = new TextDecoder('ascii').decode(fileData.slice(0, 5))

  if (pdfSignature !== '%PDF-') {
    throw new Error('The supplied file does not have a valid PDF signature.')
  }

  const filename = path.basename(options.file)
  const sql = neon(getDatabaseUrl())
  const propertyRows = await sql`
    SELECT id, name, slug
    FROM property
    WHERE id = ${options.propertyId}
      AND slug = ${CASA_LUAR_SLUG}
      AND archived_at IS NULL
    LIMIT 1
  `

  if (propertyRows.length === 0) {
    throw new Error('The active Casa Luar property record was not found.')
  }

  const duplicateRows = await sql`
    SELECT m.id, pm.property_id
    FROM property_media pm
    JOIN media m ON m.id = pm.media_id
    WHERE pm.property_id = ${options.propertyId}
      AND m.filename = ${filename}
    LIMIT 1
  `

  if (duplicateRows.length > 0) {
    throw new Error(
      `Import aborted: "${filename}" is already linked to Casa Luar as media ${duplicateRows[0].id}.`,
    )
  }

  let mediaId = null

  try {
    const mediaRows = await sql`
      INSERT INTO media (
        file_data,
        filename,
        mime_type,
        file_size,
        alt_text,
        caption,
        media_type
      )
      VALUES (
        ${fileData},
        ${filename},
        'application/pdf',
        ${fileData.byteLength},
        ${options.title},
        ${options.title},
        'document'
      )
      RETURNING id
    `

    mediaId = String(mediaRows[0].id)

    await sql`
      INSERT INTO property_media (
        property_id,
        media_id,
        role,
        sort_order
      )
      VALUES (
        ${options.propertyId},
        ${mediaId},
        'document',
        0
      )
    `
  } catch (error) {
    if (mediaId) {
      await sql`
        DELETE FROM media
        WHERE id = ${mediaId}
      `.catch(() => undefined)
    }

    throw error
  }

  console.log(`Created media id: ${mediaId}`)
  console.log(
    `Created property_media link: property_id=${options.propertyId}, media_id=${mediaId}`,
  )
  console.log(`Filename: ${filename}`)
  console.log(`MIME type: application/pdf`)
  console.log(`File size: ${fileData.byteLength}`)
  console.log('Role: document')
  console.log('Sort order: 0')
}

try {
  const options = parseArguments(process.argv.slice(2))
  await importDocument(options)
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error))
  process.exitCode = 1
}
