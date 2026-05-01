/**
 * Artifacts slice — streaming buffer for `<artifact>` blocks the
 * agent emits during a turn.
 *
 * The chat-server transforms `text_delta` events that contain
 * artifact tags into `artifact_start` / `artifact_delta` /
 * `artifact_end` `UnifiedMessage` variants (Phase 3 +
 * `kangnam-chat-server` Phase 5b-06 hookup). This slice maintains a
 * `Record<id, ArtifactState>` that the renderer can consult to show
 * a "currently emitting" indicator and the final accumulated body.
 *
 * Why a slice rather than a hook? Because:
 * 1. The PreviewIframe in another pane needs to read the same buffer
 *    that ChatContent's stream-handler writes to.
 * 2. Project-switch should preserve in-flight artifact state so the
 *    user can flip back without losing progress.
 *
 * Manifests + artifact types live in `lib/artifacts/types.ts` (Phase
 * 5b-05); this slice just stores opaque `unknown` blobs to stay
 * decoupled from parser internals.
 */
export interface ArtifactState {
  id: string
  /** `html` | `markdown` | `image` | `slide` | … (free-form). */
  kind: string
  body: string
  /** True once `artifact_end` has been seen. */
  complete: boolean
  /** Parsed manifest (open-design `parseManifestComment`); `unknown` until 5b-05. */
  manifest: unknown
  startedAt: number
  endedAt: number | null
}

export interface ArtifactsSlice {
  artifacts: Record<string, ArtifactState>

  startArtifact: (id: string, kind: string) => void
  appendArtifactDelta: (id: string, text: string) => void
  endArtifact: (id: string, manifest: unknown) => void
  /** Clear the buffer when starting a new turn. */
  clearArtifacts: () => void
  /** Targeted removal — used when the host saves an artifact to disk. */
  removeArtifact: (id: string) => void
  /** Replace body in place — used by TweakPanel to flush edits (5d-02). */
  setArtifactBody: (id: string, body: string) => void
}

type ArtifactsSet = (
  partial: Partial<ArtifactsSlice> | ((state: ArtifactsSlice) => Partial<ArtifactsSlice>),
) => void

export function createArtifactsSlice(set: ArtifactsSet): ArtifactsSlice {
  return {
    artifacts: {},

    startArtifact: (id, kind) =>
      set((s) => ({
        artifacts: {
          ...s.artifacts,
          [id]: {
            id,
            kind,
            body: '',
            complete: false,
            manifest: null,
            startedAt: Date.now(),
            endedAt: null,
          },
        },
      })),

    appendArtifactDelta: (id, text) =>
      set((s) => {
        const cur = s.artifacts[id]
        if (!cur) return {}
        return {
          artifacts: {
            ...s.artifacts,
            [id]: { ...cur, body: cur.body + text },
          },
        }
      }),

    endArtifact: (id, manifest) =>
      set((s) => {
        const cur = s.artifacts[id]
        if (!cur) return {}
        return {
          artifacts: {
            ...s.artifacts,
            [id]: { ...cur, complete: true, manifest, endedAt: Date.now() },
          },
        }
      }),

    clearArtifacts: () => set({ artifacts: {} }),

    removeArtifact: (id) =>
      set((s) => {
        if (!(id in s.artifacts)) return {}
        const next = { ...s.artifacts }
        delete next[id]
        return { artifacts: next }
      }),

    setArtifactBody: (id, body) =>
      set((s) => {
        const cur = s.artifacts[id]
        if (!cur) return {}
        return {
          artifacts: {
            ...s.artifacts,
            [id]: { ...cur, body },
          },
        }
      }),
  }
}
