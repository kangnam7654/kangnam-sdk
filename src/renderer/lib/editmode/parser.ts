/**
 * Editmode block parser — locates `<!-- @tweak ... -->` … `<!-- /@tweak -->`
 * fenced regions inside an artifact body and extracts a JSON schema
 * declaring editable controls (color / slider / select / text).
 *
 * Adapted from open-codesign `@open-codesign/shared` `parseEditmodeBlock`
 * / `replaceEditmodeBlock` / `parseTweakSchema` (MIT).
 *
 * The block format is a verbatim copy of open-codesign's so prompts /
 * skills emitting these blocks against either runtime work unchanged.
 *
 *   <!-- @tweak id="hero-title-color" type="color" label="Hero title color" value="#1f2937" -->
 *   <h1 style="color: #1f2937">Welcome</h1>
 *   <!-- /@tweak -->
 *
 * Supported `type` values: color | slider | select | text.
 * Slider extras: min, max, step.
 * Select extras: options (comma-separated).
 */
export type TweakType = 'color' | 'slider' | 'select' | 'text'

export interface TweakBlock {
  /** Unique id for the block — used as React key + persistence key. */
  id: string
  type: TweakType
  label: string
  value: string
  /** Range[min, max] for `slider`; undefined for other types. */
  min?: number
  max?: number
  step?: number
  /** Comma-split options for `select`. */
  options?: string[]
  /** Inner content between the @tweak and /@tweak markers. */
  body: string
  /** [start, end) byte offsets of the entire block in the source string. */
  range: [number, number]
  /** [start, end) byte offsets of just the body content. */
  bodyRange: [number, number]
  /** [start, end) byte offsets of the opening @tweak marker. */
  markerRange: [number, number]
}

const OPEN_PREFIX = '<!--'
const OPEN_TAG = '@tweak'
const CLOSE_RE = /<!--\s*\/@tweak\s*-->/
// Match an opening @tweak comment. Captures: 1=attr block, 2=full marker.
const OPEN_RE = /<!--\s*@tweak\s+([^>]*?)\s*-->/g

const ATTR_RE = /(\w+)\s*=\s*"([^"]*)"/g

function parseAttributes(s: string): Record<string, string> {
  const out: Record<string, string> = {}
  let m: RegExpExecArray | null
  while ((m = ATTR_RE.exec(s)) !== null) {
    out[m[1]!] = m[2]!
  }
  ATTR_RE.lastIndex = 0
  return out
}

function asTweakType(v: string | undefined): TweakType | null {
  if (v === 'color' || v === 'slider' || v === 'select' || v === 'text') return v
  return null
}

function parseNumber(v: string | undefined): number | undefined {
  if (v === undefined) return undefined
  const n = Number(v)
  return Number.isFinite(n) ? n : undefined
}

/**
 * Scan a body for all editmode blocks and return them in document order.
 * Malformed blocks (missing close marker, missing required attrs, unknown
 * type) are skipped silently — the body still renders fine without them,
 * we just won't emit a Tweak control for that region.
 */
export function parseEditmodeBlocks(body: string): TweakBlock[] {
  const out: TweakBlock[] = []
  // Reset stateful regex.
  OPEN_RE.lastIndex = 0
  let match: RegExpExecArray | null
  while ((match = OPEN_RE.exec(body)) !== null) {
    const markerStart = match.index
    const markerEnd = match.index + match[0].length
    const attrs = parseAttributes(match[1] ?? '')
    const type = asTweakType(attrs.type)
    const id = attrs.id
    if (!id || !type) continue

    // Find the close marker after this open one.
    const tail = body.slice(markerEnd)
    const closeMatch = tail.match(CLOSE_RE)
    if (!closeMatch || closeMatch.index === undefined) continue
    const bodyStart = markerEnd
    const bodyEnd = markerEnd + closeMatch.index
    const blockEnd = bodyEnd + closeMatch[0].length
    const innerBody = body.slice(bodyStart, bodyEnd)

    const block: TweakBlock = {
      id,
      type,
      label: attrs.label ?? id,
      value: attrs.value ?? '',
      body: innerBody,
      range: [markerStart, blockEnd],
      bodyRange: [bodyStart, bodyEnd],
      markerRange: [markerStart, markerEnd],
    }
    if (type === 'slider') {
      block.min = parseNumber(attrs.min)
      block.max = parseNumber(attrs.max)
      block.step = parseNumber(attrs.step)
    } else if (type === 'select' && attrs.options) {
      block.options = attrs.options.split(',').map((s) => s.trim()).filter(Boolean)
    }
    out.push(block)
    // Advance past the close marker so subsequent matches don't restart
    // inside the inner body of this block.
    OPEN_RE.lastIndex = blockEnd
  }
  return out
}

/**
 * Replace the `value=` attribute of a single block (identified by id) and
 * return the new body. Used by the TweakPanel to flush slider / color
 * picker edits back into the artifact body.
 */
export function replaceTweakValue(body: string, id: string, value: string): string {
  const blocks = parseEditmodeBlocks(body)
  const block = blocks.find((b) => b.id === id)
  if (!block) return body
  const oldMarker = body.slice(block.markerRange[0], block.markerRange[1])
  // Replace existing value="..." or insert before -->.
  let newMarker: string
  if (/value\s*=\s*"[^"]*"/.test(oldMarker)) {
    newMarker = oldMarker.replace(/value\s*=\s*"[^"]*"/, `value="${escapeAttr(value)}"`)
  } else {
    newMarker = oldMarker.replace(/-->\s*$/, ` value="${escapeAttr(value)}" -->`)
  }
  return body.slice(0, block.markerRange[0]) + newMarker + body.slice(block.markerRange[1])
}

function escapeAttr(s: string): string {
  return s.replace(/"/g, '&quot;')
}
