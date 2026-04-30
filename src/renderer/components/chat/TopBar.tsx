import { useState, useEffect, useRef } from 'react'
import { useAppStore } from '../../stores/app-store'
import { cliApi } from '../../lib/cli-api'
import { useAvailableModels } from '../../hooks/useAvailableModels'

/**
 * Top bar for the chat pane: New Chat button, provider/model/cwd
 * indicator with model picker dropdown, and a settings cog stand-in.
 *
 * Extracted verbatim from `ChatView.tsx` in Phase 5a-05 — behavior is
 * identical to the previous in-file definition. Owns its own dropdown
 * outside-click handler so adding to / removing from this component
 * doesn't ripple into the rest of the chat pane.
 */
export function TopBar() {
  const {
    currentProvider, currentWorkingDir, currentSessionId, clearMessages,
    setCurrentSessionId, isStreaming, sessionMeta, selectedModel, setSelectedModel,
  } = useAppStore()
  const [modelDropdownOpen, setModelDropdownOpen] = useState(false)
  const modelDropdownRef = useRef<HTMLDivElement>(null)
  const availableModels = useAvailableModels()

  const handleNewChat = async () => {
    if (currentSessionId) {
      try { await cliApi.stopSession(currentSessionId) } catch { /* ignore */ }
    }
    clearMessages()
    setCurrentSessionId(null)
    useAppStore.getState().setIsStreaming(false)
    useAppStore.getState().setCurrentWorkingDir(null)
  }

  const handleModelSelect = async (modelId: string) => {
    setModelDropdownOpen(false)
    if (modelId === selectedModel) return
    setSelectedModel(modelId)
    // Restart session with new model
    if (currentSessionId) {
      try { await cliApi.stopSession(currentSessionId) } catch { /* ignore */ }
    }
    clearMessages()
    setCurrentSessionId(null)
    useAppStore.getState().setIsStreaming(false)
  }

  // Close dropdown on outside click
  useEffect(() => {
    if (!modelDropdownOpen) return
    const handler = (e: MouseEvent) => {
      if (modelDropdownRef.current && !modelDropdownRef.current.contains(e.target as Node)) {
        setModelDropdownOpen(false)
      }
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [modelDropdownOpen])

  const dirName = currentWorkingDir?.split('/').pop() || currentWorkingDir
  const displayModel = sessionMeta?.model ?? selectedModel

  return (
    <div className="drag-region h-12 flex items-center justify-between shrink-0 relative px-4">
      {/* Left: New Chat */}
      <button
        onClick={handleNewChat}
        className="no-drag flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-xs text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] transition-colors"
        aria-label="New chat"
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" />
        </svg>
        New
      </button>

      {/* Center: Provider + Model (clickable) + Dir */}
      <div className="no-drag flex items-center gap-2 cursor-default">
        {currentProvider && (
          <span className="text-xs font-medium text-[var(--text-tertiary)] uppercase">{currentProvider}</span>
        )}
        {displayModel && (
          <>
            <span className="text-[var(--text-muted)]">/</span>
            <div ref={modelDropdownRef} className="relative">
              <button
                onClick={() => setModelDropdownOpen((v) => !v)}
                className="text-xs text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-hover)] px-1.5 py-0.5 rounded transition-colors flex items-center gap-1"
                title="Change model (restarts session)"
              >
                {displayModel}
                <svg width="8" height="8" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <polyline points="6 9 12 15 18 9" />
                </svg>
              </button>
              {modelDropdownOpen && (
                <div
                  style={{
                    position: 'absolute',
                    top: 'calc(100% + 6px)',
                    left: '50%',
                    transform: 'translateX(-50%)',
                    background: 'var(--bg-surface)',
                    border: '1px solid var(--border)',
                    borderRadius: 'var(--radius-md)',
                    boxShadow: '0 4px 16px rgba(0,0,0,0.2)',
                    minWidth: 220,
                    zIndex: 100,
                    overflow: 'hidden',
                  }}
                >
                  <div style={{ padding: '6px 10px 4px', fontSize: 10, color: 'var(--text-muted)', textTransform: 'uppercase', letterSpacing: '0.08em' }}>
                    Model (restarts session)
                  </div>
                  {availableModels.map((m) => {
                    const isActive = selectedModel === m.id || (!selectedModel && sessionMeta?.model === m.id)
                    return (
                      <button
                        key={m.id}
                        onClick={() => handleModelSelect(m.id)}
                        style={{
                          display: 'flex',
                          alignItems: 'center',
                          gap: 8,
                          width: '100%',
                          padding: '7px 12px',
                          background: 'none',
                          border: 'none',
                          color: 'var(--text-primary)',
                          fontSize: 13,
                          cursor: 'pointer',
                          textAlign: 'left',
                          transition: 'background 0.1s',
                        }}
                        onMouseEnter={(e) => { (e.currentTarget as HTMLElement).style.background = 'var(--bg-hover)' }}
                        onMouseLeave={(e) => { (e.currentTarget as HTMLElement).style.background = 'none' }}
                      >
                        <span style={{ flex: 1 }}>{m.label}</span>
                        <span style={{ fontSize: 10, color: 'var(--text-muted)' }}>{m.id}</span>
                        {isActive && (
                          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                            <polyline points="20 6 9 17 4 12" />
                          </svg>
                        )}
                      </button>
                    )
                  })}
                </div>
              )}
            </div>
          </>
        )}
        {dirName && (
          <>
            <span className="text-[var(--text-muted)]">/</span>
            <span className="text-xs text-[var(--text-secondary)]" title={currentWorkingDir ?? ''}>{dirName}</span>
          </>
        )}
        {isStreaming && (
          <span className="ml-1 inline-block h-2 w-2 animate-pulse rounded-full bg-green-400" title="Streaming" />
        )}
      </div>

      {/* Right: Settings */}
      <button
        onClick={() => useAppStore.getState().setShowSettings(true)}
        className="no-drag w-7 h-7 rounded-full bg-[var(--accent)] flex items-center justify-center text-white text-[11px] font-semibold cursor-pointer hover:opacity-85 transition-opacity border-none"
        aria-label="Settings"
      >
        U
      </button>
    </div>
  )
}
