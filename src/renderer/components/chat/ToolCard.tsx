/**
 * ToolCard — inline rendering of a tool_use_start / tool_use_input /
 * tool_result triplet. Shows the tool name, parameter JSON (collapsed),
 * and a status row (running / completed / error).
 *
 * Adapted from open-design `components/ToolCard.tsx` (Apache-2.0). The
 * upstream version pulls structured tool metadata from a registry; we
 * keep it generic here — any tool name renders fine, design-tools just
 * happen to have nicer names.
 */
import { useState } from 'react'

interface Props {
  toolName: string
  input?: unknown
  /** Output text from the tool — undefined while still running. */
  output?: string
  /** When true, output is an error string and is highlighted red. */
  isError?: boolean
  /** Show as expanded by default. */
  defaultOpen?: boolean
}

export function ToolCard({
  toolName,
  input,
  output,
  isError = false,
  defaultOpen = false,
}: Props) {
  const [open, setOpen] = useState(defaultOpen || isError)
  const status: 'running' | 'done' | 'error' = isError
    ? 'error'
    : output !== undefined
      ? 'done'
      : 'running'

  const accent =
    status === 'error'
      ? 'var(--danger)'
      : status === 'running'
        ? 'var(--accent)'
        : 'var(--text-muted)'

  return (
    <div
      style={{
        border: '1px solid var(--border)',
        borderLeft: `2px solid ${accent}`,
        borderRadius: 'var(--radius-md)',
        margin: '6px 0',
        background: 'var(--bg-elevated)',
        overflow: 'hidden',
      }}
    >
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        style={{
          display: 'flex',
          alignItems: 'center',
          width: '100%',
          padding: '6px 10px',
          background: 'transparent',
          border: 'none',
          cursor: 'pointer',
          color: 'var(--text-primary)',
          fontSize: 12,
          gap: 8,
        }}
      >
        <span
          style={{
            width: 8,
            height: 8,
            borderRadius: '50%',
            background: accent,
            flexShrink: 0,
          }}
        />
        <span style={{ fontWeight: 500, fontFamily: 'monospace' }}>{toolName}</span>
        <span style={{ color: 'var(--text-muted)', fontSize: 11, marginLeft: 'auto' }}>
          {status === 'running' ? '실행 중…' : status === 'error' ? '오류' : '완료'} · {open ? '▾' : '▸'}
        </span>
      </button>
      {open ? (
        <div
          style={{
            padding: '4px 10px 10px',
            display: 'flex',
            flexDirection: 'column',
            gap: 6,
            fontSize: 11,
          }}
        >
          {input !== undefined ? (
            <Block label="input">
              <pre style={preStyle}>{stringifySafe(input)}</pre>
            </Block>
          ) : null}
          {output !== undefined ? (
            <Block label={isError ? 'error' : 'output'}>
              <pre
                style={{
                  ...preStyle,
                  color: isError ? 'var(--danger)' : 'var(--text-primary)',
                }}
              >
                {output}
              </pre>
            </Block>
          ) : null}
        </div>
      ) : null}
    </div>
  )
}

function Block({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <div
        style={{
          fontSize: 9,
          fontWeight: 600,
          color: 'var(--text-muted)',
          textTransform: 'uppercase',
          letterSpacing: '0.06em',
          marginBottom: 2,
        }}
      >
        {label}
      </div>
      {children}
    </div>
  )
}

const preStyle: React.CSSProperties = {
  margin: 0,
  fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
  fontSize: 11,
  background: 'var(--bg-surface)',
  padding: 8,
  borderRadius: 6,
  border: '1px solid var(--border)',
  whiteSpace: 'pre-wrap',
  wordBreak: 'break-word',
  maxHeight: 220,
  overflow: 'auto',
}

function stringifySafe(value: unknown): string {
  try {
    if (typeof value === 'string') return value
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}
