/**
 * ToolLogPanel — aggregates all tool_use_start / tool_use_input /
 * tool_result events from the active chat into a scrollable list of
 * ToolCards. Powers the RightPanel "tools" tab (Phase 5c-12).
 */
import { useMemo } from 'react'

import { useAppStore, type UnifiedMessage } from '../../stores/app-store'
import { ToolCard } from './ToolCard'

interface ToolEntry {
  id: string
  name: string
  input: unknown
  output?: string
  isError?: boolean
}

function aggregateTools(messages: UnifiedMessage[]): ToolEntry[] {
  const byId = new Map<string, ToolEntry>()
  for (const m of messages) {
    if (m.type === 'tool_use_start') {
      byId.set(m.id, { id: m.id, name: m.name, input: m.input })
    } else if (m.type === 'tool_use_input') {
      const cur = byId.get(m.id)
      if (cur) cur.input = m.input
    } else if (m.type === 'tool_result') {
      const cur = byId.get(m.id)
      if (cur) {
        cur.output = m.output
        cur.isError = m.is_error
      } else {
        // tool_result without preceding tool_use — show as standalone
        byId.set(m.id, {
          id: m.id,
          name: '(tool)',
          input: undefined,
          output: m.output,
          isError: m.is_error,
        })
      }
    }
  }
  return Array.from(byId.values())
}

export function ToolLogPanel() {
  const messages = useAppStore((s) => s.messages)
  const entries = useMemo(() => aggregateTools(messages), [messages])

  if (entries.length === 0) {
    return (
      <div
        style={{
          padding: 12,
          color: 'var(--text-muted)',
          fontSize: 12,
          textAlign: 'center',
        }}
      >
        아직 호출된 도구가 없습니다.
      </div>
    )
  }

  return (
    <div style={{ padding: 8, display: 'flex', flexDirection: 'column' }}>
      {entries.map((e) => (
        <ToolCard
          key={e.id}
          toolName={e.name}
          input={e.input}
          output={e.output}
          isError={e.isError}
        />
      ))}
    </div>
  )
}
