import { create } from 'zustand'

import { createThemeSlice, type ThemeSlice } from './slices/theme'
import { createLayoutSlice, type LayoutSlice } from './slices/layout'
import { createSettingsSlice, type SettingsSlice } from './slices/settings'
import { createMainViewSlice, type MainViewSlice } from './slices/main-view'
import {
  createAgentsPromptsSlice,
  type AgentsPromptsSlice,
} from './slices/agents-prompts'

export interface Conversation {
  id: string
  title: string
  provider: string
  model: string | null
  pinned: number
  created_at: number
  updated_at: number
}

export interface Message {
  id: string
  conversation_id: string
  role: 'user' | 'assistant' | 'system' | 'tool'
  content: string
  tool_use_id: string | null
  tool_name: string | null
  tool_args: string | null
  token_count: number | null
  attachments: string | null
  created_at: number
}

export interface AttachmentData {
  type: 'image' | 'file'
  name: string
  dataUrl: string
}

export interface SkillReference {
  id: string
  skillId: string
  name: string
  content: string
  sortOrder: number
}

export interface Prompt {
  id: string
  name: string
  description: string
  instructions: string
  argumentHint: string | null
  model: string | null
  userInvocable: boolean
  references: SkillReference[]
}

export interface Agent {
  id: string
  name: string
  description: string
  instructions: string
  model: string | null
  allowedTools: string[] | null
  maxTurns: number
  sortOrder: number
}

export interface CliStatus {
  provider: string
  installed: boolean
  version: string | null
  path: string | null
  authenticated: boolean
}

export type UnifiedMessage =
  | { type: 'user_message'; text: string }
  | { type: 'text_delta'; text: string }
  | { type: 'tool_use_start'; id: string; name: string; input: unknown }
  | { type: 'tool_result'; id: string; output: string; is_error: boolean }
  | { type: 'permission_request'; id: string; tool: string; description: string; diff?: string }
  | { type: 'agent_start'; id: string; name: string; description: string }
  | { type: 'agent_progress'; id: string; message: string }
  | { type: 'agent_end'; id: string; result: string }
  | { type: 'skill_invoked'; name: string; args?: string }
  | { type: 'turn_end'; usage?: { input_tokens: number; output_tokens: number } }
  | { type: 'error'; message: string }
  | { type: 'session_init'; session_id: string }

export interface SessionMeta {
  session_id: string
  tools: string[]
  skills: string[]
  slash_commands: string[]
  agents: string[]
  plugins: { name: string; path: string }[]
  mcp_servers: { name: string; status: string }[]
  model: string
  permission_mode: string
  cwd: string
  claude_code_version: string
}

export interface TaskState {
  task_id: string
  description: string
  task_type: string
  status: 'running' | 'completed' | 'failed' | 'stopped'
  summary?: string
}

export interface RateLimitInfo {
  status: string
  utilization: number | null
  rate_limit_type: string
}

export interface ResultSummary {
  cost_usd: number | null
  duration_ms: number | null
  num_turns: number | null
}

export type SidePanelTab = 'chats' | 'files' | 'skills' | 'agents' | 'mcp'
/**
 * Tabs available in the right-side panel.
 *
 * `files` and `tools` were placeholder-only in the original layout.
 * They will be re-introduced by Phase 5c (FileWorkspace + ToolCard for
 * the design FileViewer), so the enum is intentionally narrow until
 * that work lands.
 */
export type RightPanelTab = 'terminal' | 'agents' | 'tasks'
/**
 * Top-level view modes the main pane swaps between.
 *
 * - `chat` / `studio` are wired today.
 * - `project` / `hub` are reserved for the design family Phase 5b/c —
 *   `project` will host the per-project workspace + preview iframe;
 *   `hub` will host the Designs Hub (last-N preview cache). Until
 *   those land, the App-level switch falls back to `chat` for the new
 *   variants so widening the type can't crash older renderer paths.
 */
export type MainView = 'chat' | 'studio' | 'project' | 'hub'
export type StudioBottomTab = 'cli' | 'tests' | 'viewer' | 'optimize'

export interface StudioState {
  type: 'skill' | 'agent'
  name?: string
  activeView: 'dashboard' | 'editor'
  bottomTab: StudioBottomTab
  bottomPanelVisible: boolean
  dirty: boolean
}

interface AppState
  extends ThemeSlice,
    LayoutSlice,
    SettingsSlice,
    MainViewSlice,
    AgentsPromptsSlice {
  // CLI
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

  // Streaming CLI messages
  messages: UnifiedMessage[]
  addMessage: (msg: UnifiedMessage) => void
  clearMessages: () => void
  pendingPermission: UnifiedMessage | null
  setPendingPermission: (msg: UnifiedMessage | null) => void

  // Conversations
  conversations: Conversation[]
  setConversations: (convs: Conversation[]) => void
  activeConversationId: string | null
  setActiveConversationId: (id: string | null) => void

  // Pending attachments (for passing from Composer to onNew)
  pendingAttachments: AttachmentData[]
  setPendingAttachments: (atts: AttachmentData[]) => void

  // Search overlay
  showSearch: boolean
  setShowSearch: (v: boolean) => void

  // Model selection (persisted — used at session start)
  selectedModel: string | null
  setSelectedModel: (model: string | null) => void

  // Enhanced (Claude-specific)
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

export const useAppStore = create<AppState>((set, get) => ({
  // === Slice composition ===
  // Each slice owns a coherent concern. The rest of this object is
  // legacy fields awaiting extraction in Phase 5a-10 through 5a-15.
  // Slices spread first; 5a-16 cleans up once everything is
  // extracted.
  ...createThemeSlice(set),
  ...createLayoutSlice(set),
  ...createSettingsSlice(set),
  ...createMainViewSlice(set),
  ...createAgentsPromptsSlice(set, get),

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

  // Streaming CLI messages
  messages: [],
  addMessage: (msg) => set((s) => {
    const last = s.messages[s.messages.length - 1]
    // Accumulate consecutive text_deltas into one message
    if (msg.type === 'text_delta' && last?.type === 'text_delta') {
      const updated = [...s.messages]
      updated[updated.length - 1] = { ...last, text: last.text + msg.text }
      return { messages: updated }
    }
    // Accumulate consecutive agent_progress from same agent
    if (msg.type === 'agent_progress' && last?.type === 'agent_progress' && last.id === msg.id) {
      const updated = [...s.messages]
      updated[updated.length - 1] = { ...last, message: last.message + msg.message }
      return { messages: updated }
    }
    return { messages: [...s.messages, msg] }
  }),
  clearMessages: () => set({ messages: [] }),
  pendingPermission: null,
  setPendingPermission: (msg) => set({ pendingPermission: msg }),

  // Conversations
  conversations: [],
  setConversations: (convs) => set({ conversations: convs }),
  activeConversationId: null,
  setActiveConversationId: (id) => set({ activeConversationId: id }),

  // Pending attachments
  pendingAttachments: [],
  setPendingAttachments: (atts) => set({ pendingAttachments: atts }),

  // Search overlay
  showSearch: false,
  setShowSearch: (v) => set({ showSearch: v }),

  // Model selection
  selectedModel: localStorage.getItem('kangnam-selected-model'),
  setSelectedModel: (model) => {
    if (model) localStorage.setItem('kangnam-selected-model', model)
    else localStorage.removeItem('kangnam-selected-model')
    set({ selectedModel: model })
  },

  // Enhanced
  sessionMeta: null,
  setSessionMeta: (meta) => set({ sessionMeta: meta }),
  activeTasks: [],
  addTask: (task) => set((s) => ({ activeTasks: [...s.activeTasks, task] })),
  updateTask: (taskId, updates) => set((s) => ({
    activeTasks: s.activeTasks.map((t) =>
      t.task_id === taskId ? { ...t, ...updates } : t
    ),
  })),
  rateLimits: {},
  setRateLimit: (info) => set((s) => ({
    rateLimits: { ...s.rateLimits, [info.rate_limit_type]: info }
  })),
  sessionCost: null,
  setSessionCost: (cost) => set({ sessionCost: cost }),

}))
