/**
 * Agents + Prompts slice — combined because activating one clears
 * the other. They share the "active item" exclusivity invariant
 * (a prompt and an agent can't both be active simultaneously),
 * so splitting them would force one slice to reach into the other.
 */
import type { Agent, Prompt } from '../app-store'

export interface AgentsPromptsSlice {
  prompts: Prompt[]
  setPrompts: (prompts: Prompt[]) => void
  activePromptId: string | null
  setActivePromptId: (id: string | null) => void

  agents: Agent[]
  setAgents: (agents: Agent[]) => void
  activeAgentId: string | null
  setActiveAgentId: (id: string | null) => void
}

type AgentsPromptsSet = (
  partial: Partial<AgentsPromptsSlice>,
) => void
type AgentsPromptsGet = () => AgentsPromptsSlice

export function createAgentsPromptsSlice(
  set: AgentsPromptsSet,
  get: AgentsPromptsGet,
): AgentsPromptsSlice {
  return {
    prompts: [],
    setPrompts: (prompts) => set({ prompts }),
    activePromptId: null,
    // Activating a prompt clears any active agent — keeps the
    // "one selection at a time" invariant.
    setActivePromptId: (id) =>
      set({ activePromptId: id, activeAgentId: id ? null : get().activeAgentId }),

    agents: [],
    setAgents: (agents) => set({ agents }),
    activeAgentId: null,
    setActiveAgentId: (id) =>
      set({ activeAgentId: id, activePromptId: id ? null : get().activePromptId }),
  }
}
