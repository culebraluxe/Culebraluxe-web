/**
 * SHIM. The live Forge worker is rust/forge (`cargo run -p forge --bin forge`).
 * This file stays so old docs that invoke the TS path still reach Rust.
 */
import { spawnSync } from 'node:child_process'
import { resolve } from 'node:path'

const child = spawnSync(
  'cargo',
  ['run', '--manifest-path', resolve('rust/Cargo.toml'), '-p', 'forge', '--bin', 'forge', '--', ...process.argv.slice(2)],
  { stdio: 'inherit', env: { ...process.env, APP_ENV: process.env.APP_ENV ?? 'production', EXECUTION_ENV: process.env.EXECUTION_ENV ?? 'PROD' } },
)
process.exit(child.status ?? 1)
