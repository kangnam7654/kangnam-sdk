/**
 * Comments slice — pin annotations on the design preview iframe.
 *
 * Comments live keyed by `artifactId`; each comment has an x/y
 * position (CSS pixels relative to the iframe), an author, body
 * text, status (open/resolved), and timestamps.
 *
 * Phase 5d-03 ships an in-memory store. A later commit can persist
 * to SQLite via a Tauri command if cross-session continuity matters.
 *
 * Adapted in spirit from open-codesign comment store (MIT).
 */
import type { StateCreator } from 'zustand'

export interface PinPosition {
  /** [0, 1] x position relative to iframe width — survives resizes. */
  xRatio: number
  /** [0, 1] y position relative to iframe height. */
  yRatio: number
}

export interface Comment {
  id: string
  artifactId: string
  author: string
  body: string
  position: PinPosition
  status: 'open' | 'resolved'
  createdAt: number
  updatedAt: number
}

export interface CommentsSlice {
  comments: Record<string, Comment>
  /** Whether the comment-mode pin overlay is active. */
  commentMode: boolean

  setCommentMode: (on: boolean) => void
  addComment: (comment: Comment) => void
  updateComment: (id: string, patch: Partial<Comment>) => void
  removeComment: (id: string) => void
  /** All comments for a single artifact, in createdAt order. */
  getCommentsForArtifact: (artifactId: string) => Comment[]
}

type CommentsSet = (
  partial: Partial<CommentsSlice> | ((state: CommentsSlice) => Partial<CommentsSlice>),
) => void

export function createCommentsSlice(set: CommentsSet, get: () => CommentsSlice): CommentsSlice {
  return {
    comments: {},
    commentMode: false,

    setCommentMode: (on) => set({ commentMode: on }),

    addComment: (comment) =>
      set((s) => ({
        comments: { ...s.comments, [comment.id]: comment },
      })),

    updateComment: (id, patch) =>
      set((s) => {
        const cur = s.comments[id]
        if (!cur) return {}
        return {
          comments: {
            ...s.comments,
            [id]: { ...cur, ...patch, updatedAt: Date.now() },
          },
        }
      }),

    removeComment: (id) =>
      set((s) => {
        if (!(id in s.comments)) return {}
        const next = { ...s.comments }
        delete next[id]
        return { comments: next }
      }),

    getCommentsForArtifact: (artifactId) => {
      const all = Object.values(get().comments)
      return all
        .filter((c) => c.artifactId === artifactId)
        .sort((a, b) => a.createdAt - b.createdAt)
    },
  }
}

// Re-export the StateCreator alias for callers that compose this with
// other slices (mirrors the pattern in projects.ts / artifacts.ts).
export type CommentsCreator = StateCreator<CommentsSlice, [], [], CommentsSlice>
