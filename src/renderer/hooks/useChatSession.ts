/**
 * Boots the agent CLI session whenever a provider is selected but
 * no session id is active. Persists the chosen working directory in
 * localStorage and refreshes the conversation list once the session
 * is up.
 *
 * Extracted from `ChatContent.tsx` in Phase 5a-08 so the renderer
 * becomes pure (props in → JSX out + subscription effects only).
 *
 * Note: this hook is *side-effect-only* — it returns nothing. The
 * underlying `startingSession` ref guards against double-fires when
 * React re-renders during the in-flight start.
 */
import { useEffect, useRef } from 'react'

import { cliApi } from '../lib/cli-api'
import type { Conversation } from '../stores/app-store'
import { useAppStore } from '../stores/app-store'

export function useChatSession(): void {
  const currentSessionId = useAppStore((s) => s.currentSessionId)
  const currentProvider = useAppStore((s) => s.currentProvider)
  const setCurrentSessionId = useAppStore((s) => s.setCurrentSessionId)
  const setCurrentWorkingDir = useAppStore((s) => s.setCurrentWorkingDir)
  const selectedModel = useAppStore((s) => s.selectedModel)

  const startingSession = useRef(false)

  useEffect(() => {
    if (currentSessionId || !currentProvider || startingSession.current) return
    startingSession.current = true

    const lastDir = localStorage.getItem('kangnam-last-workdir')
    const workingDir =
      lastDir || (navigator.platform?.includes('Win') ? 'C:\\Users' : '/Users')

    cliApi
      .startSession(currentProvider, workingDir, selectedModel)
      .then(async (sessionId) => {
        setCurrentSessionId(sessionId)
        setCurrentWorkingDir(workingDir)
        localStorage.setItem('kangnam-last-workdir', workingDir)
        // Refresh conversation list
        try {
          const convs = await window.api.conv.list()
          useAppStore.getState().setConversations(convs as Conversation[])
        } catch {
          /* ignore */
        }
      })
      .catch((e) => {
        console.error('[useChatSession] auto-start session failed:', e)
      })
      .finally(() => {
        startingSession.current = false
      })
  }, [
    currentSessionId,
    currentProvider,
    setCurrentSessionId,
    setCurrentWorkingDir,
    selectedModel,
  ])
}
