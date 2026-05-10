/**
 * Static fallback registry of selectable LLM models.
 *
 * Temporary — this constant exists only because the TopBar dropdown
 * needs *some* baseline list before the agents slice / sessionMeta
 * stream catches up on first paint. The real source of truth is the
 * `useAvailableModels()` hook (Phase 5a-03), which combines the
 * `agents` slice and the active `sessionMeta` and falls back here
 * when both are empty.
 *
 * Do NOT add provider-specific logic here. If a model needs a label,
 * an alternative id, or a provider-aware sort, that belongs in the
 * hook or in the agents-slice transformer — keep this file as a
 * static, agent-agnostic JSON-shaped list.
 */
export interface ModelOption {
  id: string
  label: string
  defaultReasoningLevel?: string | null
  supportedReasoningLevels?: {
    effort: string
    description: string | null
  }[]
}

export const FALLBACK_MODELS: ModelOption[] = [
  { id: 'claude-sonnet-4-6', label: 'Sonnet 4.6' },
  { id: 'claude-opus-4-6', label: 'Opus 4.6' },
  { id: 'claude-haiku-4-5-20251001', label: 'Haiku 4.5' },
]
