import { useState, useEffect, useRef } from 'react'
import { useAppStore } from '../../stores/app-store'
import { cliApi } from '../../lib/cli-api'

/**
 * Composer for outgoing chat messages — auto-resizing textarea, an
 * effort-level dropdown that maps to `/effort`, and a Send/Stop
 * toggle that flips into a stop control while streaming.
 *
 * Extracted verbatim from `ChatView.tsx` in Phase 5a-06.
 */
type EffortLevel = 'low' | 'medium' | 'high'

const EFFORT_OPTIONS: { value: EffortLevel; label: string; desc: string }[] = [
  { value: 'low', label: 'Low', desc: '빠른 응답' },
  { value: 'medium', label: 'Medium', desc: '균형' },
  { value: 'high', label: 'High', desc: '깊은 사고' },
]

export function MessageInput() {
  const [text, setText] = useState('')
  const [effortLevel, setEffortLevel] = useState<EffortLevel>('high')
  const [effortDropdownOpen, setEffortDropdownOpen] = useState(false)
  const { currentSessionId, addMessage, isStreaming, setIsStreaming } = useAppStore()
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const effortDropdownRef = useRef<HTMLDivElement>(null)
  const effortApplied = useRef<EffortLevel>('high')

  const handleStop = async () => {
    if (!currentSessionId) return
    try { await cliApi.stopSession(currentSessionId) } catch { /* ignore */ }
    setIsStreaming(false)
  }

  const handleSubmit = async () => {
    if (isStreaming) {
      handleStop()
      return
    }
    const trimmed = text.trim()
    if (!trimmed) return

    // Wait for session if not ready yet
    const sessionId = useAppStore.getState().currentSessionId
    if (!sessionId) {
      addMessage({ type: 'error', message: '세션 준비 중입니다. 잠시 후 다시 시도해주세요.' })
      return
    }

    // Send /effort command if level changed since last apply
    if (effortLevel !== effortApplied.current) {
      try {
        await cliApi.sendMessage(sessionId, `/effort ${effortLevel}`)
        effortApplied.current = effortLevel
      } catch { /* ignore */ }
    }

    setText('')
    addMessage({ type: 'user_message', text: trimmed })
    setIsStreaming(true)

    try {
      await cliApi.sendMessage(sessionId, trimmed)
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)
      addMessage({ type: 'error', message: `메시지 전송 실패: ${msg}` })
      setIsStreaming(false)
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleSubmit()
    }
  }

  // Auto-resize textarea
  useEffect(() => {
    const el = textareaRef.current
    if (!el) return
    el.style.height = 'auto'
    el.style.height = `${Math.min(el.scrollHeight, 200)}px`
  }, [text])

  // Close effort dropdown on outside click
  useEffect(() => {
    if (!effortDropdownOpen) return
    const handler = (e: MouseEvent) => {
      if (effortDropdownRef.current && !effortDropdownRef.current.contains(e.target as Node)) {
        setEffortDropdownOpen(false)
      }
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [effortDropdownOpen])

  const currentEffort = EFFORT_OPTIONS.find((o) => o.value === effortLevel)!

  return (
    <div style={{ flexShrink: 0, padding: '8px 16px 12px' }}>
      <div
        style={{
          background: 'var(--bg-surface)',
          border: '1px solid var(--border)',
          borderRadius: 'var(--radius-lg)',
          padding: '10px 12px',
          display: 'flex',
          alignItems: 'flex-end',
          gap: 8,
          transition: 'border-color 0.15s',
        }}
        onFocus={(e) => { (e.currentTarget as HTMLElement).style.borderColor = 'var(--accent)' }}
        onBlur={(e) => { (e.currentTarget as HTMLElement).style.borderColor = 'var(--border)' }}
      >
        <textarea
          ref={textareaRef}
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder={currentSessionId ? 'Message Claude...' : 'Preparing session — type ahead...'}
          rows={1}
          style={{
            flex: 1,
            resize: 'none',
            background: 'transparent',
            border: 'none',
            outline: 'none',
            color: 'var(--text-primary)',
            fontSize: 13,
            fontFamily: 'var(--font-sans)',
            lineHeight: 1.5,
            maxHeight: 160,
            padding: '2px 0',
          }}
          className="composer-input placeholder:text-[var(--text-muted)] disabled:opacity-40"
          aria-label="Message input"
        />

        {/* Effort level selector */}
        <div ref={effortDropdownRef} style={{ position: 'relative', flexShrink: 0 }}>
          <button
            onClick={() => setEffortDropdownOpen((v) => !v)}
            title="Effort level"
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 3,
              height: 28,
              padding: '0 7px',
              borderRadius: 'var(--radius-md)',
              border: '1px solid var(--border)',
              background: effortLevel === 'high' ? 'var(--accent)' : effortLevel === 'medium' ? 'var(--accent-soft)' : 'transparent',
              color: effortLevel === 'high' ? '#fff' : effortLevel === 'medium' ? 'var(--accent)' : 'var(--text-muted)',
              fontSize: 11,
              cursor: 'pointer',
              transition: 'background 0.15s, color 0.15s',
              whiteSpace: 'nowrap',
            }}
          >
            <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
            </svg>
            {currentEffort.label}
          </button>
          {effortDropdownOpen && (
            <div
              style={{
                position: 'absolute',
                bottom: 'calc(100% + 6px)',
                right: 0,
                background: 'var(--bg-surface)',
                border: '1px solid var(--border)',
                borderRadius: 'var(--radius-md)',
                boxShadow: '0 4px 16px rgba(0,0,0,0.2)',
                minWidth: 160,
                zIndex: 100,
                overflow: 'hidden',
              }}
            >
              <div style={{ padding: '6px 10px 4px', fontSize: 10, color: 'var(--text-muted)', textTransform: 'uppercase', letterSpacing: '0.08em' }}>
                Effort (/effort)
              </div>
              {EFFORT_OPTIONS.map((opt) => (
                <button
                  key={opt.value}
                  onClick={() => { setEffortLevel(opt.value); setEffortDropdownOpen(false) }}
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
                  <span style={{ flex: 1 }}>{opt.label}</span>
                  <span style={{ fontSize: 10, color: 'var(--text-muted)' }}>{opt.desc}</span>
                  {effortLevel === opt.value && (
                    <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                      <polyline points="20 6 9 17 4 12" />
                    </svg>
                  )}
                </button>
              ))}
            </div>
          )}
        </div>

        {/* Send / Stop button */}
        <button
          onClick={handleSubmit}
          disabled={!isStreaming && (!text.trim() || !currentSessionId)}
          aria-label={isStreaming ? 'Stop' : 'Send'}
          style={{
            flexShrink: 0,
            width: 28,
            height: 28,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            borderRadius: 'var(--radius-md)',
            border: 'none',
            background: isStreaming ? 'var(--danger)' : 'var(--accent)',
            color: '#fff',
            cursor: 'pointer',
            opacity: !isStreaming && (!text.trim() || !currentSessionId) ? 0.3 : 1,
            transition: 'opacity 0.15s, background 0.15s',
          }}
        >
          {isStreaming ? (
            <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor">
              <rect x="4" y="4" width="16" height="16" rx="2" />
            </svg>
          ) : (
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M22 2L11 13" /><path d="M22 2L15 22L11 13L2 9L22 2Z" />
            </svg>
          )}
        </button>
      </div>
    </div>
  )
}
