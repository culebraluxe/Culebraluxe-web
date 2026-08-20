// ---------------------------------------------------------------------------
// mini-xml — minimal, deterministic, dependency-free XML parser.
//
// This is a *bounded* XML 1.0 processor written specifically for the workflow
// definition grammar. It is NOT a general-purpose XML toolkit:
//
//   supported   : XML declaration, comments, CDATA, elements, attributes,
//                 character data, the five predefined entities and numeric
//                 character references
//   rejected    : DOCTYPE / DTD, processing instructions, undeclared entities,
//                 mismatched or unterminated tags, duplicate attributes,
//                 non-well-formed input
//
// The parser never executes code and never resolves external resources. It
// only produces a generic tree (`XmlElement` / `XmlText`) that the grammar
// mapper in `xml-parser.ts` consumes.
//
// No XML parser dependency is currently installed in the repository (checked
// against package.json / workflow_engine/package.json at CRM-14E time). Node's
// runtime has no built-in XML parser, so a bounded hand-rolled parser is the
// smallest practical dependency-free option for our controlled grammar. If the
// grammar ever needs DTDs, namespaces, arbitrary entity expansion, or full
// spec conformance, replace this file with a maintained parser such as
// `fast-xml-parser` or `sax` behind the same `parseXml` surface.
// ---------------------------------------------------------------------------

export interface XmlText {
  text: string;
}

export interface XmlElement {
  name: string;
  attributes: Record<string, string>;
  children: XmlNode[];
}

export type XmlNode = XmlElement | XmlText;

export function isXmlElement(node: XmlNode): node is XmlElement {
  return (node as XmlElement).name !== undefined;
}

export class XmlParseError extends Error {
  constructor(
    message: string,
    readonly line: number,
    readonly column: number,
  ) {
    super(`${message} (line ${line}, column ${column})`);
    this.name = 'XmlParseError';
  }
}

// ---------------------------------------------------------------------------
// Cursor
// ---------------------------------------------------------------------------

class Cursor {
  private pos = 0;

  constructor(private readonly src: string) {}

  get line(): number {
    let line = 1;
    for (let i = 0; i < this.pos; i++) {
      if (this.src.charCodeAt(i) === 10) line++;
    }
    return line;
  }

  get column(): number {
    let col = 1;
    for (let i = this.pos - 1; i >= 0 && this.src.charCodeAt(i) !== 10; i--) col++;
    return col;
  }

  eof(): boolean {
    return this.pos >= this.src.length;
  }

  peek(offset = 0): string | null {
    const i = this.pos + offset;
    return i < this.src.length ? this.src[i] : null;
  }

  next(): string {
    const ch = this.src[this.pos];
    this.pos++;
    return ch;
  }

  startsWith(s: string): boolean {
    return this.src.startsWith(s, this.pos);
  }

  skip(s: string): void {
    this.pos += s.length;
  }

  /** Index of `s` at or after the cursor, or -1. */
  indexOf(s: string): number {
    return this.src.indexOf(s, this.pos);
  }

  /** Slice from the cursor position to the given absolute index. */
  sliceTo(absEnd: number): string {
    return this.src.slice(this.pos, absEnd);
  }

  /** Advance the cursor to an absolute index. */
  seekTo(absIndex: number): void {
    this.pos = absIndex;
  }

  /** Raw source text from `from` (absolute) to `to` (absolute). */
  sourceSlice(from: number, to: number): string {
    return this.src.slice(from, to);
  }

  get position(): number {
    return this.pos;
  }

  fail(message: string): never {
    throw new XmlParseError(message, this.line, this.column);
  }
}

// ---------------------------------------------------------------------------
// Character helpers
// ---------------------------------------------------------------------------

function isNameStart(ch: string | null): boolean {
  if (ch === null) return false;
  return /[A-Za-z_]/.test(ch);
}

function isNameChar(ch: string | null): boolean {
  if (ch === null) return false;
  return /[A-Za-z0-9_.\-]/.test(ch);
}

function isWhitespace(ch: string | null): boolean {
  return ch !== null && /\s/.test(ch);
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

export function parseXml(source: string): XmlElement {
  const c = new Cursor(source);
  skipMisc(c); // XML declaration, whitespace, comments
  const root = parseElement(c);
  skipMisc(c);
  if (!c.eof()) {
    c.fail('Unexpected content after root element');
  }
  return root;
}

// ---------------------------------------------------------------------------
// Low-level tokens
// ---------------------------------------------------------------------------

function skipMisc(c: Cursor): void {
  for (;;) {
    while (!c.eof() && isWhitespace(c.peek())) c.next();
    if (c.startsWith('<?')) {
      if (c.startsWith('<?xml')) {
        // XML declaration: skip to the closing '?>'.
        const end = c.indexOf('?>');
        if (end === -1) c.fail('Unterminated XML declaration');
        c.seekTo(end + 2);
        continue;
      }
      c.fail('Processing instructions are not supported');
    }
    if (c.startsWith('<!--')) {
      skipComment(c);
      continue;
    }
    return;
  }
}

function skipComment(c: Cursor): void {
  c.skip('<!--');
  const end = c.indexOf('-->');
  if (end === -1) c.fail('Unterminated comment');
  c.seekTo(end + 3);
}

function parseName(c: Cursor): string {
  if (!isNameStart(c.peek())) {
    c.fail('Expected a name');
  }
  let name = c.next();
  while (isNameChar(c.peek())) name += c.next();
  return name;
}

function skipWs(c: Cursor): void {
  while (!c.eof() && isWhitespace(c.peek())) c.next();
}

function parseElement(c: Cursor): XmlElement {
  if (c.peek() !== '<') c.fail('Expected an element');
  c.next();
  if (c.peek() === '/') c.fail('Unexpected closing tag');
  if (c.peek() === '!') c.fail('Unexpected markup declaration (DOCTYPE/CDATA not allowed here)');

  const name = parseName(c);
  const attributes: Record<string, string> = {};

  for (;;) {
    skipWs(c);
    const ch = c.peek();
    if (ch === null) c.fail(`Unterminated element <${name}>`);
    if (ch === '/') {
      c.next();
      if (c.peek() !== '>') c.fail(`Malformed self-closing tag <${name}/>`);
      c.next();
      return { name, attributes, children: [] };
    }
    if (ch === '>') {
      c.next();
      break;
    }
    if (ch === '?') c.fail(`Unexpected '?' inside element <${name}>`);
    const attrName = parseName(c);
    skipWs(c);
    if (c.peek() !== '=') c.fail(`Expected '=' after attribute '${attrName}'`);
    c.next();
    skipWs(c);
    const quote = c.peek();
    if (quote !== '"' && quote !== "'") {
      c.fail(`Attribute '${attrName}' value must be quoted`);
    }
    c.next();
    const value = parseAttrValue(c, quote);
    if (attributes[attrName] !== undefined) {
      c.fail(`Duplicate attribute '${attrName}' on <${name}>`);
    }
    attributes[attrName] = value;
  }

  const children: XmlNode[] = [];
  for (;;) {
    if (c.eof()) c.fail(`Unterminated element <${name}>: missing </${name}>`);
    if (c.startsWith('</')) {
      c.skip('</');
      const close = parseName(c);
      skipWs(c);
      if (c.peek() !== '>') c.fail(`Malformed closing tag </${close}>`);
      c.next();
      if (close !== name) {
        c.fail(`Mismatched closing tag: expected </${name}> but found </${close}>`);
      }
      return { name, attributes, children };
    }
    if (c.startsWith('<!--')) {
      skipComment(c);
      continue;
    }
    if (c.startsWith('<![CDATA[')) {
      children.push({ text: parseCdata(c) });
      continue;
    }
    if (c.peek() === '<') {
      if (c.startsWith('<?')) {
        c.fail('Processing instructions are not supported inside elements');
      }
      children.push(parseElement(c));
      continue;
    }
    const text = parseText(c);
    if (text.trim().length > 0) {
      children.push({ text });
    }
  }
}

function parseAttrValue(c: Cursor, quote: string): string {
  let value = '';
  for (;;) {
    const ch = c.peek();
    if (ch === null) c.fail('Unterminated attribute value');
    if (ch === quote) {
      c.next();
      return value;
    }
    if (ch === '<') c.fail("Attribute value must not contain '<'");
    if (ch === '&') {
      value += parseEntity(c);
      continue;
    }
    value += c.next();
  }
}

function parseCdata(c: Cursor): string {
  c.skip('<![CDATA[');
  const end = c.indexOf(']]>');
  if (end === -1) c.fail('Unterminated CDATA section');
  const body = c.sliceTo(end);
  c.seekTo(end + 3);
  return body;
}

function parseText(c: Cursor): string {
  let text = '';
  for (;;) {
    const ch = c.peek();
    if (ch === null || ch === '<') return text;
    if (ch === '&') {
      text += parseEntity(c);
      continue;
    }
    text += c.next();
  }
}

function parseEntity(c: Cursor): string {
  // Assumes the cursor is positioned at '&'.
  const semi = c.indexOf(';');
  if (semi === -1) c.fail('Unterminated entity reference');
  const body = c.sourceSlice(c.position + 1, semi);
  c.seekTo(semi + 1);

  switch (body) {
    case 'amp':
      return '&';
    case 'lt':
      return '<';
    case 'gt':
      return '>';
    case 'quot':
      return '"';
    case 'apos':
      return "'";
    default:
      break;
  }

  const numeric = body.match(/^#(?:x([0-9A-Fa-f]+)|([0-9]+))$/);
  if (numeric) {
    const code = numeric[1]
      ? parseInt(numeric[1], 16)
      : parseInt(numeric[2], 10);
    if (Number.isNaN(code) || code <= 0 || code > 0x10ffff) {
      c.fail(`Invalid numeric character reference '&${body};'`);
    }
    return String.fromCodePoint(code);
  }

  c.fail(`Unknown entity '&${body};' (only & < > " ' and numeric references are supported)`);
}
