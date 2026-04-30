import { describe, expect, it } from 'vitest'
import { buildSrcdoc } from './srcdoc'

describe('buildSrcdoc', () => {
  it('wraps a fragment in a doctype shell', () => {
    const out = buildSrcdoc('<h1>Hello</h1>')
    expect(out).toMatch(/^<!doctype html>/)
    expect(out).toContain('<h1>Hello</h1>')
  })

  it('passes a full document through when passthroughFullDocuments is set', () => {
    const full = '<!doctype html><html><body>hi</body></html>'
    expect(buildSrcdoc(full, { passthroughFullDocuments: true })).toBe(full)
  })

  it('still injects shims into a full doc when passthrough is off', () => {
    const full = '<!doctype html><html><head></head><body>hi</body></html>'
    const out = buildSrcdoc(full)
    expect(out).toContain('localStorage')
    expect(out).toContain("addEventListener('error'")
  })

  it('injects baseHref into <head>', () => {
    const out = buildSrcdoc('<p>x</p>', { baseHref: 'tauri://localhost/.od/abc/' })
    expect(out).toContain('<base href="tauri://localhost/.od/abc/"')
  })

  it('escapes baseHref attribute', () => {
    const out = buildSrcdoc('<p>x</p>', { baseHref: 'a"b<c' })
    expect(out).toContain('<base href="a&quot;b&lt;c"')
  })

  it('includes a custom error channel', () => {
    const out = buildSrcdoc('<p>x</p>', { errorChannel: 'kangnam-preview' })
    expect(out).toContain('"kangnam-preview"')
  })

  it('includes the localStorage shim', () => {
    const out = buildSrcdoc('<p>x</p>')
    expect(out).toContain('tryShim(\'localStorage\')')
  })
})
