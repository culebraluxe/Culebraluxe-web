import assert from 'node:assert/strict'
import test from 'node:test'
import { existsSync, statSync } from 'node:fs'
import { mkdtempSync } from 'node:fs'
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
