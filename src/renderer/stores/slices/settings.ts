/**
 * Settings slice — overlay visibility, settings tab, and dev-mode toggle.
 *
 * Dev mode shows hidden providers (gemini / antigravity / claude OAT /
 * mock); persisted in `localStorage['kangnam-dev-mode']`, activated
 * via Ctrl+Shift+D.
 */
export type SettingsTab = 'providers' | 'mcp' | 'general'

export interface SettingsSlice {
  showSettings: boolean
  setShowSettings: (v: boolean) => void
  settingsTab: SettingsTab
  setSettingsTab: (tab: SettingsTab) => void

  devMode: boolean
  setDevMode: (v: boolean) => void
  toggleDevMode: () => void
}

type SettingsSet = (
  partial: Partial<SettingsSlice> | ((state: SettingsSlice) => Partial<SettingsSlice>),
) => void

export function createSettingsSlice(set: SettingsSet): SettingsSlice {
  return {
    showSettings: false,
    setShowSettings: (v) => set({ showSettings: v }),
    settingsTab: 'providers',
    setSettingsTab: (tab) => set({ settingsTab: tab }),

    devMode: localStorage.getItem('kangnam-dev-mode') === 'true',
    setDevMode: (v) => {
      localStorage.setItem('kangnam-dev-mode', v ? 'true' : 'false')
      set({ devMode: v })
    },
    toggleDevMode: () =>
      set((s) => {
        const next = !s.devMode
        localStorage.setItem('kangnam-dev-mode', next ? 'true' : 'false')
        return { devMode: next }
      }),
  }
}
