/**
 * Chat slice — the largest of the seven; bundles everything tied to
 * an active CLI session: provider/session ids, working directory,
 * the streaming message buffer, sessionMeta, tasks, rate limits,
 * cost summary, search overlay, pending attachments, and the
 * selected model.
 *
 * Grouped together rather than split because every consumer in the
 * chat pane reads from across these fields (the streaming
 * subscriptions in `ChatContent` touch ~9 of them on every event)
 * and each split would be either trivially-small or coupled enough
 * that the boundary is artificial. If this slice grows further the
 * candidates for further extraction are `tasks` (TaskPanel-only) or
 * `usage` (rateLimits + sessionCost — pure metering).
 */
import type {
  AttachmentData,
  CliStatus,
  RateLimitInfo,
  ResultSummary,
  SessionMeta,
  TaskState,
  UnifiedMessage,
} from '../app-store'

export interface ChatSlice {
  // CLI provider + session bootstrap
  cliStatuses: CliStatus[]
  setCliStatuses: (statuses: CliStatus[]) => void
  currentSessionId: string | null
  setCurrentSessionId: (id: string | null) => void
  currentProvider: string | null
  setCurrentProvider: (provider: string | null) => void
  setupComplete: boolean
  setSetupComplete: (complete: boolean) => void
  currentWorkingDir: string | null
  setCurrentWorkingDir: (dir: string | null) => void
  isStreaming: boolean
  setIsStreaming: (v: boolean) => void

  // Streaming message buffer
  messages: UnifiedMessage[]
  addMessage: (msg: UnifiedMessage) => void
  clearMessages: () => void
  pendingPermission: UnifiedMessage | null
  setPendingPermission: (msg: UnifiedMessage | null) => void

  // Composer-side attachments staged by the user
  pendingAttachments: AttachmentData[]
  setPendingAttachments: (atts: AttachmentData[]) => void

  // Search overlay (Cmd+F over the message buffer)
  showSearch: boolean
  setShowSearch: (v: boolean) => void

  // Selected model — persisted; used at session start
  selectedModel: string | null
  setSelectedModel: (model: string | null) => void

  // Claude-specific session telemetry
  sessionMeta: SessionMeta | null
  setSessionMeta: (meta: SessionMeta | null) => void
  activeTasks: TaskState[]
  addTask: (task: TaskState) => void
  updateTask: (taskId: string, updates: Partial<TaskState>) => void
  rateLimits: Record<string, RateLimitInfo>
  setRateLimit: (info: RateLimitInfo) => void
  sessionCost: ResultSummary | null
  setSessionCost: (cost: ResultSummary | null) => void
}

type ChatSet = (
  partial: Partial<ChatSlice> | ((state: ChatSlice) => Partial<ChatSlice>),
) => void

export function createChatSlice(set: ChatSet): ChatSlice {
  return {
    // CLI
    cliStatuses: [],
    setCliStatuses: (statuses) => set({ cliStatuses: statuses }),
    currentSessionId: null,
    setCurrentSessionId: (id) => set({ currentSessionId: id }),
    currentProvider: localStorage.getItem('kangnam-provider'),
    setCurrentProvider: (provider) => {
      if (provider) localStorage.setItem('kangnam-provider', provider)
      else localStorage.removeItem('kangnam-provider')
      set({ currentProvider: provider })
    },
    setupComplete: localStorage.getItem('kangnam-setup-complete') === 'true',
    setSetupComplete: (complete) => {
      localStorage.setItem('kangnam-setup-complete', complete ? 'true' : 'false')
      set({ setupComplete: complete })
    },
    currentWorkingDir: null,
    setCurrentWorkingDir: (dir) => set({ currentWorkingDir: dir }),
    isStreaming: false,
    setIsStreaming: (v) => set({ isStreaming: v }),

    // Streaming messages — accumulates consecutive text_deltas /
    // agent_progress events from the same source so the renderer
    // doesn't churn on every chunk.
    messages: [],
    addMessage: (msg) =>
      set((s) => {
        const last = s.messages[s.messages.length - 1]
        if (msg.type === 'text_delta' && last?.type === 'text_delta') {
          const updated = [...s.messages]
          updated[updated.length - 1] = { ...last, text: last.text + msg.text }
          return { messages: updated }
        }
        if (
          msg.type === 'agent_progress' &&
          last?.type === 'agent_progress' &&
          last.id === msg.id
        ) {
          const updated = [...s.messages]
          updated[updated.length - 1] = { ...last, message: last.message + msg.message }
          return { messages: updated }
        }
        return { messages: [...s.messages, msg] }
      }),
    clearMessages: () => set({ messages: [] }),
    pendingPermission: null,
    setPendingPermission: (msg) => set({ pendingPermission: msg }),

    // Pending attachments
    pendingAttachments: [],
    setPendingAttachments: (atts) => set({ pendingAttachments: atts }),

    // Search overlay
    showSearch: false,
    setShowSearch: (v) => set({ showSearch: v }),

    // Selected model
    selectedModel: localStorage.getItem('kangnam-selected-model'),
    setSelectedModel: (model) => {
      if (model) localStorage.setItem('kangnam-selected-model', model)
      else localStorage.removeItem('kangnam-selected-model')
      set({ selectedModel: model })
    },

    // Enhanced telemetry
    sessionMeta: null,
    setSessionMeta: (meta) => set({ sessionMeta: meta }),
    activeTasks: [],
    addTask: (task) => set((s) => ({ activeTasks: [...s.activeTasks, task] })),
    updateTask: (taskId, updates) =>
      set((s) => ({
        activeTasks: s.activeTasks.map((t) =>
          t.task_id === taskId ? { ...t, ...updates } : t,
        ),
      })),
    rateLimits: {},
    setRateLimit: (info) =>
      set((s) => ({
        rateLimits: { ...s.rateLimits, [info.rate_limit_type]: info },
      })),
    sessionCost: null,
    setSessionCost: (cost) => set({ sessionCost: cost }),
  }
}
