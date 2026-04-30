/**
 * Projects slice — design-mode workspaces.
 *
 * A `Project` is a per-design conversation root: a skill choice, an
 * (optional) design-system pick, a kept-around chat history (linked
 * via `conversationId`), and a working directory the artifact files
 * live in. The full open-design Project shape carries a `metadata`
 * union for prototype/deck/template/template-source nuances; v1
 * (Phase 5b) keeps the shape narrow and lets later phases widen it.
 *
 * Persistence: localStorage `kangnam-projects` keeps the list across
 * reloads. Phase 5c will swap this for a Tauri command-backed
 * persistence so multiple SDK frontends share the same project DB.
 *
 * Cross-slice: opening a project doesn't directly flip
 * `activeMainView` — the consumer (App.tsx) reads `activeProjectId`
 * and switches the main pane. Keeping the side effect out of this
 * slice avoids a circular dependency on the main-view slice.
 */
const PROJECTS_LS_KEY = 'kangnam-projects'
const ACTIVE_PROJECT_LS_KEY = 'kangnam-active-project-id'

export interface Project {
  id: string
  name: string
  skillId: string | null
  designSystemId: string | null
  /** Working directory on disk where the project's artifact files live. */
  workingDir: string | null
  /** Linked chat conversation; null until the first message is sent. */
  conversationId: string | null
  /** Prompt prefilled into the composer on first open; cleared after use. */
  pendingPrompt: string | null
  createdAt: number
  updatedAt: number
}

export interface ProjectsSlice {
  projects: Project[]
  activeProjectId: string | null

  setProjects: (projects: Project[]) => void
  setActiveProjectId: (id: string | null) => void
  /**
   * Insert or replace by `id`. New projects bump `createdAt`/`updatedAt`
   * to now; existing rows keep `createdAt` but bump `updatedAt`.
   */
  upsertProject: (project: Project) => void
  removeProject: (id: string) => void
  /** Clear `pendingPrompt` after the composer has consumed it. */
  consumePendingPrompt: (id: string) => void
}

type ProjectsSet = (
  partial: Partial<ProjectsSlice> | ((state: ProjectsSlice) => Partial<ProjectsSlice>),
) => void

function loadProjects(): Project[] {
  try {
    const raw = localStorage.getItem(PROJECTS_LS_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw)
    return Array.isArray(parsed) ? (parsed as Project[]) : []
  } catch {
    return []
  }
}

function saveProjects(projects: Project[]): void {
  try {
    localStorage.setItem(PROJECTS_LS_KEY, JSON.stringify(projects))
  } catch {
    /* quota / serialize fail — drop silently, projects survive in-memory */
  }
}

export function createProjectsSlice(set: ProjectsSet): ProjectsSlice {
  return {
    projects: loadProjects(),
    activeProjectId: localStorage.getItem(ACTIVE_PROJECT_LS_KEY),

    setProjects: (projects) => {
      saveProjects(projects)
      set({ projects })
    },

    setActiveProjectId: (id) => {
      if (id) localStorage.setItem(ACTIVE_PROJECT_LS_KEY, id)
      else localStorage.removeItem(ACTIVE_PROJECT_LS_KEY)
      set({ activeProjectId: id })
    },

    upsertProject: (project) =>
      set((s) => {
        const idx = s.projects.findIndex((p) => p.id === project.id)
        const now = Date.now()
        const next: Project[] = [...s.projects]
        if (idx >= 0) {
          next[idx] = { ...project, updatedAt: now }
        } else {
          next.unshift({ ...project, createdAt: project.createdAt || now, updatedAt: now })
        }
        saveProjects(next)
        return { projects: next }
      }),

    removeProject: (id) =>
      set((s) => {
        const next = s.projects.filter((p) => p.id !== id)
        saveProjects(next)
        const activeProjectId = s.activeProjectId === id ? null : s.activeProjectId
        if (activeProjectId === null) localStorage.removeItem(ACTIVE_PROJECT_LS_KEY)
        return { projects: next, activeProjectId }
      }),

    consumePendingPrompt: (id) =>
      set((s) => {
        const idx = s.projects.findIndex((p) => p.id === id)
        if (idx < 0) return {}
        const target = s.projects[idx]
        if (!target.pendingPrompt) return {}
        const updated: Project = { ...target, pendingPrompt: null, updatedAt: Date.now() }
        const next = [...s.projects]
        next[idx] = updated
        saveProjects(next)
        return { projects: next }
      }),
  }
}
