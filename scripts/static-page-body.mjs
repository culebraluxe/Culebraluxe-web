// Turn a static TypeScript page into a Rust body, WITHOUT retyping its text.
//
// The wording matters — this is the privacy policy Meta requires for the WhatsApp integration — so nothing is
// transcribed by hand and nothing is summarised: the script reads the page, takes its headings and body text in order,
// and writes a Rust constant plus a render function. The text that was reviewed is the text that ships.
//
//   node /tmp/static-body.mjs <screen-key> <app-dir> <fn-name>
import { readFileSync, writeFileSync } from 'node:fs'

const [key, dir, fn] = process.argv.slice(2)
if (!key || !dir || !fn) {
  console.error('usage: node scripts/static-page-body.mjs <screen-key> <app-dir> <fn-name>')
  console.error('  e.g. node scripts/static-page-body.mjs site-privacy /privacy privacy_view')
  process.exit(1)
}
const source = readFileSync(`app${dir}/page.tsx`, 'utf8')

// Constants the page interpolates, so the body carries the value rather than the expression.
const consts = {}
for (const m of source.matchAll(/const ([A-Z_]+) = '([^']*)'/g)) consts[m[1]] = m[2]

const parts = []
const push = (kind, raw) => {
  let text = raw
    // {CONST} -> its value; anything else interpolated is reported rather than silently dropped.
    .replace(/\{([A-Z_]+)\}/g, (_m, name) => consts[name] ?? `{${name}}`)
    .replace(/\{[^}]*\}/g, '')
    .replace(/<[^>]+>/g, '')
    .replace(/&apos;/g, "'")
    .replace(/&amp;/g, '&')
    .replace(/\s+/g, ' ')
    .trim()
  if (!text) return
  parts.push({ kind, text })
}

// Sections in document order: headings, paragraphs, list items.
for (const m of source.matchAll(/<(h1|h2|h3|p|li)\b[^>]*>([\s\S]*?)<\/\1>/g)) {
  push(m[1], m[2])
}

const escaped = (s) => s.replace(/\\/g, '\\\\').replace(/"/g, '\\"')
const rows = parts.map((p) => `    ("${p.kind}", "${escaped(p.text)}"),`).join('\n')

const code = `
/// The ${key} page's own text, extracted from the TypeScript page it replaces.
///
/// NOT retyped, deliberately: this is reviewed wording (a policy Meta requires for the WhatsApp integration), and a
/// hand-copied paragraph is a paragraph that can quietly differ. \`kind\` is the tag it had, so the render below can put
/// it back in the same shape.
const ${fn.toUpperCase()}_CONTENT: [(&str, &str); ${parts.length}] = [
${rows}
];

/// The static page, rendered from that text. No read model and no fetch: a static page is content, and content does not
/// belong in a database query.
fn ${fn}() -> String {
    let body = ${fn.toUpperCase()}_CONTENT
        .iter()
        .map(|(kind, text)| match *kind {
            "h1" | "h2" | "h3" => format!(
                "<h2 class=\\"font-serif text-2xl font-light text-foreground\\">{}</h2>",
                escape(text)
            ),
            "li" => format!("<li class=\\"ml-6 list-disc\\">{}</li>", escape(text)),
            _ => format!(
                "<p class=\\"mt-4 text-sm font-light leading-7 text-muted-foreground\\">{}</p>",
                escape(text)
            ),
        })
        .collect::<String>();
    format!(
        "<article class=\\"px-6 py-20 md:px-12 md:py-28\\"><div class=\\"mx-auto max-w-4xl space-y-6\\">{body}</div></article>"
    )
}
`

const view = readFileSync('rust/ui/src/view.rs', 'utf8')
const marker = '/// Screens that render markup of their own instead of a generic list of rows.'
if (!view.includes(marker)) {
  console.error('marker not found in view.rs')
  process.exit(1)
}
writeFileSync('rust/ui/src/view.rs', view.replace(marker, `${code}\n${marker}`))

// And the dispatch entry.
const updated = readFileSync('rust/ui/src/view.rs', 'utf8')
writeFileSync(
  'rust/ui/src/view.rs',
  updated.replace(
    `        "rust-lab" => Some(rust_lab(model)),`,
    `        "rust-lab" => Some(rust_lab(model)),\n        "${key}" => Some(${fn}()),`,
  ),
)

console.log(`${key}: ${parts.length} pieces of text -> ${fn}()`)
