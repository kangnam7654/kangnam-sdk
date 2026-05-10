import { RpcClient } from './rpc/client'
import { createWsTransport } from './rpc/transport-ws'
import type { CliStatus, UnifiedMessage } from '../stores/app-store'
import { createArtifactParser } from './artifacts/parser'

export interface ProviderMeta {
  name: string
  display_name: string
  description: string
  install_hint: string
  auth_source: string
  subscription_based: boolean
  login_hint: string | null
}

export interface CliModel {
  name: string
  display_name: string
  description: string | null
  input_token_limit: number | null
  output_token_limit: number | null
  default_reasoning_level?: string | null
  supported_reasoning_levels?: {
    effort: string
    description: string | null
  }[]
}

// Connect to Axum WebSocket server
const WS_PORT = (globalThis as Record<string, unknown>).__KANGNAM_PORT ?? '3001'
const WS_URL = `ws://localhost:${WS_PORT}/ws`
const transport = createWsTransport(WS_URL)
const rpc = new RpcClient(transport)

export const cliApi = {
  listProviders: () =>
    rpc.call<ProviderMeta[]>('cli.listProviders'),

  checkInstalled: (provider: string) =>
    rpc.call<CliStatus>('cli.checkInstalled', { provider }),

  listModels: (provider: string) =>
    rpc.call<CliModel[]>('cli.listModels', { provider }),

  install: (provider: string) =>
    rpc.call<void>('cli.install', { provider }),

  startSession: (provider: string, workingDir?: string, model?: string | null) =>
    rpc.call<string>('cli.startSession', {
      provider,
      ...(workingDir && { workingDir }),
      ...(model && { model }),
    }),

  sendMessage: (sessionId: string, message: string) =>
    rpc.call<void>('cli.sendMessage', { sessionId, message }),

  permissionResponse: (id: string, allowed: boolean) =>
    rpc.call<void>('cli.permissionResponse', { id, allowed }),

  /**
   * Resolve a parked `<question-form>` posted by the design `ask`
   * tool (Phase 4). `payload` is the user's answers — shape matches
   * what the agent's question-form schema expects.
   */
  questionFormResponse: (id: string, payload: Record<string, unknown>) =>
    rpc.call<void>('cli.questionFormResponse', { id, payload }),

  /**
   * Resolve a parked `preview` request from the design tool
   * (Phase 4). `payload` carries `{ screenshot, console, errors }`
   * captured from the sandboxed iframe.
   */
  previewResult: (id: string, payload: Record<string, unknown>) =>
    rpc.call<void>('cli.previewResult', { id, payload }),

  stopSession: (sessionId: string) =>
    rpc.call<void>('cli.stopSession', { sessionId }),

  /**
   * Subscribe to CLI stream events (JSON-RPC Notifications).
   *
   * `text_delta` chunks are piped through a per-subscription
   * `<artifact>` parser so the consumer gets typed
   * `artifact_start` / `artifact_delta` / `artifact_end` variants
   * for free. Plain text outside any artifact still arrives as a
   * regular `text_delta`.
   *
   * On `turn_end` the parser is flushed so any unterminated
   * artifact yields its final body before the turn closes.
   */
  onMessage: (callback: (msg: UnifiedMessage) => void): (() => void) => {
    const parser = createArtifactParser()
    return rpc.onNotification((method, params) => {
      if (method !== 'cli.stream') return
      const msg = params as UnifiedMessage
      if (msg.type === 'text_delta') {
        for (const ev of parser.feed(msg.text)) {
          if (ev.type === 'text') {
            if (ev.delta) callback({ type: 'text_delta', text: ev.delta })
          } else if (ev.type === 'artifact:start') {
            callback({ type: 'artifact_start', id: ev.identifier, kind: ev.artifactType })
          } else if (ev.type === 'artifact:chunk') {
            callback({ type: 'artifact_delta', id: ev.identifier, text: ev.delta })
          } else if (ev.type === 'artifact:end') {
            callback({ type: 'artifact_end', id: ev.identifier, manifest: undefined })
          }
        }
        return
      }
      if (msg.type === 'turn_end' || msg.type === 'error') {
        for (const ev of parser.flush()) {
          if (ev.type === 'text') {
            if (ev.delta) callback({ type: 'text_delta', text: ev.delta })
          } else if (ev.type === 'artifact:chunk') {
            callback({ type: 'artifact_delta', id: ev.identifier, text: ev.delta })
          } else if (ev.type === 'artifact:end') {
            callback({ type: 'artifact_end', id: ev.identifier, manifest: undefined })
          }
        }
      }
      callback(msg)
    })
  },

  /** Subscribe to MCP permission request notifications */
  onPermissionRequest: (callback: (req: { id: string; tool: string; description: string; input?: unknown }) => void): (() => void) =>
    rpc.onNotification((method, params) => {
      if (method === 'cli.permissionRequest') {
        callback(params as { id: string; tool: string; description: string; input?: unknown })
      }
    }),

  /**
   * Subscribe to design `preview` tool requests from the host MCP
   * server (Phase 4d `mcp__kangnam__preview`). The PreviewIframe
   * component (5b-11) listens here to render an artifact in a
   * sandboxed iframe and reply via `previewResult(id, payload)`.
   */
  onPreviewRequest: (
    callback: (req: { id: string; path: string; viewport?: { width: number; height: number } }) => void,
  ): (() => void) =>
    rpc.onNotification((method, params) => {
      if (method === 'cli.previewRequest') {
        callback(params as { id: string; path: string; viewport?: { width: number; height: number } })
      }
    }),

  /** Subscribe to Claude-enhanced events (JSON-RPC Notifications) */
  onEnhanced: (callback: (event: Record<string, unknown>) => void): (() => void) =>
    rpc.onNotification((method, params) => {
      if (method === 'cli.enhanced') {
        callback(params as Record<string, unknown>)
      }
    }),
}
