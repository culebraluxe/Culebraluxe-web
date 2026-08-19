import { readFile, readdir } from 'node:fs/promises'
import path from 'node:path'
import { neon } from '@neondatabase/serverless'

const FILE_TO_SLUG = new Map([
  ['caracoles_restaurant.jpg', 'caracoles'],
  ['punta_soldado.jpg', 'punta-soldado'],
  ['datiles_beach.jpg', 'datiles-beach'],
  ['resaca_beach.jpg', 'resaca-beach'],
  ['harspoons.jpg', 'harspoons'],
  ['rolls_of_heaven.jpg', 'rolls-of-heaven'],
  ['heathers_pizza.jpg', 'heathers-pizza'],
  ['susies.jpg', 'susies'],
  ['la_cocina_del_navegante.jpg', 'la-cocina-del-navegante'],
  ['tikis_grill.jpg', 'tikis-grill'],
  ['la_jibara_pizzeria_creativa.jpg', 'la-jibara'],
  ['zacos_tacos.jpeg', 'zacos-tacos'],
])

const MIME = new Map([
  ['.jpg', 'image/jpeg'],
  ['.jpeg', 'image/jpeg'],
  ['.png', 'image/png'],
  ['.webp', 'image/webp'],
])

function parseArgs() {
  const args = process.argv.slice(2)
  const dirIndex = args.indexOf('--dir')
  if (dirIndex === -1 || !args[dirIndex + 1]) throw new Error('Missing --dir')
  return { dir: args[dirIndex + 1], dryRun: args.includes('--dry-run') }
}

function getDatabaseUrl() {
  const appEnv = process.env.APP_ENV ?? 'development'
  const url = appEnv === 'production' ? process.env.DATABASE_URL_PROD : process.env.DATABASE_URL_DEV
  if (!url) throw new Error(`Database URL missing for APP_ENV="${appEnv}"`)
  return url
}

async function scan(dir) {
  const entries = await readdir(dir, { withFileTypes: true })
  return entries
    .filter((e) => e.isFile() && MIME.has(path.extname(e.name).toLowerCase()))
    .map((e) => ({
      filename: e.name,
      absolutePath: path.join(dir, e.name),
      extension: path.extname(e.name).toLowerCase(),
      slug: FILE_TO_SLUG.get(e.name) ?? null,
    }))
}

async function main() {
  const options = parseArgs()
  const files = await scan(options.dir)
  const unresolved = files.filter((f) => !f.slug)
  const missingExpected = [...FILE_TO_SLUG.keys()].filter(
    (name) => !files.some((f) => f.filename === name),
  )
  const mapped = files.filter((f) => f.slug)

  console.log(`Mapped files: ${mapped.length}`)
  console.log(`Unresolved files: ${unresolved.length}`)
  console.log(`Expected files missing: ${missingExpected.length}`)
  console.log('')
  for (const item of mapped) console.log(`${item.filename} -> ${item.slug}`)

  if (unresolved.length || missingExpected.length) {
    console.log('')
    if (unresolved.length) {
      console.log('UNRESOLVED')
      for (const item of unresolved) console.log(item.filename)
    }
    if (missingExpected.length) {
      console.log('EXPECTED BUT MISSING')
      for (const name of missingExpected) console.log(name)
    }
    throw new Error('Local verification failed; no database changes made.')
  }

  if (options.dryRun) {
    console.log('')
    console.log('DRY RUN ONLY — Neon was not contacted and no data was changed.')
    return
  }

  const sql = neon(getDatabaseUrl())
  const verified = []

  for (const item of mapped) {
    const rows = await sql`
      select
        gi.id,
        gi.name,
        exists (
          select 1 from guide_item_media gim
          where gim.guide_item_id = gi.id
        ) as already_has_media
      from guide_item gi
      where gi.slug = ${item.slug}
        and gi.is_active = true
      limit 1
    `
    const row = rows[0]
    if (!row) throw new Error(`Active guide item not found: ${item.slug}`)
    if (row.already_has_media) {
      throw new Error(`Refusing to overwrite existing media for: ${item.slug}`)
    }
    verified.push({ ...item, guideItemId: String(row.id), guideItemName: String(row.name) })
  }

  console.log('')
  console.log(`Verified new assignments: ${verified.length}`)

  let success = 0
  for (const item of verified) {
    const fileData = new Uint8Array(await readFile(item.absolutePath))
    const mimeType = MIME.get(item.extension)
    const altText = `${item.guideItemName} in Culebra, Puerto Rico`
    console.log(`Uploading ${item.filename} -> ${item.slug}`)

    let mediaId = null
    try {
      const mediaRows = await sql`
        insert into media (
          file_data, filename, mime_type, file_size, alt_text, media_type
        )
        values (
          ${fileData}, ${item.filename}, ${mimeType}, ${fileData.byteLength}, ${altText}, 'image'
        )
        returning id
      `
      mediaId = String(mediaRows[0].id)

      await sql`
        insert into guide_item_media (
          guide_item_id, media_id, role, sort_order
        )
        values (
          ${item.guideItemId}, ${mediaId}, 'card', 0
        )
      `
      success += 1
    } catch (error) {
      if (mediaId) {
        try {
          await sql`delete from media where id = ${mediaId}`
        } catch {}
      }
      throw error
    }
  }

  console.log('')
  console.log(`Missing guide image import complete. Success: ${success}.`)
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : error)
  process.exitCode = 1
})