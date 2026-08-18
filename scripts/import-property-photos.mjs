#!/usr/bin/env node

import { readFile, readdir } from 'node:fs/promises'
import path from 'node:path'

import { neon } from '@neondatabase/serverless'

const SUPPORTED_IMAGE_TYPES = new Map([
  ['.jpg', 'image/jpeg'],
  ['.jpeg', 'image/jpeg'],
  ['.png', 'image/png'],
  ['.webp', 'image/webp'],
])

const naturalCollator = new Intl.Collator('en', {
  numeric: true,
  sensitivity: 'base',
})

function naturalFilenameCompare(left, right) {
  const parseFilename = (filename) => {
    const extension = path.extname(filename)
    const stem = path.basename(filename, extension)
    const match = stem.match(/^(.*?)(?: (\d+))?$/)

    return {
      base: match?.[1] ?? stem,
      sequence: match?.[2] ? Number(match[2]) : 1,
    }
  }

  const leftParts = parseFilename(left)
  const rightParts = parseFilename(right)
  const baseComparison = naturalCollator.compare(
    leftParts.base,
    rightParts.base,
  )

  if (baseComparison !== 0) return baseComparison

  const sequenceComparison = leftParts.sequence - rightParts.sequence
  if (sequenceComparison !== 0) return sequenceComparison

  return naturalCollator.compare(left, right)
}

function usage() {
  return `Usage:
  node --env-file=.env.local scripts/import-property-photos.mjs \\
    --property-id <uuid> \\
    --dir "/absolute/path/to/photos" \\
    [--hero "filename.jpg"] \\
    [--replace] \\
    [--dry-run]

Safety:
  --dry-run  Plans the import without connecting to Neon.
  --replace  Removes only the target property's property_media links first.
             It never deletes shared media rows.`
}

function parseArguments(argv) {
  const options = {
    propertyId: null,
    directory: null,
    heroFilename: null,
    replace: false,
    dryRun: false,
  }

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index]

    if (argument === '--property-id') {
      options.propertyId = argv[++index] ?? null
    } else if (argument === '--dir') {
      options.directory = argv[++index] ?? null
    } else if (argument === '--hero') {
      options.heroFilename = argv[++index] ?? null
    } else if (argument === '--replace') {
      options.replace = true
    } else if (argument === '--dry-run') {
      options.dryRun = true
    } else if (argument === '--help' || argument === '-h') {
      console.log(usage())
      process.exit(0)
    } else {
      throw new Error(`Unknown argument: ${argument}\n\n${usage()}`)
    }
  }

  if (!options.propertyId || !options.directory) {
    throw new Error(`--property-id and --dir are required.\n\n${usage()}`)
  }

  if (!path.isAbsolute(options.directory)) {
    throw new Error('--dir must be an absolute path.')
  }

  return options
}

async function findImages(directory) {
  const entries = await readdir(directory, { withFileTypes: true })

  return entries
    .filter((entry) => {
      if (!entry.isFile()) return false
      return SUPPORTED_IMAGE_TYPES.has(path.extname(entry.name).toLowerCase())
    })
    .map((entry) => entry.name)
    .sort(naturalFilenameCompare)
}

function buildPlan(filenames, heroFilename) {
  if (filenames.length === 0) {
    throw new Error('No supported image files were found.')
  }

  const hero = heroFilename ?? filenames[0]

  if (!filenames.includes(hero)) {
    throw new Error(
      `Hero file "${hero}" was not found in the supported image set.`,
    )
  }

  const gallery = filenames.filter((filename) => filename !== hero)

  return [
    { filename: hero, role: 'hero', sortOrder: 0 },
    ...gallery.map((filename, index) => ({
      filename,
      role: 'gallery',
      sortOrder: index + 1,
    })),
  ]
}

function printPlan(options, filenames, plan) {
  console.log(`Property ID: ${options.propertyId}`)
  console.log(`Directory: ${options.directory}`)
  console.log(`Supported image files found: ${filenames.length}`)
  console.log(`Selected hero: ${plan[0].filename}`)
  console.log(`Replace existing links: ${options.replace ? 'yes' : 'no'}`)
  console.log('')
  console.log('Planned assignments:')

  for (const item of plan) {
    console.log(
      `${String(item.sortOrder).padStart(2, '0')}  ${item.role.padEnd(7)}  ${item.filename}`,
    )
  }
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

async function importPhotos(options, plan) {
  const sql = neon(getDatabaseUrl())

  const propertyRows = await sql`
    SELECT id, name, slug
    FROM property
    WHERE id = ${options.propertyId}
      AND archived_at IS NULL
    LIMIT 1
  `

  if (propertyRows.length === 0) {
    throw new Error(`Property ${options.propertyId} was not found.`)
  }

  const property = propertyRows[0]
  const existingLinks = await sql`
    SELECT count(*)::integer AS count
    FROM property_media
    WHERE property_id = ${options.propertyId}
  `
  const existingCount = Number(existingLinks[0]?.count ?? 0)

  console.log(
    `Resolved property: ${String(property.name)} (${String(property.slug ?? 'no slug')})`,
  )
  console.log(`Existing property_media links: ${existingCount}`)

  if (existingCount > 0 && !options.replace) {
    throw new Error(
      'Import aborted: the property already has media links. ' +
        'Review them first, then rerun with --replace if replacement is intended.',
    )
  }

  if (options.replace && existingCount > 0) {
    await sql`
      DELETE FROM property_media
      WHERE property_id = ${options.propertyId}
    `
    console.log(`Removed ${existingCount} existing target-property links only.`)
  }

  let successCount = 0

  for (const item of plan) {
    const filePath = path.join(options.directory, item.filename)
    const fileData = new Uint8Array(await readFile(filePath))
    const mimeType = SUPPORTED_IMAGE_TYPES.get(
      path.extname(item.filename).toLowerCase(),
    )
    const altText = `${String(property.name)} property photo`

    console.log(
      `Uploading ${item.filename} | role=${item.role} | sort_order=${item.sortOrder}`,
    )

    let mediaId = null

    try {
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
          ${fileData},
          ${item.filename},
          ${mimeType},
          ${fileData.byteLength},
          ${altText},
          'image'
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
          ${item.role},
          ${item.sortOrder}
        )
      `

      successCount += 1
      console.log(`Created media ${mediaId}`)
    } catch (error) {
      if (mediaId) {
        await sql`
          DELETE FROM media
          WHERE id = ${mediaId}
        `.catch(() => undefined)
      }

      console.error(
        `Import failed after ${successCount} successful file(s); failure count: 1.`,
      )
      throw error
    }
  }

  console.log('')
  console.log(`Import complete. Success: ${successCount}. Failures: 0.`)
}

async function main() {
  const options = parseArguments(process.argv.slice(2))
  const filenames = await findImages(options.directory)
  const plan = buildPlan(filenames, options.heroFilename)

  printPlan(options, filenames, plan)

  if (options.dryRun) {
    console.log('')
    console.log('DRY RUN ONLY — Neon was not contacted and no data was changed.')
    return
  }

  console.log('')
  await importPhotos(options, plan)
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : error)
  process.exitCode = 1
})
