import { useState, useEffect, useRef } from 'react'
import { useAppStore } from '../../stores/app-store'
import type { SessionMeta, TaskState, Conversation } from '../../stores/app-store'
import { cliApi } from '../../lib/cli-api'
import { FALLBACK_MODELS } from '../../lib/models/registry'
import { MessageRenderer } from '@kangnam/chat-ui'
import { MarkdownPreview } from '../common/MarkdownPreview'
import { SafetyDialog } from './SafetyDialog'

// Temporary alias preserved so the rest of TopBar reads identically.
// Phase 5a-03 swaps this for `useAvailableModels()`.
const AVAILABLE_MODELS = FALLBACK_MODELS

type EffortLevel = 'low' | 'medium' | 'high'

const EFFORT_OPTIONS: { value: EffortLevel; label: string; desc: string }[] = [
  { value: 'low', label: 'Low', desc: '빠른 응답' },
  { value: 'medium', label: 'Medium', desc: '균형' },
  { value: 'high', label: 'High', desc: '깊은 사고' },
]

function TopBar() {
  const {
    currentProvider, currentWorkingDir, currentSessionId, clearMessages,
    setCurrentSessionId, isStreaming, sessionMeta, selectedModel, setSelectedModel,
  } = useAppStore()
  const [modelDropdownOpen, setModelDropdownOpen] = useState(false)
  const modelDropdownRef = useRef<HTMLDivElement>(null)

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
                  {AVAILABLE_MODELS.map((m) => {
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

function MessageInput() {
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

function StreamingIndicator() {
  return (
    <div className="flex items-center gap-1 py-3">
      <span className="h-1.5 w-1.5 rounded-full bg-[var(--text-muted)] animate-bounce" style={{ animationDelay: '0ms' }} />
      <span className="h-1.5 w-1.5 rounded-full bg-[var(--text-muted)] animate-bounce" style={{ animationDelay: '150ms' }} />
      <span className="h-1.5 w-1.5 rounded-full bg-[var(--text-muted)] animate-bounce" style={{ animationDelay: '300ms' }} />
    </div>
  )
}

function ChatContent() {
  const { messages, addMessage, setPendingPermission, currentSessionId, setCurrentSessionId,
          setIsStreaming, isStreaming, currentProvider, setCurrentWorkingDir,
          setSessionMeta, addTask, updateTask, setRateLimit, setSessionCost, selectedModel } = useAppStore()
  const messagesEndRef = useRef<HTMLDivElement>(null)
  const startingSession = useRef(false)

  // Auto-start session when provider is set but no session exists
  useEffect(() => {
    if (currentSessionId || !currentProvider || startingSession.current) return
    startingSession.current = true

    const lastDir = localStorage.getItem('kangnam-last-workdir')
    const workingDir = lastDir || (navigator.platform?.includes('Win') ? 'C:\\Users' : '/Users')

    cliApi.startSession(currentProvider, workingDir, selectedModel)
      .then(async (sessionId) => {
        setCurrentSessionId(sessionId)
        setCurrentWorkingDir(workingDir)
        localStorage.setItem('kangnam-last-workdir', workingDir)
        // Refresh conversation list
        try {
          const convs = await window.api.conv.list()
          useAppStore.getState().setConversations(convs as Conversation[])
        } catch { /* ignore */ }
      })
      .catch((e) => {
        console.error('[ChatContent] auto-start session failed:', e)
      })
      .finally(() => {
        startingSession.current = false
      })
  }, [currentSessionId, currentProvider, setCurrentSessionId, setCurrentWorkingDir, selectedModel])

  useEffect(() => {
    const unlisten = cliApi.onMessage((msg) => {
      if (msg.type === 'turn_end') {
        setIsStreaming(false)
        addMessage(msg)
      } else if (msg.type === 'error') {
        setIsStreaming(false)
        addMessage(msg)
      } else {
        addMessage(msg)
      }
    })
    return unlisten
  }, [addMessage, setIsStreaming])

  useEffect(() => {
    const unlisten = cliApi.onPermissionRequest((req) => {
      setPendingPermission({
        type: 'permission_request',
        id: req.id,
        tool: req.tool,
        description: req.description,
      })
    })
    return unlisten
  }, [setPendingPermission])

  useEffect(() => {
    const unlisten = cliApi.onEnhanced((event) => {
      const type = event.type as string
      switch (type) {
        case 'session_meta':
          setSessionMeta(event as unknown as SessionMeta)
          break
        case 'task_started':
          addTask({
            task_id: event.task_id as string,
            description: event.description as string,
            task_type: event.task_type as string,
            status: 'running',
          })
          break
        case 'task_progress':
          updateTask(event.task_id as string, {
            description: event.description as string,
          })
          break
        case 'task_notification':
          updateTask(event.task_id as string, {
            status: event.status as TaskState['status'],
            summary: event.summary as string | undefined,
          })
          break
        case 'result_summary':
          setSessionCost({
            cost_usd: event.cost_usd as number | null,
            duration_ms: event.duration_ms as number | null,
            num_turns: event.num_turns as number | null,
          })
          break
        case 'rate_limit':
          console.log('[rate_limit]', JSON.stringify(event))
          setRateLimit({
            status: event.status as string,
            utilization: event.utilization as number | null,
            rate_limit_type: event.rate_limit_type as string,
          })
          break
      }
    })
    return unlisten
  }, [setSessionMeta, addTask, updateTask, setRateLimit, setSessionCost])

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  return (
    <div className="flex-1 flex flex-col min-w-0 min-h-0">
      <TopBar />

      <div className="flex-1 overflow-y-auto px-4 py-4">
        <div style={{ maxWidth: '48rem', margin: '0 auto' }}>
          {messages.length === 0 && !isStreaming ? (
            <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 8, paddingTop: '30vh' }}>
              <div style={{ width: 40, height: 40, borderRadius: 'var(--radius-lg)', display: 'flex', alignItems: 'center', justifyContent: 'center', color: '#fff', fontWeight: 700, fontSize: 18, background: 'var(--accent)' }}>K</div>
              <div style={{ fontSize: 18, fontWeight: 600, color: 'var(--text-primary)', marginTop: 8 }}>
                {currentSessionId ? '무엇을 도와드릴까요?' : 'Claude Code 시작 중'}
              </div>
              {!currentSessionId && (
                <div style={{ display: 'flex', alignItems: 'center', gap: 8, color: 'var(--text-muted)', fontSize: 13 }}>
                  <span style={{ display: 'inline-block', width: 12, height: 12, border: '2px solid var(--text-muted)', borderTopColor: 'var(--accent)', borderRadius: '50%', animation: 'spin 0.8s linear infinite' }} />
                  세션 초기화 중... (hooks, plugins, MCP 로드)
                </div>
              )}
              {currentSessionId && (
                <div style={{ color: 'var(--text-muted)', fontSize: 13 }}>
                  Claude Code가 준비되었습니다
                </div>
              )}
            </div>
          ) : (
            <>
              {messages.map((msg, i) => <MessageRenderer key={i} message={msg} isLast={i === messages.length - 1} isStreaming={isStreaming} renderMarkdown={(content) => <MarkdownPreview content={content} />} />)}
              {isStreaming && messages[messages.length - 1]?.type !== 'text_delta' && messages[messages.length - 1]?.type !== 'agent_progress' && (
                <StreamingIndicator />
              )}
            </>
          )}
          <div ref={messagesEndRef} />
        </div>
      </div>
      <MessageInput />

      <SafetyDialog />
    </div>
  )
}

export function ChatView() {
  return <ChatContent />
}
