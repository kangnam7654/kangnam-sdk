/**
 * Per-project workspace — chat on the left, preview iframe on the right.
 *
 * v1 layout (Phase 5b-12):
 * - Chat pane: full <ChatContent>
 * - Preview pane: <PreviewIframe artifactId={latest}>
 *
 * Phase 5c (5c-FileWorkspace) will widen this into a 3-pane layout
 * with the FileWorkspace tree on the right and the iframe inset.
 */
import { useMemo } from 'react'

import { useAppStore } from '../../stores/app-store'
import { ChatContent } from '../chat/ChatContent'
import { PreviewIframe } from '../preview/PreviewIframe'

export function ProjectView() {
  const activeProjectId = useAppStore((s) => s.activeProjectId)
  const projects = useAppStore((s) => s.projects)
  const artifacts = useAppStore((s) => s.artifacts)

  const project = useMemo(
    () => projects.find((p) => p.id === activeProjectId) ?? null,
    [projects, activeProjectId],
  )

  // Pick the most recently started artifact to surface in the preview.
  // Phase 5c lets the user pin / cycle through; this is "show me the
  // latest" v1 behavior.
  const latestArtifactId = useMemo(() => {
    const ids = Object.keys(artifacts)
    if (ids.length === 0) return undefined
    return ids.reduce((best, id) => {
      const a = artifacts[id]
      const b = artifacts[best]
      if (!a) return best
      if (!b) return id
      return a.startedAt > b.startedAt ? id : best
    }, ids[0])
  }, [artifacts])

  if (!project) {
    return (
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          height: '100%',
          color: 'var(--text-muted)',
          fontSize: 13,
        }}
      >
        프로젝트를 선택하거나 새로 만들어주세요
      </div>
    )
  }

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'row',
        height: '100%',
        minHeight: 0,
      }}
    >
      <div
        style={{
          flex: '1 1 50%',
          minWidth: 0,
          display: 'flex',
          flexDirection: 'column',
          borderRight: '1px solid var(--border)',
        }}
      >
        <ChatContent />
      </div>
      <div
        style={{
          flex: '1 1 50%',
          minWidth: 0,
          padding: 12,
          background: 'var(--bg-base)',
          display: 'flex',
          flexDirection: 'column',
        }}
      >
        <div style={{ flex: 1, minHeight: 0 }}>
          <PreviewIframe artifactId={latestArtifactId} />
        </div>
      </div>
    </div>
  )
}
