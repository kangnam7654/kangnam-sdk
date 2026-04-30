/**
 * Main-view slice — which top-level pane is active and the Studio
 * sub-state.
 *
 * Studio is grouped here because `openStudio` flips
 * `activeMainView` to `'studio'` as a side effect; splitting them
 * would create a two-slice coupling that's worse than the current
 * combination.
 */
import type { MainView, StudioBottomTab, StudioState } from '../app-store'

export interface MainViewSlice {
  activeMainView: MainView
  setActiveMainView: (view: MainView) => void

  studioState: StudioState | null
  openStudio: (type: 'skill' | 'agent', name?: string) => void
  closeStudio: () => void
  setStudioBottomTab: (tab: StudioBottomTab) => void
  toggleStudioBottomPanel: () => void
  setStudioDirty: (dirty: boolean) => void
}

type MainViewSet = (
  partial: Partial<MainViewSlice> | ((state: MainViewSlice) => Partial<MainViewSlice>),
) => void

export function createMainViewSlice(set: MainViewSet): MainViewSlice {
  return {
    activeMainView: 'chat',
    setActiveMainView: (view) => set({ activeMainView: view }),

    studioState: null,
    openStudio: (type, name) =>
      set({
        activeMainView: 'studio',
        studioState: {
          type,
          name,
          activeView: name ? 'editor' : 'dashboard',
          bottomTab: 'cli',
          bottomPanelVisible: false,
          dirty: false,
        },
      }),
    closeStudio: () =>
      set({
        // Note: studioState is reset to a sentinel rather than null
        // so the Studio view doesn't unmount mid-transition. Matches
        // pre-extraction behavior verbatim.
        studioState: {
          type: 'skill',
          activeView: 'dashboard',
          bottomTab: 'cli',
          bottomPanelVisible: false,
          dirty: false,
        },
      }),
    setStudioBottomTab: (tab) =>
      set((s) => ({
        studioState: s.studioState
          ? { ...s.studioState, bottomTab: tab, bottomPanelVisible: true }
          : null,
      })),
    toggleStudioBottomPanel: () =>
      set((s) => ({
        studioState: s.studioState
          ? { ...s.studioState, bottomPanelVisible: !s.studioState.bottomPanelVisible }
          : null,
      })),
    setStudioDirty: (dirty) =>
      set((s) => ({
        studioState: s.studioState ? { ...s.studioState, dirty } : null,
      })),
  }
}
