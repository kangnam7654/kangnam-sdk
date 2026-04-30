/**
 * Artifact metadata enum + manifest shape ported from open-design
 * `src/artifacts/types.ts` (Apache-2.0).
 *
 * The chat-server emits `artifact_end` with a parsed manifest blob;
 * the consumer (PreviewIframe in 5b-11) interprets `kind` + `renderer`
 * to pick a viewer. Export kinds drive the Phase 6a 4-format chip
 * choices.
 */
export type ArtifactKind =
  | 'html'
  | 'deck'
  | 'react-component'
  | 'markdown-document'
  | 'svg'
  | 'diagram'
  | 'code-snippet'
  | 'mini-app'
  | 'design-system'

export type ArtifactRendererId =
  | 'html'
  | 'deck-html'
  | 'react-component'
  | 'markdown'
  | 'svg'
  | 'diagram'
  | 'code'
  | 'mini-app'
  | 'design-system'

export type ArtifactExportKind =
  | 'html'
  | 'pdf'
  | 'zip'
  | 'pptx'
  | 'jsx'
  | 'md'
  | 'svg'
  | 'txt'

export interface ArtifactManifest {
  version: 1
  kind: ArtifactKind
  title: string
  entry: string
  renderer: ArtifactRendererId
  exports: ArtifactExportKind[]
  /** Reserved for future multi-file artifact packaging. */
  supportingFiles?: string[]
  createdAt?: string
  updatedAt?: string
  sourceSkillId?: string
  designSystemId?: string | null
  metadata?: Record<string, unknown>
}
