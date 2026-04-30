/**
 * Source the model dropdown list from live state instead of a static
 * constant.
 *
 * Priority:
 * 1. `agents` slice — every Agent that declares a `model` contributes an
 *    option. The agents list is provider-aware (Claude / Codex / Gemini /
 *    LM Studio) so this naturally surfaces whatever the host has wired.
 * 2. `sessionMeta.model` — if a live session is running with a model not
 *    represented in the agents list (Claude Code rare picks, manual
 *    override), prepend it so the dropdown can show the *current* choice.
 * 3. `FALLBACK_MODELS` — only when both above are empty (cold start before
 *    any agents load).
 *
 * The returned list is deduped by `id`, ordered with the active session's
 * model first and the rest in agents-slice order.
 *
 * Phase 5a-03 of the design family work.
 */
import { useMemo } from 'react'

import { useAppStore } from '../stores/app-store'
import { FALLBACK_MODELS, type ModelOption } from '../lib/models/registry'

/** Pretty label for a few well-known model ids; falls back to the id. */
const KNOWN_LABELS: Record<string, string> = {
  'claude-sonnet-4-6': 'Sonnet 4.6',
  'claude-opus-4-6': 'Opus 4.6',
  'claude-haiku-4-5-20251001': 'Haiku 4.5',
}

function labelFor(id: string): string {
  return KNOWN_LABELS[id] ?? id
}

export function useAvailableModels(): ModelOption[] {
  const agents = useAppStore((s) => s.agents)
  const sessionMeta = useAppStore((s) => s.sessionMeta)

  return useMemo(() => {
    const out: ModelOption[] = []
    const seen = new Set<string>()

    // 2 first — surface the *current* session's model at the top so a
    // dropdown render right after a session start always shows the live pick.
    if (sessionMeta?.model && !seen.has(sessionMeta.model)) {
      out.push({ id: sessionMeta.model, label: labelFor(sessionMeta.model) })
      seen.add(sessionMeta.model)
    }

    for (const agent of agents) {
      if (!agent.model) continue
      if (seen.has(agent.model)) continue
      out.push({ id: agent.model, label: labelFor(agent.model) })
      seen.add(agent.model)
    }

    if (out.length === 0) {
      return FALLBACK_MODELS
    }
    return out
  }, [agents, sessionMeta?.model])
}
