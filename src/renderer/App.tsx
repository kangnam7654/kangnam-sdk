import { useEffect } from 'react'
import { useAppStore } from './stores/app-store'
import { cliApi } from './lib/cli-api'

import { ActivityBar } from './components/layout/ActivityBar'
import { SidePanel } from './components/layout/SidePanel'
import { RightPanel } from './components/layout/RightPanel'
import { ResizeHandle } from './components/layout/ResizeHandle'
import { StatusBar } from './components/layout/StatusBar'
import { ChatView } from './components/chat/ChatView'
import { StudioView } from './components/studio/StudioView'
import { SettingsPanel } from './components/settings/SettingsPanel'
import { SearchOverlay } from './components/sidebar/SearchPanel'

export default function App() {
  const {
    theme, currentProvider, setCurrentProvider, setSetupComplete,
    sidePanelVisible, sidePanelWidth, setSidePanelWidth,
    rightPanelVisible, rightPanelWidth, setRightPanelWidth,
    toggleSidePanel, toggleRightPanel, activeMainView,
  } = useAppStore()

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme)
  }, [theme])

  // Auto-detect CLI providers on startup
  useEffect(() => {
    if (currentProvider) return
    cliApi.listProviders()
      .then(async (metas) => {
        for (const meta of metas) {
          try {
            const status = await cliApi.checkInstalled(meta.name)
            if (status.installed) {
              setCurrentProvider(meta.name)
              setSetupComplete(true)
              return
            }
          } catch { /* ignore */ }
        }
        useAppStore.getState().setSettingsTab('providers')
        useAppStore.getState().setShowSettings(true)
      })
      .catch(() => {})
  }, [currentProvider, setCurrentProvider, setSetupComplete])

  // Keyboard shortcuts
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const meta = e.ctrlKey || e.metaKey
      if (meta && e.shiftKey && e.key === 'D') {
        e.preventDefault()
        useAppStore.getState().toggleDevMode()
      }
      if (meta && e.key === '\\') {
        e.preventDefault()
        toggleSidePanel()
      }
      if (meta && e.key === 'b') {
        e.preventDefault()
        toggleRightPanel()
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [toggleSidePanel, toggleRightPanel])

  const handleSidePanelResize = (delta: number) => {
    setSidePanelWidth(Math.min(400, Math.max(200, sidePanelWidth + delta)))
  }

  const handleRightPanelResize = (delta: number) => {
    setRightPanelWidth(Math.min(500, Math.max(260, rightPanelWidth + delta)))
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh', width: '100vw', overflow: 'hidden', background: 'var(--bg-main)' }}>
      {/* Main row: Activity Bar + Side Panel + Chat + Right Panel */}
      <div style={{ display: 'flex', flex: 1, minHeight: 0 }}>
        <ActivityBar />

        {sidePanelVisible && (
          <>
            <SidePanel />
            <ResizeHandle side="left" onResize={handleSidePanelResize} />
          </>
        )}

        {/* Main content area: Studio, Chat, or — once Phase 5b lands —
            Project / Hub. Unknown variants fall back to Chat so this
            switch is exhaustive without forcing the type. */}
        <div style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column' }}>
          {activeMainView === 'studio' ? (
            <StudioView />
          ) : activeMainView === 'project' || activeMainView === 'hub' ? (
            // Placeholders until Phase 5b/c land — render Chat so the
            // app stays functional if a stale localStorage value
            // selects one of the new variants.
            <ChatView />
          ) : (
            <ChatView />
          )}
        </div>

        {rightPanelVisible && (
          <>
            <ResizeHandle side="right" onResize={handleRightPanelResize} />
            <RightPanel />
          </>
        )}
      </div>

      {/* Status Bar */}
      <StatusBar />

      {/* Overlays */}
      <SettingsPanel />
      <SearchOverlay />
    </div>
  )
}
