/**
 * Conversations slice — list of past chat sessions + the active one.
 *
 * `Conversation` itself is sourced from the host's SQLite via
 * `window.api.conv.list()` (called from `useChatSession` and
 * conversation-list components). This slice only stores the
 * fetched list; persistence is on the backend.
 */
import type { Conversation } from '../app-store'

export interface ConversationsSlice {
  conversations: Conversation[]
  setConversations: (convs: Conversation[]) => void
  activeConversationId: string | null
  setActiveConversationId: (id: string | null) => void
}

type ConversationsSet = (partial: Partial<ConversationsSlice>) => void

export function createConversationsSlice(set: ConversationsSet): ConversationsSlice {
  return {
    conversations: [],
    setConversations: (convs) => set({ conversations: convs }),
    activeConversationId: null,
    setActiveConversationId: (id) => set({ activeConversationId: id }),
  }
}
