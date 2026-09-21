#!/usr/bin/env node
/**
 * The frontend build: the Rust UI's WASM first, then Next.
 *
 * WHY THIS EXISTS. `lib/rust-ui/` is a gitignored build artifact, and `next build` does not produce it. So a deploy
 * built the frontend with no WASM in it, and the screens that mount the Rust UI host would have failed at runtime with
 * no build error to explain it. The two had to be joined.
 *
 * THE TOOLCHAIN PROBLEM, handled deliberately. Building the WASM needs a Rust toolchain with the
 * `wasm32-unknown-unknown` target. Whether the frontend builder has one is not something this script can rely on - and
 * a build that FAILS there is worse than a build that ships a slightly stale artifact. So:
 *
 *   toolchain present  -> rebuild the WASM, and fail if that fails (a real error should stop the build)
 *   toolchain absent   -> keep the committed artifact and warn loudly
 *   neither            -> fail, because shipping a UI whose WASM is missing is a broken UI
 *
 * That is why the artifacts are committed. WASM is platform-independent, so committing it does not have the usual
 * "built binary from someone's laptop" problem - the server is the thing that must be built on Linux, and it is
 * (`rust/Dockerfile.vercel` builds it inside the Linux container).
 */
import { execFileSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..')
const wasmGlue = resolve(repoRoot, 'lib/rust-ui/ui.js')
const wasmBin = resolve(repoRoot, 'lib/rust-ui/ui_bg.wasm')

function hasRustWasmTarget() {
  try {
    const targets = execFileSync('rustup', ['target', 'list', '--installed'], { encoding: 'utf8' })
    return targets.includes('wasm32-unknown-unknown')
  } catch {
    return false
  }
}

function buildRustUi() {
  console.log('==> rust ui (wasm)')
  execFileSync('bash', ['scripts/rust-ui-build.sh'], {
    cwd: repoRoot,
    stdio: 'inherit',
    env: { ...process.env, RUST_UI_PROFILE: process.env.RUST_UI_PROFILE ?? 'release' },
  })
}

const artifactsPresent = existsSync(wasmGlue) && existsSync(wasmBin)

if (hasRustWasmTarget()) {
  buildRustUi()
} else if (artifactsPresent) {
  console.warn(
    '==> WARNING: no wasm32-unknown-unknown Rust target here, so the Rust UI was NOT rebuilt.\n' +
      '    Using the committed artifact in lib/rust-ui/. If you changed rust/ui, run `pnpm ui:build:release`\n' +
      '    first on a machine that has the target, and commit the result.',
  )
} else {
  console.error(
    '==> FAIL: the Rust UI artifacts are missing and cannot be built here (no wasm32 target).\n' +
      '    Run `pnpm ui:build:release` on a machine with the target and commit lib/rust-ui/.',
  )
  process.exit(1)
}

console.log('==> next build')
execFileSync('npx', ['next', 'build', '--webpack'], { cwd: repoRoot, stdio: 'inherit' })
