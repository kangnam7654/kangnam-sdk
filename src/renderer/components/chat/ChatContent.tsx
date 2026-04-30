import { useEffect, useRef } from 'react'
import { useAppStore } from '../../stores/app-store'
import type { SessionMeta, TaskState } from '../../stores/app-store'
import { cliApi } from '../../lib/cli-api'
import { MessageRenderer } from '@kangnam/chat-ui'
import { MarkdownPreview } from '../common/MarkdownPreview'
import { SafetyDialog } from './SafetyDialog'
import { TopBar } from './TopBar'
import { MessageInput } from './MessageInput'
import { useChatSession } from '../../hooks/useChatSession'
import { QuestionFormView } from '../artifacts/QuestionForm'
import type { QuestionForm } from '../../lib/artifacts/question-form'

/**
 * The chat pane content — wires up the four streaming subscriptions
 * (`onMessage`, `onPermissionRequest`, `onEnhanced`, scroll-to-bottom),
 * the session bootstrap effect, and renders the message list with
 * empty / streaming states.
 *
 * Extracted from `ChatView.tsx` in Phase 5a-07. Phase 5a-08 will
 * further extract the session bootstrap effect into a `useChatSession`
 * hook so this file becomes a pure renderer.
 */
function StreamingIndicator() {
  return (
    <div className="flex items-center gap-1 py-3">
      <span className="h-1.5 w-1.5 rounded-full bg-[var(--text-muted)] animate-bounce" style={{ animationDelay: '0ms' }} />
      <span className="h-1.5 w-1.5 rounded-full bg-[var(--text-muted)] animate-bounce" style={{ animationDelay: '150ms' }} />
      <span className="h-1.5 w-1.5 rounded-full bg-[var(--text-muted)] animate-bounce" style={{ animationDelay: '300ms' }} />
    </div>
  )
}

export function ChatContent() {
  const { messages, addMessage, setPendingPermission, currentSessionId,
          setIsStreaming, isStreaming,
          setSessionMeta, addTask, updateTask, setRateLimit, setSessionCost,
          startArtifact, appendArtifactDelta, endArtifact } = useAppStore()
  const messagesEndRef = useRef<HTMLDivElement>(null)

  // Auto-start session when provider is set but no session exists.
  // The bootstrap effect, the conversation-list refresh, and the
  // localStorage workdir persistence all live in the hook now.
  useChatSession()

  useEffect(() => {
    const unlisten = cliApi.onMessage((msg) => {
      // Mirror artifact_* events into the artifacts slice so other
      // panes (PreviewIframe in 5b-11) can read the in-flight body.
      // The message itself still flows into the chat buffer as a
      // typed envelope; the renderer (5b-09) decides whether to
      // surface it inline or only via the preview pane.
      if (msg.type === 'artifact_start') {
        startArtifact(msg.id, msg.kind)
      } else if (msg.type === 'artifact_delta') {
        appendArtifactDelta(msg.id, msg.text)
      } else if (msg.type === 'artifact_end') {
        endArtifact(msg.id, msg.manifest)
      }
      if (msg.type === 'turn_end' || msg.type === 'error') {
        setIsStreaming(false)
      }
      addMessage(msg)
    })
    return unlisten
  }, [addMessage, setIsStreaming, startArtifact, appendArtifactDelta, endArtifact])

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
              {messages.map((msg, i) => {
                // Design-family variants get dedicated renderers.
                // The chat-ui MessageRenderer's switch default returns
                // null so falling through is safe, but rendering the
                // typed component here surfaces the form / artifact
                // affordance the user actually needs.
                if (msg.type === 'question_form_posted') {
                  // The schema is whatever the agent emitted between
                  // the <question-form> tags; the chat-server has
                  // already validated it via design_artifact::parse_question_form.
                  const schema = msg.schema as Partial<QuestionForm> | undefined
                  if (!schema || !Array.isArray(schema.questions)) return null
                  const form: QuestionForm = {
                    id: msg.id,
                    title: schema.title ?? '확인이 필요해요',
                    description: schema.description,
                    questions: schema.questions ?? [],
                    submitLabel: schema.submitLabel,
                  }
                  return <QuestionFormView key={i} form={form} interactive />
                }
                if (
                  msg.type === 'artifact_start' ||
                  msg.type === 'artifact_delta' ||
                  msg.type === 'artifact_end' ||
                  msg.type === 'turn_suspended_pending_form' ||
                  msg.type === 'tool_use_input' ||
                  msg.type === 'thinking_delta'
                ) {
                  // Phase 5b: artifact_* events drive the PreviewIframe
                  // pane (5b-11), not the chat list. thinking_delta and
                  // tool_use_input are status-only — we ignore in-list
                  // for now, ChatContent's stream summary surfaces them
                  // separately if needed.
                  return null
                }
                return (
                  <MessageRenderer
                    key={i}
                    // eslint-disable-next-line @typescript-eslint/no-explicit-any
                    message={msg as any}
                    isLast={i === messages.length - 1}
                    isStreaming={isStreaming}
                    renderMarkdown={(content) => <MarkdownPreview content={content} />}
                  />
                )
              })}
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
