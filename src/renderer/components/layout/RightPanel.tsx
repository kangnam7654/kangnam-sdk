import { useAppStore, type RightPanelTab } from '../../stores/app-store'
import { TaskPanel } from '../sidebar/TaskPanel'

const tabs: { id: RightPanelTab; label: string }[] = [
  { id: 'terminal', label: 'Terminal' },
  { id: 'agents', label: 'Agents' },
  { id: 'tasks', label: 'Tasks' },
  { id: 'files', label: 'Files' },
  { id: 'tools', label: 'Tools' },
]

export function RightPanel() {
  const { rightPanelVisible, rightPanelTab, setRightPanelTab, rightPanelWidth } = useAppStore()

  if (!rightPanelVisible) return null

  return (
    <div
      style={{
        width: rightPanelWidth,
        minWidth: rightPanelWidth,
        height: '100%',
        background: 'var(--bg-surface)',
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
        borderLeft: '1px solid var(--border)',
      }}
    >
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 0,
          borderBottom: '1px solid var(--border)',
          flexShrink: 0,
          paddingLeft: 8,
        }}
      >
        {tabs.map((tab) => {
          const isActive = rightPanelTab === tab.id
          return (
            <button
              key={tab.id}
              onClick={() => setRightPanelTab(tab.id)}
              style={{
                padding: '8px 12px',
                fontSize: 11,
                fontWeight: isActive ? 600 : 400,
                color: isActive ? 'var(--text-primary)' : 'var(--text-muted)',
                background: 'none',
                border: 'none',
                borderBottom: isActive ? '2px solid var(--accent)' : '2px solid transparent',
                cursor: 'pointer',
                transition: 'all 0.15s',
              }}
            >
              {tab.label}
            </button>
          )
        })}
      </div>

      <div style={{ flex: 1, overflow: 'auto' }}>
        {rightPanelTab === 'tasks' && <TaskPanel />}
        {rightPanelTab === 'terminal' && <Placeholder label="Terminal Log (Phase 4)" />}
        {rightPanelTab === 'agents' && <Placeholder label="Agent Tracker (Phase 4)" />}
        {rightPanelTab === 'files' && <Placeholder label="File Workspace (5c-09 lands tree+viewer)" />}
        {rightPanelTab === 'tools' && <Placeholder label="Tool Log (5c-11 lands ToolCard)" />}
      </div>
    </div>
  )
}

function Placeholder({ label }: { label: string }) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%', color: 'var(--text-muted)', fontSize: 12 }}>
      {label}
    </div>
  )
}
