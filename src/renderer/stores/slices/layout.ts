/**
 * Layout slice — sidebar collapse + side/right panel tabs, visibility,
 * and widths.
 *
 * `SidePanelTab` and `RightPanelTab` are imported from `app-store.ts`
 * for now to keep this commit focused on extraction. A future cleanup
 * can move the tab type aliases here once nothing else in the store
 * file needs them.
 */
import type { SidePanelTab, RightPanelTab } from '../app-store'

export interface LayoutSlice {
  // Activity bar collapse
  sidebarCollapsed: boolean
  setSidebarCollapsed: (v: boolean) => void
  toggleSidebar: () => void

  // Side panel
  sidePanelTab: SidePanelTab
  setSidePanelTab: (tab: SidePanelTab) => void
  sidePanelVisible: boolean
  setSidePanelVisible: (v: boolean) => void
  /**
   * Toggle the side panel. If `tab` is supplied and is *not* the
   * currently active tab, switch to it and force-show the panel
   * (mirrors the activity-bar click behavior). Otherwise toggle
   * visibility while leaving the active tab alone.
   */
  toggleSidePanel: (tab?: SidePanelTab) => void
  sidePanelWidth: number
  setSidePanelWidth: (w: number) => void

  // Right panel
  rightPanelTab: RightPanelTab
  setRightPanelTab: (tab: RightPanelTab) => void
  rightPanelVisible: boolean
  setRightPanelVisible: (v: boolean) => void
  toggleRightPanel: () => void
  rightPanelWidth: number
  setRightPanelWidth: (w: number) => void
}

type LayoutSet = (
  partial: Partial<LayoutSlice> | ((state: LayoutSlice) => Partial<LayoutSlice>),
) => void

export function createLayoutSlice(set: LayoutSet): LayoutSlice {
  return {
    sidebarCollapsed: false,
    setSidebarCollapsed: (v) => set({ sidebarCollapsed: v }),
    toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),

    sidePanelTab: 'chats',
    setSidePanelTab: (tab) => set({ sidePanelTab: tab }),
    sidePanelVisible: true,
    setSidePanelVisible: (v) => set({ sidePanelVisible: v }),
    toggleSidePanel: (tab) =>
      set((s) => {
        if (tab && tab !== s.sidePanelTab) {
          return { sidePanelTab: tab, sidePanelVisible: true }
        }
        return { sidePanelVisible: !s.sidePanelVisible }
      }),
    sidePanelWidth: 280,
    setSidePanelWidth: (w) => set({ sidePanelWidth: w }),

    rightPanelTab: 'terminal',
    setRightPanelTab: (tab) => set({ rightPanelTab: tab }),
    rightPanelVisible: false,
    setRightPanelVisible: (v) => set({ rightPanelVisible: v }),
    toggleRightPanel: () => set((s) => ({ rightPanelVisible: !s.rightPanelVisible })),
    rightPanelWidth: 360,
    setRightPanelWidth: (w) => set({ rightPanelWidth: w }),
  }
}
