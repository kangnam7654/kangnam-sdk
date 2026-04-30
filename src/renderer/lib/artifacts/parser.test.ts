import { describe, expect, it } from 'vitest'
import { createArtifactParser, type ArtifactEvent } from './parser'

function feed(parser: ReturnType<typeof createArtifactParser>, chunks: string[]): ArtifactEvent[] {
  const out: ArtifactEvent[] = []
  for (const c of chunks) {
    for (const ev of parser.feed(c)) out.push(ev)
  }
  for (const ev of parser.flush()) out.push(ev)
  return out
}

describe('createArtifactParser', () => {
  it('passes through plain text without artifact', () => {
    const events = feed(createArtifactParser(), ['hello world'])
    expect(events).toEqual([{ type: 'text', delta: 'hello world' }])
  })

  it('emits start/chunk/end for a complete artifact in one chunk', () => {
    const html =
      'before <artifact identifier="a" type="text/html" title="t">body</artifact> after'
    const events = feed(createArtifactParser(), [html])
    expect(events.map((e) => e.type)).toEqual([
      'text',
      'artifact:start',
      'artifact:chunk',
      'artifact:end',
      'text',
    ])
    const start = events[1] as Extract<ArtifactEvent, { type: 'artifact:start' }>
    expect(start.identifier).toBe('a')
    expect(start.artifactType).toBe('text/html')
    expect(start.title).toBe('t')
    const end = events[3] as Extract<ArtifactEvent, { type: 'artifact:end' }>
    expect(end.fullContent).toBe('body')
  })

  it('handles open tag split across chunks', () => {
    const events = feed(createArtifactParser(), [
      'pre <art',
      'ifact identifier="x" type="md" title="t">',
      'body</artifact>',
    ])
    const types = events.map((e) => e.type)
    expect(types).toContain('artifact:start')
    expect(types).toContain('artifact:end')
    const end = events.find((e) => e.type === 'artifact:end') as Extract<
      ArtifactEvent,
      { type: 'artifact:end' }
    >
    expect(end.fullContent).toBe('body')
  })

  it('handles close tag split across chunks', () => {
    const events = feed(createArtifactParser(), [
      '<artifact identifier="x" type="md" title="t">hello',
      ' world</art',
      'ifact>tail',
    ])
    const end = events.find((e) => e.type === 'artifact:end') as Extract<
      ArtifactEvent,
      { type: 'artifact:end' }
    >
    expect(end.fullContent).toBe('hello world')
    const tailText = events.find(
      (e) => e.type === 'text' && (e as { delta: string }).delta === 'tail',
    )
    expect(tailText).toBeTruthy()
  })

  it('does not match <artifactual or other prefixes', () => {
    const events = feed(createArtifactParser(), ['<artifactual prose'])
    expect(events).toEqual([{ type: 'text', delta: '<artifactual prose' }])
  })

  it('streams chunks while inside artifact, holding back close-tag tail', () => {
    const parser = createArtifactParser()
    const buf: ArtifactEvent[] = []
    // Feed enough body so that body.length > CLOSE_TAG.length - 1 (= 10),
    // forcing the parser to emit a chunk for the leading bytes.
    for (const ev of parser.feed(
      '<artifact identifier="x" type="md" title="t">aaaaaaaaaaaaaaa',
    ))
      buf.push(ev)
    expect(buf.map((e) => e.type)).toEqual(['artifact:start', 'artifact:chunk'])
    const chunk = buf[1] as Extract<ArtifactEvent, { type: 'artifact:chunk' }>
    expect(chunk.delta.length).toBeGreaterThan(0)
  })

  it('holds back the buffer when body is shorter than close-tag length', () => {
    const parser = createArtifactParser()
    const buf: ArtifactEvent[] = []
    for (const ev of parser.feed('<artifact identifier="x" type="md" title="t">hi')) buf.push(ev)
    // Only the start event fires; "hi" is held back because it could be
    // the prefix of "</artifact>".
    expect(buf.map((e) => e.type)).toEqual(['artifact:start'])
  })

  it('flush emits dangling text without close', () => {
    const parser = createArtifactParser()
    const out: ArtifactEvent[] = []
    for (const ev of parser.feed('plain only')) out.push(ev)
    for (const ev of parser.flush()) out.push(ev)
    expect(out).toEqual([{ type: 'text', delta: 'plain only' }])
  })

  it('flush closes an unterminated artifact', () => {
    const parser = createArtifactParser()
    const out: ArtifactEvent[] = []
    for (const ev of parser.feed('<artifact identifier="z" type="md" title="t">half'))
      out.push(ev)
    for (const ev of parser.flush()) out.push(ev)
    const end = out.find((e) => e.type === 'artifact:end') as Extract<
      ArtifactEvent,
      { type: 'artifact:end' }
    >
    expect(end.fullContent).toBe('half')
  })

  it('parses single-quoted attributes', () => {
    const events = feed(createArtifactParser(), [
      "<artifact identifier='a' type='md' title='t'>x</artifact>",
    ])
    const start = events.find((e) => e.type === 'artifact:start') as Extract<
      ArtifactEvent,
      { type: 'artifact:start' }
    >
    expect(start.identifier).toBe('a')
  })
})
