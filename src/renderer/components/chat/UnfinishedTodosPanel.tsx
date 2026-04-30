/**
 * UnfinishedTodosPanel — surfaces in-flight TodoWrite items at the
 * top of the assistant pane so the user can see what the model
 * intends to do next without scrolling.
 *
 * Adapted from open-design `runtime/todos.ts` + the panel rendering
 * in `components/AssistantMessage.tsx` (Apache-2.0). The upstream
 * extraction logic walks the entire conversation looking for the
 * latest TodoWrite tool_use; we reproduce that here over the chat
 * slice's messages.
 *
 * Heuristic: scans messages newest-first for the most recent
 * `tool_use_input` (or `tool_use_start`) where the tool name matches
 * `TodoWrite` (case-insensitive) and the input has a `todos` array.
 * Each todo is rendered with its status icon.
 */
import { useMemo } from 'react'

import { useAppStore, type UnifiedMessage } from '../../stores/app-store'

interface Todo {
  content: string
  status: 'pending' | 'in_progress' | 'completed'
  activeForm?: string
}

function extractTodos(messages: UnifiedMessage[]): Todo[] | null {
  // Scan in reverse — only the latest TodoWrite is interesting; earlier
  // ones are stale once a new write supersedes them.
  for (let i = messages.length - 1; i >= 0; i--) {
    const m = messages[i]
    if (!m) continue
    if (m.type === 'tool_use_input' || m.type === 'tool_use_start') {
      const name = (m as { name?: string }).name
      const input = m.input
      if (!name || !name.toLowerCase().includes('todowrite')) continue
      if (typeof input !== 'object' || input === null) continue
      const obj = input as Record<string, unknown>
      const list = obj.todos
      if (!Array.isArray(list)) continue
      const todos: Todo[] = []
      for (const t of list) {
        if (typeof t !== 'object' || t === null) continue
        const r = t as Record<string, unknown>
        const content = typeof r.content === 'string' ? r.content : ''
        const status = r.status === 'in_progress' || r.status === 'completed' ? r.status : 'pending'
        const activeForm = typeof r.activeForm === 'string' ? r.activeForm : undefined
        if (content) todos.push({ content, status, activeForm })
      }
      return todos
    }
  }
  return null
}

export function UnfinishedTodosPanel() {
  const messages = useAppStore((s) => s.messages)
  const todos = useMemo(() => extractTodos(messages), [messages])

  if (!todos || todos.length === 0) return null
  const unfinished = todos.filter((t) => t.status !== 'completed')
  if (unfinished.length === 0) return null

  return (
    <div
      style={{
        margin: '8px 0',
        padding: '10px 12px',
        background: 'var(--bg-surface)',
        border: '1px solid var(--border)',
        borderRadius: 'var(--radius-md)',
      }}
    >
      <div
        style={{
          fontSize: 10,
          fontWeight: 600,
          color: 'var(--text-muted)',
          textTransform: 'uppercase',
          letterSpacing: '0.06em',
          marginBottom: 6,
        }}
      >
        Unfinished todos · {unfinished.length}
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
        {todos.map((t, i) => (
          <Row key={i} todo={t} />
        ))}
      </div>
    </div>
  )
}

function Row({ todo }: { todo: Todo }) {
  const icon =
    todo.status === 'completed'
      ? '✔'
      : todo.status === 'in_progress'
        ? '◐'
        : '○'
  const color =
    todo.status === 'completed'
      ? 'var(--text-muted)'
      : todo.status === 'in_progress'
        ? 'var(--accent)'
        : 'var(--text-secondary)'
  const label = todo.status === 'in_progress' && todo.activeForm ? todo.activeForm : todo.content
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'flex-start',
        gap: 8,
        fontSize: 12,
        color,
        textDecoration: todo.status === 'completed' ? 'line-through' : 'none',
      }}
    >
      <span
        style={{
          fontSize: 10,
          width: 14,
          flexShrink: 0,
          textAlign: 'center',
          marginTop: 2,
        }}
      >
        {icon}
      </span>
      <span style={{ flex: 1, lineHeight: 1.45 }}>{label}</span>
    </div>
  )
}
