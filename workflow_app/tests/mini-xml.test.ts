import { test } from 'node:test'
import assert from 'node:assert/strict'
import { parseXml, XmlParseError } from '../xml/mini-xml'

test('parses a simple element with attributes and nested children', () => {
  const root = parseXml('<process key="x" version="1"><node id="a"/><node id="b">hi</node></process>')
  assert.equal(root.name, 'process')
  assert.equal(root.attributes.key, 'x')
  assert.equal(root.attributes.version, '1')
  assert.equal(root.children.length, 2)
  const first = root.children[0] as any
  assert.equal(first.name, 'node')
  assert.equal(first.attributes.id, 'a')
})

test('parses an XML declaration and comments', () => {
  const root = parseXml('<?xml version="1.0" encoding="UTF-8"?><!-- a comment --><root><x/></root>')
  assert.equal(root.name, 'root')
  assert.equal(root.children.length, 1)
})

test('resolves the five predefined entities and numeric references', () => {
  // Build the XML at runtime so the source file does not itself contain
  // entity escape sequences (which would be decoded by the editor tooling).
  const amp = String.fromCharCode(38) // '&'
  const xml = `<p a="${amp}amp;${amp}lt;${amp}gt;${amp}quot;${amp}apos;${amp}#65;${amp}#x42;">${amp}amp;</p>`
  const root = parseXml(xml)
  assert.equal(root.attributes.a, '&<>"\'AB')
  assert.equal((root.children[0] as any).text, '&')
})

test('preserves CDATA verbatim', () => {
  const root = parseXml('<p><![CDATA[a < b & c]]></p>')
  assert.equal((root.children[0] as any).text, 'a < b & c')
})

test('rejects mismatched closing tags with a useful error', () => {
  assert.throws(() => parseXml('<a><b></a>'), (err: unknown) => {
    assert.ok(err instanceof XmlParseError)
    assert.match(err.message, /Mismatched closing tag/)
    return true
  })
})

test('rejects unterminated elements', () => {
  assert.throws(() => parseXml('<a><b></b>'), XmlParseError)
})

test('rejects duplicate attributes', () => {
  assert.throws(() => parseXml('<a x="1" x="2"/>'), (err: unknown) => {
    assert.match((err as Error).message, /Duplicate attribute/)
    return true
  })
})

test('rejects unknown entities', () => {
  assert.throws(() => parseXml('<a>&nope;</a>'), (err: unknown) => {
    assert.match((err as Error).message, /Unknown entity/)
    return true
  })
})

test('rejects DOCTYPE declarations', () => {
  assert.throws(() => parseXml('<!DOCTYPE html><html/>'), (err: unknown) => {
    assert.match((err as Error).message, /Unexpected markup declaration|Unexpected/)
    return true
  })
})

test('rejects processing instructions other than the XML declaration', () => {
  assert.throws(() => parseXml('<?php echo 1; ?><root/>'), (err: unknown) => {
    assert.match((err as Error).message, /Processing instructions are not supported/)
    return true
  })
})

test('rejects trailing content after the root element', () => {
  assert.throws(() => parseXml('<root/>extra'), (err: unknown) => {
    assert.match((err as Error).message, /Unexpected content after root element/)
    return true
  })
})

test('rejects an unquoted attribute value', () => {
  assert.throws(() => parseXml('<a x=1/>'), (err: unknown) => {
    assert.match((err as Error).message, /must be quoted/)
    return true
  })
})

test('reports line/column in parse errors', () => {
  assert.throws(() => parseXml('<a>\n  <b></a>\n</b>'), (err: unknown) => {
    assert.ok(err instanceof XmlParseError)
    assert.ok(err.line >= 2)
    return true
  })
})
