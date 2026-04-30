/**
 * Manifest helpers — validation + legacy-format inference.
 *
 * Ported from open-design `src/artifacts/manifest.ts` (Apache-2.0).
 * Trimmed to the entry points the renderer actually needs:
 *
 * - `parseArtifactManifest(raw)` — strict JSON validate; null on shape mismatch
 * - `artifactManifestNameFor(entry)` — `<entry>.artifact.json`
 * - `inferLegacyManifest({ entry, title })` — for old projects without manifest
 *
 * Constructors for new manifests (createHtmlArtifactManifest, etc.) are
 * not ported here; the agent emits manifests as part of the `<artifact>`
 * end-tag content, and the host backend (Tauri commands in 5b-14) is
 * responsible for writing them to disk.
 */
import type {
  ArtifactExportKind,
  ArtifactKind,
  ArtifactManifest,
  ArtifactRendererId,
} from './types'

const MANIFEST_VERSION = 1
const ALLOWED_KINDS: ReadonlySet<ArtifactKind> = new Set([
  'html',
  'deck',
  'react-component',
  'markdown-document',
  'svg',
  'diagram',
  'code-snippet',
  'mini-app',
  'design-system',
])
const ALLOWED_RENDERERS: ReadonlySet<ArtifactRendererId> = new Set([
  'html',
  'deck-html',
  'react-component',
  'markdown',
  'svg',
  'diagram',
  'code',
  'mini-app',
  'design-system',
])
const ALLOWED_EXPORTS: ReadonlySet<ArtifactExportKind> = new Set([
  'html',
  'pdf',
  'zip',
  'pptx',
  'jsx',
  'md',
  'svg',
  'txt',
])

function normalizeExt(name: string): string {
  const i = name.lastIndexOf('.')
  return i >= 0 ? name.slice(i).toLowerCase() : ''
}

function inferKindFromEntry(entry: string): ArtifactKind | null {
  const ext = normalizeExt(entry)
  if (['.html', '.htm'].includes(ext)) return 'html'
  if (ext === '.svg') return 'svg'
  if (ext === '.md') return 'markdown-document'
  if (['.jsx', '.tsx'].includes(ext)) return 'react-component'
  if (['.js', '.ts', '.json', '.css'].includes(ext)) return 'code-snippet'
  return null
}

function exportsForKind(kind: ArtifactKind): ArtifactExportKind[] {
  if (kind === 'deck') return ['html', 'pdf', 'pptx', 'zip']
  if (kind === 'react-component') return ['jsx', 'html', 'zip']
  if (kind === 'markdown-document') return ['md', 'html', 'pdf', 'zip']
  if (kind === 'svg' || kind === 'diagram') return ['svg', 'zip']
  if (kind === 'code-snippet') return ['txt', 'zip']
  return ['html', 'pdf', 'zip']
}

export function artifactManifestNameFor(entry: string): string {
  return `${entry}.artifact.json`
}

export function parseArtifactManifest(raw: string): ArtifactManifest | null {
  try {
    const parsed = JSON.parse(raw) as Partial<ArtifactManifest>
    if (parsed?.version !== MANIFEST_VERSION) return null
    if (typeof parsed.entry !== 'string' || !parsed.entry) return null
    if (typeof parsed.title !== 'string' || !parsed.title) return null
    if (!Array.isArray(parsed.exports)) return null
    if (typeof parsed.kind !== 'string' || typeof parsed.renderer !== 'string') return null
    if (!ALLOWED_KINDS.has(parsed.kind as ArtifactKind)) return null
    if (!ALLOWED_RENDERERS.has(parsed.renderer as ArtifactRendererId)) return null
    if (parsed.exports.length === 0) return null
    if (parsed.exports.some((value) => !ALLOWED_EXPORTS.has(value as ArtifactExportKind)))
      return null
    return {
      version: MANIFEST_VERSION,
      kind: parsed.kind as ArtifactKind,
      title: parsed.title,
      entry: parsed.entry,
      renderer: parsed.renderer as ArtifactRendererId,
      exports: parsed.exports as ArtifactExportKind[],
      supportingFiles: Array.isArray(parsed.supportingFiles)
        ? parsed.supportingFiles.filter((x): x is string => typeof x === 'string')
        : undefined,
      createdAt: typeof parsed.createdAt === 'string' ? parsed.createdAt : undefined,
      updatedAt: typeof parsed.updatedAt === 'string' ? parsed.updatedAt : undefined,
      sourceSkillId:
        typeof parsed.sourceSkillId === 'string' ? parsed.sourceSkillId : undefined,
      designSystemId:
        typeof parsed.designSystemId === 'string' || parsed.designSystemId === null
          ? parsed.designSystemId
          : undefined,
      metadata:
        parsed.metadata &&
        typeof parsed.metadata === 'object' &&
        !Array.isArray(parsed.metadata)
          ? parsed.metadata
          : undefined,
    }
  } catch {
    return null
  }
}

export function inferLegacyManifest(input: {
  entry: string
  title?: string
  metadata?: Record<string, unknown>
}): ArtifactManifest | null {
  const kind = inferKindFromEntry(input.entry)
  if (!kind) return null
  const lowerEntry = input.entry.toLowerCase()
  const isDeck =
    kind === 'html' &&
    (lowerEntry.includes('deck') ||
      lowerEntry.includes('slides') ||
      lowerEntry.includes('pitch'))
  const renderer: ArtifactRendererId = isDeck
    ? 'deck-html'
    : kind === 'html'
      ? 'html'
      : kind === 'markdown-document'
        ? 'markdown'
        : kind === 'react-component'
          ? 'react-component'
          : kind === 'code-snippet'
            ? 'code'
            : kind === 'svg'
              ? 'svg'
              : kind === 'diagram'
                ? 'diagram'
                : kind === 'mini-app'
                  ? 'mini-app'
                  : kind === 'design-system'
                    ? 'design-system'
                    : 'html' // fallback — shouldn't be reached, but type-narrows

  const resolvedKind = isDeck ? 'deck' : kind
  return {
    version: MANIFEST_VERSION,
    kind: resolvedKind,
    title: input.title || input.entry,
    entry: input.entry,
    renderer,
    exports: exportsForKind(resolvedKind),
    metadata: input.metadata,
  }
}
