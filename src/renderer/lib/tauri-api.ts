/**
 * Tauri API adapter — provides `window.api`-compatible interface.
 *
 * This file is the host-side façade for Tauri `invoke` / `listen`
 * calls. It currently spans 11 namespaces (auth, chat, claudeCommands,
 * conv, mcp, prompts, eval, agents, studio, settings, cowork) and
 * is paired with `cli-api.ts`, which carries the chat WebSocket
 * transport. The two together form the renderer→backend transport
 * surface.
 *
 * The original Phase 5a plan called out unifying / restructuring
 * this dual-transport setup as a refactor blocker. Investigation
 * showed that consolidating the 442 LoC across 11 namespaces is a
 * separate ~5-commit project of its own — large enough that the
 * design family work shouldn't carry it.
 *
 * **Deferred to ADR-011** (see plan §3.2 + §11). Until that ADR
 * lands, treat this file as the canonical Tauri-side transport and
 * `cli-api.ts` as the WS-side transport; do not extend the dual
 * pattern further. New design-mode chat-rpc methods land in
 * `cli-api.ts`; new domain commands land here as a new namespace.
 */
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

type EventCallback<T> = (data: T) => void

function onEvent<T>(eventName: string, callback: EventCallback<T>): () => void {
  let unlisten: UnlistenFn | undefined
  listen<T>(eventName, (e) => callback(e.payload)).then((u) => (unlisten = u))
  return () => unlisten?.()
}

const api = {
  // Auth (Phase 2)
  auth: {
    connect: (provider: string, options?: { setupToken?: string }) =>
      invoke('auth_connect', { provider, options }),
    disconnect: (provider: string) => invoke('auth_disconnect', { provider }),
    status: () => invoke('auth_status'),
    onConnected: (cb: EventCallback<string>) => onEvent('auth:on-connected', cb),
    onDisconnected: (cb: EventCallback<string>) => onEvent('auth:on-disconnected', cb),
    onCopilotDeviceCode: (
      cb: EventCallback<{ userCode: string; verificationUri: string }>
    ) => onEvent('auth:copilot-device-code', cb)
  },

  // Chat (Phase 4)
  chat: {
    send: (
      conversationId: string,
      message: string,
      provider: string,
      attachments?: string,
      model?: string,
      reasoningEffort?: string,
      promptId?: string
    ) =>
      invoke('chat_send', {
        conversationId,
        message,
        provider,
        attachments,
        model,
        reasoningEffort,
        promptId
      }),
    stop: (conversationId: string) => invoke('chat_stop', { conversationId }),
    onStream: (cb: EventCallback<{ conversationId: string; chunk: string }>) =>
      onEvent('chat:stream', cb),
    onComplete: (cb: EventCallback<{ conversationId: string }>) =>
      onEvent('chat:complete', cb),
    onError: (cb: EventCallback<{ conversationId: string; error: string }>) =>
      onEvent('chat:error', cb),
    onToolCall: (
      cb: EventCallback<{ conversationId: string; tool: string; args: unknown }>
    ) => onEvent('chat:tool-call', cb),
    onThinking: (cb: EventCallback<{ conversationId: string; chunk: string }>) =>
      onEvent('chat:thinking', cb),
    onContextUsage: (
      cb: EventCallback<{ conversationId: string; used: number; max: number }>
    ) => onEvent('chat:context-usage', cb)
  },

  // Claude Code file-based commands (~/.claude/commands/)
  claudeCommands: {
    list: () => invoke('list_claude_commands') as Promise<{ name: string; description: string; is_directory: boolean }[]>,
    read: (name: string) => invoke('read_claude_command', { name }) as Promise<{
      name: string; description: string; instructions: string
      is_directory: boolean; refs: { filename: string; size: number; is_main: boolean }[]
    } | null>,
    write: (name: string, description: string, instructions: string) =>
      invoke('write_claude_command', { name, description, instructions }),
    delete: (name: string) => invoke('delete_claude_command', { name }),
    // Reference file management
    listRefs: (name: string) => invoke('list_skill_refs', { name }) as Promise<{ filename: string; size: number; is_main: boolean }[]>,
    readRef: (name: string, filename: string) => invoke('read_skill_ref', { name, filename }) as Promise<string>,
    writeRef: (name: string, filename: string, content: string) =>
      invoke('write_skill_ref', { name, filename, content }),
    deleteRef: (name: string, filename: string) => invoke('delete_skill_ref', { name, filename }),
    forkPlugin: (pluginPath: string, skillName: string) => invoke('fork_plugin_skill', { pluginPath, skillName }),
  },

  // Conversations (Phase 3)
  conv: {
    list: () => invoke('conv_list'),
    create: (provider: string) => invoke('conv_create', { provider }),
    delete: (id: string) => invoke('conv_delete', { id }),
    getMessages: (id: string) => invoke('conv_get_messages', { id }),
    updateTitle: (id: string, title: string) =>
      invoke('conv_update_title', { id, title }),
    togglePin: (id: string) => invoke('conv_toggle_pin', { id }),
    deleteAll: () => invoke('conv_delete_all'),
    export: (id: string, format: 'markdown' | 'json') =>
      invoke('conv_export', { id, format }),
    search: (query: string) => invoke('conv_search', { query }),
    onTitleUpdated: (
      cb: EventCallback<{ conversationId: string; title: string }>
    ) => onEvent('conv:title-updated', cb)
  },

  // MCP (Phase 5)
  mcp: {
    listServers: () => invoke('mcp_list_servers'),
    aiAssist: (prompt: string, provider: string, model?: string) =>
      invoke('mcp_ai_assist', { prompt, provider, model }),
    getConfig: (name: string) => invoke('mcp_get_config', { name }),
    addServer: (config: unknown) => invoke('mcp_add_server', { config }),
    reconnectServer: (name: string) => invoke('mcp_reconnect_server', { name }),
    updateServer: (oldName: string, config: unknown) =>
      invoke('mcp_update_server', { oldName, config }),
    removeServer: (name: string) => invoke('mcp_remove_server', { name }),
    listTools: () => invoke('mcp_list_tools'),
    serverStatus: () => invoke('mcp_server_status')
  },

  // Skills / Prompts (Phase 3)
  prompts: {
    list: () => invoke('prompts_list'),
    get: (id: string) => invoke('prompts_get', { id }),
    getInstructions: (id: string) => invoke('prompts_get_instructions', { id }),
    create: (
      name: string,
      description: string,
      instructions: string,
      argumentHint?: string,
      model?: string,
      userInvocable?: boolean
    ) =>
      invoke('prompts_create', {
        name,
        description,
        instructions,
        argumentHint,
        model,
        userInvocable
      }),
    update: (
      id: string,
      name: string,
      description: string,
      instructions: string,
      argumentHint?: string,
      model?: string,
      userInvocable?: boolean
    ) =>
      invoke('prompts_update', {
        id,
        name,
        description,
        instructions,
        argumentHint,
        model,
        userInvocable
      }),
    delete: (id: string) => invoke('prompts_delete', { id }),
    // References
    refList: (skillId: string) => invoke('prompts_ref_list', { skillId }),
    refAdd: (skillId: string, name: string, content: string) =>
      invoke('prompts_ref_add', { skillId, name, content }),
    refUpdate: (id: string, name: string, content: string) =>
      invoke('prompts_ref_update', { id, name, content }),
    refDelete: (id: string) => invoke('prompts_ref_delete', { id }),
    // AI Assist (Phase 6)
    aiGenerate: (userRequest: string, provider: string, model?: string) =>
      invoke('prompts_ai_generate', { userRequest, provider, model }),
    aiImprove: (
      currentSkill: { name: string; description: string; instructions: string },
      feedback: string,
      provider: string,
      model?: string
    ) => invoke('prompts_ai_improve', { currentSkill, feedback, provider, model }),
    aiGenerateRef: (
      skillInstructions: string,
      userRequest: string,
      provider: string,
      model?: string
    ) =>
      invoke('prompts_ai_generate_ref', {
        skillInstructions,
        userRequest,
        provider,
        model
      }),
    aiGenerateEvals: (
      skill: { name: string; description: string; instructions: string },
      provider: string,
      model?: string
    ) => invoke('prompts_ai_generate_evals', { skill, provider, model }),
    // Sub-Agents (Phase 6)
    aiGrade: (
      skill: { name: string; description: string; instructions: string },
      criteria: string[],
      provider: string,
      model?: string
    ) => invoke('prompts_ai_grade', { skill, criteria, provider, model }),
    aiCompare: (
      skillA: { name: string; description: string; instructions: string },
      skillB: { name: string; description: string; instructions: string },
      provider: string,
      model?: string
    ) => invoke('prompts_ai_compare', { skillA, skillB, provider, model }),
    aiAnalyze: (
      comparisonResult: unknown,
      winnerSkill: { name: string; description: string; instructions: string },
      loserSkill: { name: string; description: string; instructions: string },
      provider: string,
      model?: string
    ) =>
      invoke('prompts_ai_analyze', {
        comparisonResult,
        winnerSkill,
        loserSkill,
        provider,
        model
      })
  },

  // Eval (Phase 6)
  eval: {
    setCreate: (skillId: string, name?: string) =>
      invoke('eval_set_create', { skillId, name }),
    setList: (skillId: string) => invoke('eval_set_list', { skillId }),
    setDelete: (id: string) => invoke('eval_set_delete', { id }),
    caseAdd: (
      evalSetId: string,
      prompt: string,
      expected: string,
      shouldTrigger: boolean
    ) => invoke('eval_case_add', { evalSetId, prompt, expected, shouldTrigger }),
    caseBulkAdd: (
      evalSetId: string,
      cases: Array<{ prompt: string; expected: string; shouldTrigger: boolean }>
    ) => invoke('eval_case_bulk_add', { evalSetId, cases }),
    caseUpdate: (
      id: string,
      prompt: string,
      expected: string,
      shouldTrigger: boolean
    ) => invoke('eval_case_update', { id, prompt, expected, shouldTrigger }),
    caseDelete: (id: string) => invoke('eval_case_delete', { id }),
    caseList: (evalSetId: string) => invoke('eval_case_list', { evalSetId }),
    runStart: (
      evalSetId: string,
      skillId: string,
      provider: string,
      model?: string
    ) => invoke('eval_run_start', { evalSetId, skillId, provider, model }),
    runStop: (runId: string) => invoke('eval_run_stop', { runId }),
    runList: (evalSetId: string) => invoke('eval_run_list', { evalSetId }),
    runGet: (runId: string) => invoke('eval_run_get', { runId }),
    runResults: (runId: string) => invoke('eval_run_results', { runId }),
    runStats: (runId: string) => invoke('eval_run_stats', { runId }),
    runDelete: (runId: string) => invoke('eval_run_delete', { runId }),
    resultFeedback: (resultId: string, feedback: string, rating: number) =>
      invoke('eval_result_feedback', { resultId, feedback, rating }),
    aiGenerate: (
      skill: { name: string; description: string; instructions: string },
      provider: string,
      model?: string
    ) => invoke('eval_ai_generate', { skill, provider, model }),
    optimizeStart: (
      skillId: string,
      evalSetId: string,
      provider: string,
      model?: string
    ) => invoke('eval_optimize_start', { skillId, evalSetId, provider, model }),
    onProgress: (
      cb: EventCallback<{
        runId: string
        caseIndex: number
        totalCases: number
        result: unknown
      }>
    ) => onEvent('eval:progress', cb),
    onRunComplete: (cb: EventCallback<{ runId: string; stats: unknown }>) =>
      onEvent('eval:run-complete', cb),
    onRunError: (cb: EventCallback<{ runId: string; error: string }>) =>
      onEvent('eval:run-error', cb),
    onOptimizeProgress: (
      cb: EventCallback<{ step: string; [key: string]: unknown }>
    ) => onEvent('eval:optimize-progress', cb),
    onOptimizeComplete: (cb: EventCallback<{ candidates: unknown[] }>) =>
      onEvent('eval:optimize-complete', cb)
  },

  // Agents
  agents: {
    list: () => invoke('agents_list'),
    get: (id: string) => invoke('agents_get', { id }),
    create: (
      name: string,
      description: string,
      instructions: string,
      model?: string,
      allowedTools?: string[],
      maxTurns?: number
    ) =>
      invoke('agents_create', {
        name,
        description,
        instructions,
        model,
        allowedTools,
        maxTurns
      }),
    update: (
      id: string,
      name: string,
      description: string,
      instructions: string,
      model?: string,
      allowedTools?: string[],
      maxTurns?: number
    ) =>
      invoke('agents_update', {
        id,
        name,
        description,
        instructions,
        model,
        allowedTools,
        maxTurns
      }),
    delete: (id: string) => invoke('agents_delete', { id }),
    readDescription: (name: string) => invoke('read_agent_description', { name }) as Promise<string | null>,
    // Claude Code file-based agents (~/.claude/agents/)
    listClaude: () => invoke('list_claude_agents') as Promise<{ name: string; description: string; model: string | null; is_directory: boolean }[]>,
    readClaude: (name: string) => invoke('read_claude_agent', { name }) as Promise<{
      name: string; description: string; instructions: string; model: string | null
      is_directory: boolean; refs: { filename: string; size: number; is_main: boolean }[]
    } | null>,
    writeClaude: (name: string, description: string, instructions: string, model?: string | null) =>
      invoke('write_claude_agent', { name, description, instructions, model }),
    deleteClaude: (name: string) => invoke('delete_claude_agent', { name }),
    listRefs: (name: string) => invoke('list_agent_refs', { name }) as Promise<{ filename: string; size: number; is_main: boolean }[]>,
    readRef: (name: string, filename: string) => invoke('read_agent_ref', { name, filename }) as Promise<string>,
    writeRef: (name: string, filename: string, content: string) =>
      invoke('write_agent_ref', { name, filename, content }),
    deleteRef: (name: string, filename: string) => invoke('delete_agent_ref', { name, filename }),
    execute: (
      agentId: string,
      conversationId: string,
      task: string,
      provider: string,
      model?: string
    ) =>
      invoke('agents_execute', {
        agentId,
        conversationId,
        task,
        provider,
        model
      }),
    onRunStart: (
      cb: EventCallback<{
        runId: string
        agentName: string
        conversationId: string
      }>
    ) => onEvent('agent:run-start', cb),
    onRunComplete: (
      cb: EventCallback<{
        runId: string
        conversationId: string
        result: string
      }>
    ) => onEvent('agent:run-complete', cb),
    onRunError: (
      cb: EventCallback<{
        runId: string
        conversationId: string
        error: string
      }>
    ) => onEvent('agent:run-error', cb),
    onStream: (cb: EventCallback<{ runId: string; chunk: string }>) =>
      onEvent('agent:stream', cb)
  },

  // Studio snapshots
  studio: {
    snapshotSkill: (name: string) => invoke('snapshot_skill', { name }) as Promise<string>,
    snapshotAgent: (name: string) => invoke('snapshot_agent', { name }) as Promise<string>,
    listSkillSnapshots: (name: string) =>
      invoke('list_skill_snapshots', { name }) as Promise<{ filename: string; timestamp: number; size: number }[]>,
    listAgentSnapshots: (name: string) =>
      invoke('list_agent_snapshots', { name }) as Promise<{ filename: string; timestamp: number; size: number }[]>,
  },

  // Settings (Phase 1 — active)
  settings: {
    get: () => invoke('settings_get'),
    set: (settings: unknown) => invoke('settings_set', { partial: settings })
  },

  // Cowork (Phase 6)
  cowork: {
    start: (
      task: string,
      provider: string,
      model?: string,
      reasoningEffort?: string
    ) => invoke('cowork_start', { task, provider, model, reasoningEffort }),
    stop: () => invoke('cowork_stop'),
    followUp: (instruction: string) =>
      invoke('cowork_follow_up', { instruction }),
    onPlan: (cb: EventCallback<{ steps: string[] }>) =>
      onEvent('cowork:plan', cb),
    onStepStart: (cb: EventCallback<{ step: number }>) =>
      onEvent('cowork:step-start', cb),
    onStream: (cb: EventCallback<{ chunk: string }>) =>
      onEvent('cowork:stream', cb),
    onToolCall: (
      cb: EventCallback<{ id: string; name: string; input: Record<string, unknown> }>
    ) => onEvent('cowork:tool-call', cb),
    onToolResult: (
      cb: EventCallback<{
        id: string
        result: string
        status: 'success' | 'error'
      }>
    ) => onEvent('cowork:tool-result', cb),
    onStepComplete: (cb: EventCallback<{ step: number }>) =>
      onEvent('cowork:step-complete', cb),
    onComplete: (cb: EventCallback<{ summary: string }>) =>
      onEvent('cowork:complete', cb),
    onError: (cb: EventCallback<{ error: string }>) =>
      onEvent('cowork:error', cb)
  }
}

declare global {
  interface Window {
    api: typeof api
  }
}

// Expose as window.api for compatibility with existing components
window.api = api

export type TauriAPI = typeof api
