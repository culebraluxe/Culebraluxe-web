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

const SECTION_DIRECTORY_TO_DB_SECTION = new Map([
  ['beaches', 'beaches'],
  ['water', 'water'],
  ['wildlife', 'wildlife-land'],
  ['coffee-casual', 'coffee-casual'],
  ['dining', 'dining'],
  ['getting-here', 'getting-here'],
  ['getting-around', 'getting-around'],
  ['essentials', 'essentials'],
  ['island-story', 'island-story'],
])

const FILE_TO_SLUG_OVERRIDES = new Map([
  ['coffee-casual/pandeli.jpg', 'pan-deli'],
  ['coffee-casual/pandeli2.jpg', 'pan-deli'],
  ['essentials/emergency-medical.jpg', 'emergency-medical-services'],
  ['essentials/ferreteria gonzalez.jpg', 'ferreteria-gonzalez'],
  ['essentials/milka2.jpg', 'supermercado-milka'],
  ['getting-around/chocos-bronco-rentals.jpg', 'chocosbroncorental'],
  ['getting-around/island-taxis.jpg', 'island-taxi'],
  ['getting-around/jerrys-jeep-rentals.jpg', 'jerry-jeep-rentals'],
  ['getting-here/culebra-airport2.jpg', 'culebra-airport'],
  ['getting-here/ceiba-ferry-terminal2.jpg', 'ceiba-ferry-terminal'],
  ['getting-here/luis-muoz-marn-international-airport.jpg', 'san-juan-connection'],
  ['getting-here/luis-muoz-marn- international-airport copy.png', 'san-juan-connection'],
  ['island-story/conservation.jpg', 'conservation-legacy'],
  ['island-story/conservation2.jpg', 'conservation-legacy'],
  ['island-story/culebra-1975.jpg', 'navy-departure'],
  ['island-story/pirates.jpg', 'island-life'],
  ['island-story/resistance.jpg', 'community-resistance'],
  ['island-story/spainish-period.jpg', 'spanish-period'],
  ['island-story/us-navy.jpg', 'us-navy-presence'],
  ['island-story/us-navy2.jpg', 'us-navy-presence'],
  ['water/boat-charters2.jpg', 'boat-charters'],
  ['water/boat-charters3.jpg', 'boat-charters'],
  ['water/kayaking2.jpg', 'kayaking'],
  ['water/scuba-diving.jpg', 'diving'],
  ['water/snorkeling2.jpg', 'snorkeling'],
  ['wildlife/conservation.jpg', 'conservation'],
  ['wildlife/hiking.jpg', 'hiking-and-trails'],
  ['wildlife/wildlife.jpg', 'wildlife-refuge'],
])

const INTENTIONALLY_SKIPPED = new Set([
  'coffee-casual/cafe-blue.jpg',
  'island-story/images.jpeg',
  'island-story/images-1.jpeg',
  'island-story/images-2.jpeg',
  'island-story/images-3.jpeg',
  'island-story/images-4.jpeg',
  'island-story/images-5.jpeg',
  'island-story/images-6.jpeg',
  'island-story/map.jpg',
])

const naturalCollator = new Intl.Collator('en', {
  numeric: true,
  sensitivity: 'base',
})

function usage() {
  return `Usage:
  node --env-file=.env.local scripts/import-guide-images.mjs \\
    --dir "/absolute/path/to/public/images/guide" \\
    [--replace] \\
    [--dry-run]

Safety:
  --dry-run  Does not connect to Neon. Prints the file-to-guide-item plan only.
  --replace  Removes existing guide_item_media links before importing.
             It never deletes shared media rows.

Behavior:
  - Scans guide subdirectories recursively.
  - Ignores .DS_Store and unsupported files.
  - Uses exact filename -> slug matching by default.
  - Uses explicit overrides only for known exceptions.
  - Never fuzzy-matches or guesses ambiguous filenames.
  - First image for a guide item becomes role=card; additional images become gallery.
`
}

function parseArguments(argv) {
  const options = {
    directory: null,
    replace: false,
    dryRun: false,
  }

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index]

    if (argument === '--dir') {
      options.directory = argv[++index] ?? null
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

  if (!options.directory) {
    throw new Error(`--dir is required.\n\n${usage()}`)
  }

  if (!path.isAbsolute(options.directory)) {
    throw new Error('--dir must be an absolute path.')
  }

  return options
}

async function walkImages(rootDirectory) {
  const found = []

  async function walk(currentDirectory) {
    const entries = await readdir(currentDirectory, { withFileTypes: true })

    for (const entry of entries) {
      if (entry.name === '.DS_Store') continue

      const absolutePath = path.join(currentDirectory, entry.name)

      if (entry.isDirectory()) {
        await walk(absolutePath)
        continue
      }

      if (!entry.isFile()) continue

      const extension = path.extname(entry.name).toLowerCase()
      if (!SUPPORTED_IMAGE_TYPES.has(extension)) continue

      const relativePath = path
        .relative(rootDirectory, absolutePath)
        .split(path.sep)
        .join('/')

      found.push({
        absolutePath,
        relativePath,
        filename: entry.name,
        extension,
      })
    }
  }

  await walk(rootDirectory)

  return found.sort((left, right) =>
    naturalCollator.compare(left.relativePath, right.relativePath),
  )
}

function defaultSlug(filename) {
  return path.basename(filename, path.extname(filename))
}

function buildFilePlan(files) {
  const matched = []
  const skipped = []
  const unresolved = []

  for (const file of files) {
    const parts = file.relativePath.split('/')
    const directoryName = parts[0]
    const section = SECTION_DIRECTORY_TO_DB_SECTION.get(directoryName)

    if (!section) {
      unresolved.push({
        ...file,
        reason: `unknown guide directory "${directoryName}"`,
      })
      continue
    }

    if (INTENTIONALLY_SKIPPED.has(file.relativePath)) {
      skipped.push({
        ...file,
        reason: 'intentional legacy/research skip',
      })
      continue
    }

    const overrideSlug = FILE_TO_SLUG_OVERRIDES.get(file.relativePath)
    const slug = overrideSlug ?? defaultSlug(file.filename)

    matched.push({
      ...file,
      section,
      slug,
      matchType: overrideSlug ? 'override' : 'exact',
    })
  }

  return { matched, skipped, unresolved }
}

function assignRoles(matched) {
  const bySlug = new Map()

  for (const item of matched) {
    const list = bySlug.get(item.slug) ?? []
    list.push(item)
    bySlug.set(item.slug, list)
  }

  const assignments = []

  for (const [slug, items] of bySlug) {
    items
      .sort((left, right) =>
        naturalCollator.compare(left.relativePath, right.relativePath),
      )
      .forEach((item, index) => {
        assignments.push({
          ...item,
          role: index === 0 ? 'card' : 'gallery',
          sortOrder: index,
        })
      })
  }

  return assignments.sort((left, right) => {
    const slugComparison = naturalCollator.compare(left.slug, right.slug)
    if (slugComparison !== 0) return slugComparison
    return left.sortOrder - right.sortOrder
  })
}

function printDryRun(files, assignments, skipped, unresolved) {
  console.log(`Supported image files found: ${files.length}`)
  console.log(`Mapped files: ${assignments.length}`)
  console.log(`Intentionally skipped files: ${skipped.length}`)
  console.log(`Unresolved files: ${unresolved.length}`)
  console.log('')

  console.log('MAPPED')
  for (const item of assignments) {
    console.log(
      `${item.matchType.padEnd(8)}  ${item.role.padEnd(7)}  ${item.relativePath} -> ${item.slug}`,
    )
  }

  console.log('')
  console.log('INTENTIONALLY SKIPPED')
  if (skipped.length === 0) {
    console.log('none')
  } else {
    for (const item of skipped) {
      console.log(`${item.relativePath} -> ${item.reason}`)
    }
  }

  console.log('')
  console.log('UNRESOLVED / BLOCKING')
  if (unresolved.length === 0) {
    console.log('none')
  } else {
    for (const item of unresolved) {
      console.log(`${item.relativePath} -> ${item.reason}`)
    }
  }

  console.log('')
  console.log('DRY RUN ONLY — Neon was not contacted and no data was changed.')
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

async function resolveGuideItems(sql) {
  const rows = await sql`
    SELECT id, slug, section, name
    FROM guide_item
    WHERE is_active = true
    ORDER BY section, sort_order, name
  `

  return new Map(
    rows.map((row) => [
      String(row.slug),
      {
        id: String(row.id),
        slug: String(row.slug),
        section: String(row.section),
        name: String(row.name),
      },
    ]),
  )
}

async function verifyPlanAgainstDatabase(sql, assignments, skipped, unresolved) {
  const guideItems = await resolveGuideItems(sql)
  const valid = []
  const invalid = [...unresolved]
  const matchedSlugs = new Set()

  for (const item of assignments) {
    const guideItem = guideItems.get(item.slug)

    if (!guideItem) {
      invalid.push({
        ...item,
        reason: `guide_item slug "${item.slug}" does not exist`,
      })
      continue
    }

    if (guideItem.section !== item.section) {
      invalid.push({
        ...item,
        reason:
          `section mismatch: file implies "${item.section}" but DB has "${guideItem.section}"`,
      })
      continue
    }

    matchedSlugs.add(item.slug)
    valid.push({
      ...item,
      guideItem,
    })
  }

  const missingGuideItems = [...guideItems.values()].filter(
    (item) => !matchedSlugs.has(item.slug),
  )

  return {
    valid,
    skipped,
    invalid,
    missingGuideItems,
  }
}

function printDatabaseVerification(result) {
  console.log('')
  console.log(`Verified upload assignments: ${result.valid.length}`)
  console.log(`Intentionally skipped files: ${result.skipped.length}`)
  console.log(`Rejected/unresolved assignments: ${result.invalid.length}`)
  console.log(`Active guide items with no mapped image: ${result.missingGuideItems.length}`)

  if (result.skipped.length > 0) {
    console.log('')
    console.log('INTENTIONALLY SKIPPED')
    for (const item of result.skipped) {
      console.log(`${item.relativePath} -> ${item.reason}`)
    }
  }

  if (result.invalid.length > 0) {
    console.log('')
    console.log('REJECTED / BLOCKING')
    for (const item of result.invalid) {
      console.log(`${item.relativePath} -> ${item.reason}`)
    }
  }

  if (result.missingGuideItems.length > 0) {
    console.log('')
    console.log('GUIDE ITEMS WITH NO MAPPED IMAGE')
    for (const item of result.missingGuideItems) {
      console.log(`${item.section.padEnd(18)}  ${item.slug}  (${item.name})`)
    }
  }
}

async function importGuideImages(sql, options, verified) {
  if (verified.invalid.length > 0) {
    throw new Error(
      'Import aborted because unresolved/rejected files remain. ' +
        'Resolve them before modifying Neon.',
    )
  }

  if (options.replace) {
    const deleted = await sql`
      DELETE FROM guide_item_media
      RETURNING guide_item_id
    `
    console.log(`Removed ${deleted.length} existing guide_item_media link(s).`)
  } else {
    const existing = await sql`
      SELECT count(*)::integer AS count
      FROM guide_item_media
    `
    const existingCount = Number(existing[0]?.count ?? 0)

    if (existingCount > 0) {
      throw new Error(
        `Import aborted: guide_item_media already contains ${existingCount} link(s). ` +
          'Review them first, then rerun with --replace if replacement is intended.',
      )
    }
  }

  let successCount = 0

  for (const item of verified.valid) {
    const fileData = new Uint8Array(await readFile(item.absolutePath))
    const mimeType = SUPPORTED_IMAGE_TYPES.get(item.extension)
    const altText = `${item.guideItem.name} in Culebra, Puerto Rico`

    console.log(
      `Uploading ${item.relativePath} -> ${item.slug} | role=${item.role} | sort_order=${item.sortOrder}`,
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
        INSERT INTO guide_item_media (
          guide_item_id,
          media_id,
          role,
          sort_order
        )
        VALUES (
          ${item.guideItem.id},
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
  console.log(`Guide image import complete. Success: ${successCount}. Failures: 0.`)
}

async function main() {
  const options = parseArguments(process.argv.slice(2))
  const files = await walkImages(options.directory)
  const { matched, skipped, unresolved } = buildFilePlan(files)
  const assignments = assignRoles(matched)

  if (options.dryRun) {
    printDryRun(files, assignments, skipped, unresolved)
    return
  }

  const sql = neon(getDatabaseUrl())
  const verified = await verifyPlanAgainstDatabase(
    sql,
    assignments,
    skipped,
    unresolved,
  )

  printDatabaseVerification(verified)

  if (verified.invalid.length > 0) {
    throw new Error(
      'Nothing was changed. Fix the unresolved/rejected mappings and run again.',
    )
  }

  await importGuideImages(sql, options, verified)
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : error)
  process.exitCode = 1
})