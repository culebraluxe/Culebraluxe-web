import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  applySurfaceMultiplier,
  ripwireSurfaceFromPack,
  surfaceMultiplier,
} from '../forge/forge-ripwire-surface'

const smallPack = `<sigs><f p="./a.ts"><d l="1" n="foo" cx="2" ccx="3" rel="caller"/></f><f p="./b.ts"><d l="8" n="bar" ccx="7"/></f></sigs>`
const largePack = `<sigs><f p="./a.ts"><d l="1" n="f1" ccx="10" rel="caller"/><d l="2" n="f2" ccx="20" rel="caller"/></f>
  <f p="./b.ts"><d l="4" n="g1" ccx="30" rel="caller"/></f><f p="./c.ts"><d l="6" n="h1" ccx="40"/></f>
  <f p="./d.ts"><d l="8" n="i1" ccx="50"/></f><f p="./e.ts"><d l="9" n="j1" ccx="60"/></f>
  <f p="./f.ts"><d l="10" n="k1" ccx="70"/></f><f p="./g.ts"><d l="11" n="l1" ccx="80"/></f>
  <f p="./h.ts"><d l="12" n="m1" ccx="90"/></f><f p="./i.ts"><d l="13" n="n1" ccx="100"/></f></sigs>`

test('V2: no ripwire pack -> identity (multiplier 1, no surface)', () => {
  assert.deepEqual(ripwireSurfaceFromPack(null), { files: 0, ccxTotal: 0, callers: 0, symbols: 0 })
  assert.equal(surfaceMultiplier(ripwireSurfaceFromPack(null)), 1)
})

test('V2: a broad pack measures a larger surface than a narrow one', () => {
  const small = ripwireSurfaceFromPack(smallPack)
  const large = ripwireSurfaceFromPack(largePack)
  assert.ok(small.files > 0)
  assert.equal(small.files, 2)
  assert.ok(large.files > small.files)
  assert.ok(large.ccxTotal > small.ccxTotal)
  assert.ok(surfaceMultiplier(large) > surfaceMultiplier(small))
  // multiplier stays within the operator bounds
  assert.ok(surfaceMultiplier(small) >= 0.6)
  assert.ok(surfaceMultiplier(large) <= 3.5)
})

test('V2: the surface multiplier scales the prior forecast', () => {
  const base = {
    estimatedTokens: 1000,
    estimatedWidgets: 20,
    estimatedCostUsd: 1.0,
    estimatedMinutes: 20,
    estimatedSloc: 150,
  }
  const mult = 1.5
  const out = applySurfaceMultiplier(base, mult)
  assert.equal(out.estimatedTokens, 1500)
  assert.equal(out.estimatedWidgets, 30)
  assert.equal(out.estimatedMinutes, 30)
  assert.equal(out.estimatedSloc, 225)
  // Cost is NOT scaled here: it is derived from the scaled token/widget counts
  // at the unit rate elsewhere (the multiplier never multiplies money directly).
  assert.equal(out.estimatedCostUsd, 1.0)
})
