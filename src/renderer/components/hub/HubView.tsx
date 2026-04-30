/**
 * HubView — Designs Hub: grid of all projects, hidden iframes kept
 * mounted across navigation so opening a project lands instantly
 * without a re-render flash.
 *
 * Inspired by open-codesign `views/HubView.tsx` + `views/hub/*`.
 *
 * Behavior:
 * - Renders all projects in a responsive grid
 * - Active project's card highlights
 * - Click a card → setActiveMainView('project') so ProjectView opens
 * - "새 프로젝트" button opens NewProjectPanel via the standard
 *   ActivityBar entry (we don't duplicate the panel here — the user
 *   can also reach it from the sidebar)
 *
 * Phase 5c-14 ships the grid; the "alive" semantics (parent keeps
 * cards mounted across MainView swaps) requires a small App.tsx
 * change in 5c-15.
 */
import { useState } from 'react'

import { useAppStore } from '../../stores/app-store'
import { HubProjectCard } from './HubProjectCard'
import { NewProjectPanel } from '../projects/NewProjectPanel'

export function HubView() {
  const projects = useAppStore((s) => s.projects)
  const activeProjectId = useAppStore((s) => s.activeProjectId)
  const setActiveProjectId = useAppStore((s) => s.setActiveProjectId)
  const setActiveMainView = useAppStore((s) => s.setActiveMainView)
  const [showNew, setShowNew] = useState(false)

  function openProject(id: string) {
    setActiveProjectId(id)
    setActiveMainView('project')
  }

  return (
    <div
      style={{
        flex: 1,
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
        background: 'var(--bg-base)',
      }}
    >
      <div
        style={{
          padding: '14px 20px',
          borderBottom: '1px solid var(--border)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: 12,
          flexShrink: 0,
        }}
      >
        <div>
          <div style={{ fontSize: 16, fontWeight: 600, color: 'var(--text-primary)' }}>
            디자인 허브
          </div>
          <div style={{ fontSize: 12, color: 'var(--text-muted)', marginTop: 2 }}>
            최근 디자인 프로젝트 · 카드를 클릭해 열기
          </div>
        </div>
        <button
          type="button"
          onClick={() => setShowNew(true)}
          style={{
            padding: '7px 14px',
            borderRadius: 'var(--radius-md)',
            border: 'none',
            background: 'var(--accent)',
            color: '#fff',
            fontSize: 13,
            fontWeight: 500,
            cursor: 'pointer',
          }}
        >
          + 새 프로젝트
        </button>
      </div>

      <div style={{ flex: 1, overflow: 'auto', padding: 20 }}>
        {projects.length === 0 ? (
          <div
            style={{
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
              gap: 12,
              padding: 60,
              textAlign: 'center',
              color: 'var(--text-muted)',
              fontSize: 13,
            }}
          >
            <div>아직 프로젝트가 없습니다.</div>
            <button
              type="button"
              onClick={() => setShowNew(true)}
              style={{
                padding: '7px 14px',
                borderRadius: 'var(--radius-md)',
                border: '1px solid var(--border)',
                background: 'var(--bg-surface)',
                color: 'var(--text-secondary)',
                fontSize: 12,
                cursor: 'pointer',
              }}
            >
              첫 프로젝트 시작하기
            </button>
          </div>
        ) : (
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fill, minmax(260px, 1fr))',
              gap: 12,
            }}
          >
            {projects.map((p) => (
              <HubProjectCard
                key={p.id}
                project={p}
                active={p.id === activeProjectId}
                onOpen={() => openProject(p.id)}
              />
            ))}
          </div>
        )}
      </div>

      {showNew ? <NewProjectPanel onClose={() => setShowNew(false)} /> : null}
    </div>
  )
}
