import assert from 'node:assert/strict'
import test from 'node:test'
import { chmodSync, existsSync, readFileSync, rmSync, statSync } from 'node:fs'
import { mkdtempSync, writeFileSync } from 'node:fs'
import { execFileSync } from 'node:child_process'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import {
  applyRtkToEnv,
  rtkRewrite,
  rtkShimDir,
  rtkSupportsCommand,
  serenaAllowedToolsForRole,
  serenaMcpAddCommand,
  serenaMcpRegistration,
  writeRtkShims,
} from '../forge/forge-tool-seams'

// --- V5-24 rtk: transparent rewrite, exact exit truth ------------------------

test('V5-24: supported commands rewrite to the rtk proxy', () => {
  assert.equal(rtkRewrite('git status'), 'rtk git status')
  assert.equal(rtkRewrite('git diff --stat'), 'rtk git diff --stat')
  assert.equal(rtkRewrite('ls -la'), 'rtk ls -la')
  assert.equal(rtkRewrite('tree src'), 'rtk tree src')
})

test('V5-24: unsupported commands are left alone, so the lane degrades rather than fails', () => {
  assert.equal(rtkRewrite('pnpm test'), null)
  assert.equal(rtkRewrite('rg foo'), null)
  assert.equal(rtkRewrite(''), null)
  assert.equal(rtkSupportsCommand('git log'), true)
  assert.equal(rtkSupportsCommand('pnpm build'), false)
})

test('V5-24: shims exec the proxy so exit codes are preserved exactly', () => {
  const workspace = mkdtempSync(join(tmpdir(), 'forge-rtk-'))
  const written = writeRtkShims({ workspace, rtkBin: '/usr/local/bin/rtk' })
  assert.equal(written.length, 4)
  for (const path of written) {
    assert.ok(existsSync(path), path)
    assert.ok(statSync(path).mode & 0o111, `${path} must be executable`)
  }
  // The shim must exec (replace the process), never run-and-continue: that is
  // what makes the exit status rtk's own.
  const shim = require('node:fs').readFileSync(join(rtkShimDir(workspace), 'git'), 'utf8') as string
  assert.match(shim, /^exec "/m)
  assert.match(shim, /"\/usr\/local\/bin\/rtk" git "\$@"/)
})

test('V5-24: the shim dir is prepended to PATH for a granted position', () => {
  const workspace = mkdtempSync(join(tmpdir(), 'forge-rtk-env-'))
  const result = applyRtkToEnv({
    role: 'smith',
    env: { PATH: '/usr/bin' },
    workspace,
    rtkBin: 'rtk',
  })
  if (result.applied) {
    assert.ok(result.env.PATH!.startsWith(rtkShimDir(workspace)))
  } else {
    // Until the catalog reports rtk runnable, the seam refuses to half-apply.
    assert.match(result.reason!, /not wired|not granted/)
    assert.equal(result.env.PATH, '/usr/bin')
  }
})

test('V5-24: a position with no rtk grant is never given the shim path', () => {
  const workspace = mkdtempSync(join(tmpdir(), 'forge-rtk-none-'))
  const result = applyRtkToEnv({ role: 'scout', env: { PATH: '/usr/bin' }, workspace, rtkBin: 'rtk' })
  assert.equal(result.applied, false)
  assert.equal(result.env.PATH, '/usr/bin')
})

// --- V5-23 serena: registered as an MCP server, authority not delegated ------

test('V5-23: the serena registration invokes the real MCP entrypoint', () => {
  const reg = serenaMcpRegistration({ workspace: '/w', serenaBin: '/usr/local/bin/serena' })
  assert.equal(reg.name, 'serena')
  assert.equal(reg.command, '/usr/local/bin/serena')
  assert.deepEqual(reg.args, ['start-mcp-server', '--project', '/w'])
  assert.match(serenaMcpAddCommand({ workspace: '/w' }), /^opencode mcp add serena -- serena start-mcp-server/)
})

test('V5-23: the tool list follows the catalog grant, not the registration', () => {
  // Smith may edit; Architect may not; Scout is excluded entirely.
  const smith = serenaAllowedToolsForRole('smith')
  assert.ok(smith.includes('rename_symbol'))
  assert.ok(smith.includes('find_symbol'))

  const architect = serenaAllowedToolsForRole('architect')
  assert.ok(architect.includes('find_symbol'))
  assert.equal(architect.includes('rename_symbol'), false)
  assert.equal(architect.includes('replace_symbol_body'), false)

  for (const role of ['scout', 'inspector', 'assay', 'dev_ops'] as const) {
    assert.deepEqual(serenaAllowedToolsForRole(role), [], role)
  }
})

// --- V5-24 rtk: the shim must not be able to re-enter itself ------------------
//
// The old shim was `exec rtk <cmd> "$@"` with its own directory FIRST on PATH.
// rtk resolves the underlying command BY NAME through PATH, so rtk's own `git`
// lookup found the shim again, which exec'd rtk again — an unbounded loop. One
// Architect tool call left 4,627 live processes and load 34 on the machine, and
// every call blocked ~80s at 0% CPU because each parent slept on its child.
test('V5-24: the shim strips its own dir from PATH so the proxy cannot re-enter itself', () => {
  const tmp = mkdtempSync(join(tmpdir(), 'forge-rtk-recursion-'))
  const counter = join(tmp, 'depth')
  const seenPath = join(tmp, 'seen-path')
  const rtkBin = join(tmp, 'fake-rtk')
  try {
    // A fake proxy that behaves like the real one in the ONE way that matters:
    // it runs the underlying command by name, through PATH. Depth is bounded so
    // a regression FAILS here instead of forking the test machine.
    writeFileSync(
      rtkBin,
      [
        '#!/bin/sh',
        `depth=$(cat "${counter}" 2>/dev/null || echo 0)`,
        'depth=$((depth + 1))',
        `printf '%s' "$depth" > "${counter}"`,
        '[ "$depth" -gt 5 ] && exit 3',
        `printf '%s' "$PATH" > "${seenPath}"`,
        'shift',
        'exec git "$@"',
      ].join('\n') + '\n',
      'utf8',
    )
    chmodSync(rtkBin, 0o755)

    const workspace = mkdtempSync(join(tmpdir(), 'forge-rtk-ws-'))
    writeRtkShims({ workspace, rtkBin })
    const shimDir = rtkShimDir(workspace)
    const shim = join(shimDir, 'git')

    // Invoke it exactly as a lane does: the shim dir FIRST on PATH.
    const out = execFileSync(shim, ['rev-parse', '--git-dir'], {
      cwd: process.cwd(),
      encoding: 'utf8',
      env: { ...process.env, PATH: `${shimDir}:${process.env.PATH ?? ''}` },
    })

    assert.match(out, /\.git/, 'the proxied command still runs and returns the real answer')
    const proxied = readFileSync(seenPath, 'utf8')
    assert.ok(
      !proxied.split(':').includes(shimDir),
      'the proxy must not see its own shim directory on PATH',
    )
    assert.equal(readFileSync(counter, 'utf8'), '1', 'the proxy must be entered exactly once')
  } finally {
    rmSync(tmp, { recursive: true, force: true })
  }
})
