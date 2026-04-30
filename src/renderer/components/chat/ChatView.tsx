import { useState, useEffect, useRef } from 'react'
import { useAppStore } from '../../stores/app-store'
import type { SessionMeta, TaskState, Conversation } from '../../stores/app-store'
import { cliApi } from '../../lib/cli-api'
import { MessageRenderer } from '@kangnam/chat-ui'
import { MarkdownPreview } from '../common/MarkdownPreview'
import { SafetyDialog } from './SafetyDialog'
import { TopBar } from './TopBar'
import { MessageInput } from './MessageInput'


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
