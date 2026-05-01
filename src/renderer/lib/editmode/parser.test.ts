import { describe, expect, it } from 'vitest'

import { parseEditmodeBlocks, replaceTweakValue } from './parser'

describe('parseEditmodeBlocks', () => {
  it('extracts a single color block', () => {
    const body =
      '<!-- @tweak id="hero-color" type="color" label="Hero" value="#1f2937" -->\n<h1>Hi</h1>\n<!-- /@tweak -->'
    const blocks = parseEditmodeBlocks(body)
    expect(blocks).toHaveLength(1)
    expect(blocks[0]!.id).toBe('hero-color')
    expect(blocks[0]!.type).toBe('color')
    expect(blocks[0]!.value).toBe('#1f2937')
    expect(blocks[0]!.label).toBe('Hero')
  })

  it('parses slider extras', () => {
    const body =
      '<!-- @tweak id="size" type="slider" label="Size" value="20" min="10" max="40" step="2" -->\n<p>x</p>\n<!-- /@tweak -->'
    const blocks = parseEditmodeBlocks(body)
    expect(blocks[0]!.min).toBe(10)
    expect(blocks[0]!.max).toBe(40)
    expect(blocks[0]!.step).toBe(2)
  })

  it('parses select options', () => {
    const body =
      '<!-- @tweak id="font" type="select" label="Font" value="Inter" options="Inter,Pretendard,Geist" -->\n<p>x</p>\n<!-- /@tweak -->'
    const blocks = parseEditmodeBlocks(body)
    expect(blocks[0]!.options).toEqual(['Inter', 'Pretendard', 'Geist'])
  })

  it('skips blocks missing required attrs', () => {
    const body =
      '<!-- @tweak label="No id" type="color" value="#fff" -->\n<p>x</p>\n<!-- /@tweak -->'
    expect(parseEditmodeBlocks(body)).toHaveLength(0)
  })

  it('skips unknown types', () => {
    const body =
      '<!-- @tweak id="x" type="bogus" label="X" value="a" -->\n<p>x</p>\n<!-- /@tweak -->'
    expect(parseEditmodeBlocks(body)).toHaveLength(0)
  })

  it('skips unclosed blocks', () => {
    const body =
      '<!-- @tweak id="open" type="color" label="X" value="#fff" -->\n<p>no closer</p>'
    expect(parseEditmodeBlocks(body)).toHaveLength(0)
  })

  it('extracts multiple blocks in order', () => {
    const body =
      '<!-- @tweak id="a" type="color" label="A" value="#000" -->x<!-- /@tweak -->' +
      '<!-- @tweak id="b" type="text" label="B" value="hi" -->y<!-- /@tweak -->'
    const blocks = parseEditmodeBlocks(body)
    expect(blocks.map((b) => b.id)).toEqual(['a', 'b'])
  })
})

describe('replaceTweakValue', () => {
  it('replaces existing value', () => {
    const body =
      '<!-- @tweak id="c" type="color" label="C" value="#fff" -->\n<p>x</p>\n<!-- /@tweak -->'
    const next = replaceTweakValue(body, 'c', '#000')
    expect(next).toContain('value="#000"')
    expect(next).not.toContain('value="#fff"')
  })

  it('inserts value when absent', () => {
    const body =
      '<!-- @tweak id="d" type="color" label="D" -->\n<p>x</p>\n<!-- /@tweak -->'
    const next = replaceTweakValue(body, 'd', '#abc')
    expect(next).toContain('value="#abc"')
  })

  it('escapes quotes in value', () => {
    const body =
      '<!-- @tweak id="e" type="text" label="E" value="ok" -->\n<p>x</p>\n<!-- /@tweak -->'
    const next = replaceTweakValue(body, 'e', 'a"b')
    expect(next).toContain('value="a&quot;b"')
  })
})
