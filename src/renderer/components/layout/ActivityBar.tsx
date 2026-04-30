import { useAppStore, type SidePanelTab, type MainView } from '../../stores/app-store'

const tabs: { id: SidePanelTab; label: string; icon: React.ReactNode }[] = [
  {
    id: 'chats',
    label: 'Chats',
    icon: (
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
        <path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z" />
      </svg>
    ),
  },
  {
    id: 'files',
    label: 'Files',
    icon: (
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
        <path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z" />
      </svg>
    ),
  },
  {
    id: 'skills',
    label: 'Skills',
    icon: (
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
        <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
      </svg>
    ),
  },
  {
    id: 'agents',
    label: 'Agents',
    icon: (
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
        <path d="M17 21v-2a4 4 0 00-4-4H5a4 4 0 00-4 4v2" />
        <circle cx="9" cy="7" r="4" />
        <path d="M23 21v-2a4 4 0 00-3-3.87" />
        <path d="M16 3.13a4 4 0 010 7.75" />
      </svg>
    ),
  },
  {
    id: 'mcp',
    label: 'MCP',
    icon: (
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
        <rect x="2" y="2" width="20" height="8" rx="2" ry="2" />
        <rect x="2" y="14" width="20" height="8" rx="2" ry="2" />
        <line x1="6" y1="6" x2="6.01" y2="6" />
        <line x1="6" y1="18" x2="6.01" y2="18" />
      </svg>
    ),
  },
]

const studioTab = {
  id: 'studio' as const,
  label: 'Studio',
  icon: (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 20h9" />
      <path d="M16.5 3.5a2.121 2.121 0 013 3L7 19l-4 1 1-4L16.5 3.5z" />
    </svg>
  ),
}

const projectTab = {
  id: 'project' as const,
  label: 'Project (Design)',
  icon: (
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 3h7v7H3z" />
      <path d="M14 3h7v7h-7z" />
      <path d="M14 14h7v7h-7z" />
      <path d="M3 14h7v7H3z" />
    </svg>
  ),
}

export function ActivityBar() {
  const { sidePanelTab, sidePanelVisible, toggleSidePanel, setShowSettings, activeMainView, setActiveMainView } = useAppStore()

  return (
    <div
      style={{
        width: 'var(--activity-bar-width)',
        minWidth: 'var(--activity-bar-width)',
        background: 'var(--bg-activity-bar)',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        paddingTop: 12,
        gap: 4,
        borderRight: '1px solid var(--border)',
      }}
    >
      <div className="drag-region" style={{ height: 32, width: '100%', flexShrink: 0 }} />

      {tabs.map((tab) => {
        const isActive = sidePanelVisible && sidePanelTab === tab.id
        return (
          <button
            key={tab.id}
            onClick={() => toggleSidePanel(tab.id)}
            title={tab.label}
            aria-label={tab.label}
            className="no-drag"
            style={{
              width: 36,
              height: 36,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              borderRadius: 'var(--radius-md)',
              border: 'none',
              background: isActive ? 'var(--accent-soft)' : 'transparent',
              color: isActive ? 'var(--activity-icon-active)' : 'var(--activity-icon)',
              cursor: 'pointer',
              transition: 'all 0.15s',
            }}
          >
            {tab.icon}
          </button>
        )
      })}

      {/* Separator */}
      <div style={{ width: 24, height: 1, background: 'var(--border)', margin: '4px 0' }} />

      {/* Studio tab — switches main view, not side panel */}
      <button
        onClick={() => setActiveMainView(activeMainView === 'studio' ? 'chat' : 'studio')}
        title={studioTab.label}
        aria-label={studioTab.label}
        className="no-drag"
        style={{
          width: 36,
          height: 36,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          borderRadius: 'var(--radius-md)',
          border: 'none',
          background: activeMainView === 'studio' ? 'var(--accent-soft)' : 'transparent',
          color: activeMainView === 'studio' ? 'var(--activity-icon-active)' : 'var(--activity-icon)',
          cursor: 'pointer',
          transition: 'all 0.15s',
        }}
      >
        {studioTab.icon}
      </button>

      {/* Design Project tab — Phase 5b. Click switches MainView to
          'project'; the user opens the new-project modal via the empty
          state inside ProjectView (Phase 5b-13). */}
      <button
        onClick={() => {
          if (activeMainView === 'project') {
            setActiveMainView('chat')
            return
          }
          setActiveMainView('project')
          // Auto-open the new-project modal when there's no active
          // project yet so the empty state has an explicit action.
          const w = window as unknown as { __openNewProject?: () => void }
          if (!useAppStore.getState().activeProjectId && w.__openNewProject) {
            w.__openNewProject()
          }
        }}
        title={projectTab.label}
        aria-label={projectTab.label}
        className="no-drag"
        style={{
          width: 36,
          height: 36,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          borderRadius: 'var(--radius-md)',
          border: 'none',
          background: activeMainView === 'project' ? 'var(--accent-soft)' : 'transparent',
          color:
            activeMainView === 'project'
              ? 'var(--activity-icon-active)'
              : 'var(--activity-icon)',
          cursor: 'pointer',
          transition: 'all 0.15s',
        }}
      >
        {projectTab.icon}
      </button>

      <div style={{ flex: 1 }} />

      <button
        onClick={() => setShowSettings(true)}
        title="Settings"
        aria-label="Settings"
        className="no-drag"
        style={{
          width: 36,
          height: 36,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          borderRadius: 'var(--radius-md)',
          border: 'none',
          background: 'transparent',
          color: 'var(--activity-icon)',
          cursor: 'pointer',
          marginBottom: 8,
          transition: 'all 0.15s',
        }}
      >
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="3" />
          <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z" />
        </svg>
      </button>
    </div>
  )
}
