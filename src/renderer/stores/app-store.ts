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
  ...createConversationsSlice(set),
  ...createChatSlice(set),
}))
