/**
 * Theme slice — `'light' | 'dark'` persisted to localStorage under
 * `kangnam-theme`. Defaults to dark when no value is stored.
 *
 * First slice extracted in Phase 5a-09. Sets the pattern for the
 * rest: an interface describing the slice's surface, a creator
 * function `(set) => sliceFields`, both spread into `useAppStore`
 * by `app-store.ts` so existing `useAppStore(s => s.theme)`
 * callsites are unaffected.
 *
 * The creator returns a plain object instead of using Zustand's
 * `StateCreator` type so each slice file stays decoupled from the
 * full `AppState` shape (no circular imports). The host store
 * passes its own `set` and the partial-state spread works because
 * Zustand's `set` always accepts a `Partial<AppState>`.
 */
type ThemeSet = (partial: Partial<ThemeSlice>) => void

export interface ThemeSlice {
  theme: 'light' | 'dark'
  setTheme: (t: 'light' | 'dark') => void
}

export function createThemeSlice(set: ThemeSet): ThemeSlice {
  return {
    theme: (localStorage.getItem('kangnam-theme') as 'light' | 'dark') || 'dark',
    setTheme: (t) => {
      localStorage.setItem('kangnam-theme', t)
      set({ theme: t })
    },
  }
}
