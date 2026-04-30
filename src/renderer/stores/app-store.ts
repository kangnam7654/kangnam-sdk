/**
 * Top-level Zustand store + the data shapes it carries.
 *
 * Phase 5a-09 through 5a-15 split the previously-monolithic 427 LoC
 * store object into seven focused slices. This file is now the
 * composer: it lists the type shapes the rest of the renderer
 * depends on, defines the union `AppState` from the slice
 * interfaces, and assembles the store by spreading slice creators
 * in dependency order (`get()` is needed only by
 * agents-prompts for cross-field clearing).
 *
 * Adding a new slice:
 * 1. Create `slices/<name>.ts` exporting `<Name>Slice` interface
 *    and `create<Name>Slice(set[, get])` factory.
 * 2. Import here, extend `AppState`, and append the spread inside
 *    `useAppStore`.
 * 3. Existing `useAppStore(s => s.field)` callsites work unchanged.
 *
 * Type aliases (`Conversation`, `Agent`, `Prompt`, `UnifiedMessage`,
 * `SessionMeta`, …) stay in this file because they are referenced
 * across multiple slices and many components — moving them would
 * fan out unnecessary import churn. The slice files import these
 * types from here; the back-references are type-only and have no
 * runtime effect.
 */
import { create } from 'zustand'

import { createThemeSlice, type ThemeSlice } from './slices/theme'
import { createLayoutSlice, type LayoutSlice } from './slices/layout'
import { createSettingsSlice, type SettingsSlice } from './slices/settings'
import { createMainViewSlice, type MainViewSlice } from './slices/main-view'
import {
  createAgentsPromptsSlice,
  type AgentsPromptsSlice,
} from './slices/agents-prompts'
import {
  createConversationsSlice,
  type ConversationsSlice,
} from './slices/conversations'
import { createChatSlice, type ChatSlice } from './slices/chat'

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
  // Phase 0b chat-agent parity additions —
  | { type: 'thinking_delta'; text: string }
  | { type: 'tool_use_input'; id: string; input: unknown }
  // Phase 3 + 5b design-family additions —
  | { type: 'artifact_start'; id: string; kind: string }
  | { type: 'artifact_delta'; id: string; text: string }
  | { type: 'artifact_end'; id: string; manifest?: unknown }
  | { type: 'question_form_posted'; id: string; schema: unknown }
  | { type: 'turn_suspended_pending_form'; form_id: string }

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
    AgentsPromptsSlice,
    ConversationsSlice,
    ChatSlice {}

export const useAppStore = create<AppState>((set, get) => ({
  ...createThemeSlice(set),
  ...createLayoutSlice(set),
  ...createSettingsSlice(set),
  ...createMainViewSlice(set),
  ...createAgentsPromptsSlice(set, get),
  ...createConversationsSlice(set),
  ...createChatSlice(set),
}))
