import { ChatContent } from './ChatContent'

/**
 * Public entry point for the chat pane.
 *
 * After Phase 5a-05 / 5a-06 / 5a-07 the heavy lifting moved out of
 * this file: TopBar, MessageInput, ChatContent (with the streaming
 * subscriptions and session bootstrap effect) each live in their
 * own modules. ChatView is intentionally thin — App.tsx imports it
 * by name, and any future composition (e.g. wrapping ChatContent
 * with a side panel or analytics provider) lands here.
 */
export function ChatView() {
  return <ChatContent />
}
