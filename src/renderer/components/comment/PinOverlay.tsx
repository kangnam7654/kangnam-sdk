/**
 * PinOverlay — overlay div that captures clicks while comment-mode
 * is active and renders existing pin markers + popovers.
 *
 * Adapted from open-codesign components/comment/{PinOverlay,
 * CommentBubble,InlineCommentComposer} (MIT). Trimmed: single overlay
 * file in v1; we can split if it grows.
 *
 * Coordinates are stored as ratios in [0,1] so pins survive iframe
 * resizes (frame mode toggles, container resizes via ResizeObserver).
 */
import { useState } from 'react'

import { useAppStore } from '../../stores/app-store'
import type { Comment } from '../../stores/slices/comments'

interface Props {
  /** The artifact whose comments + new clicks belong to. */
  artifactId: string
  /** Optional author label for new comments. */
  author?: string
}

export function PinOverlay({ artifactId, author = '나' }: Props) {
  const commentMode = useAppStore((s) => s.commentMode)
  const comments = useAppStore((s) => s.getCommentsForArtifact(artifactId))
  const addComment = useAppStore((s) => s.addComment)
  const updateComment = useAppStore((s) => s.updateComment)
  const removeComment = useAppStore((s) => s.removeComment)
  const [draft, setDraft] = useState<{ x: number; y: number } | null>(null)
  const [openCommentId, setOpenCommentId] = useState<string | null>(null)

  function handleClick(e: React.MouseEvent<HTMLDivElement>) {
    if (!commentMode) return
    const rect = e.currentTarget.getBoundingClientRect()
    const x = (e.clientX - rect.left) / rect.width
    const y = (e.clientY - rect.top) / rect.height
    setDraft({ x, y })
  }

  function commitDraft(body: string) {
    if (!draft || !body.trim()) {
      setDraft(null)
      return
    }
    const id = crypto.randomUUID()
    const now = Date.now()
    addComment({
      id,
      artifactId,
      author,
      body: body.trim(),
      position: { xRatio: draft.x, yRatio: draft.y },
      status: 'open',
      createdAt: now,
      updatedAt: now,
    })
    setDraft(null)
  }

  return (
    <div
      onClick={handleClick}
      style={{
        position: 'absolute',
        inset: 0,
        // Pointer events only when comment-mode is on so the iframe
        // remains interactive otherwise.
        pointerEvents: commentMode ? 'auto' : 'none',
        cursor: commentMode ? 'crosshair' : 'default',
      }}
    >
      {comments.map((c, i) => (
        <PinMarker
          key={c.id}
          index={i + 1}
          comment={c}
          open={openCommentId === c.id}
          onToggle={() => setOpenCommentId(openCommentId === c.id ? null : c.id)}
          onResolve={() =>
            updateComment(c.id, {
              status: c.status === 'open' ? 'resolved' : 'open',
            })
          }
          onDelete={() => {
            removeComment(c.id)
            if (openCommentId === c.id) setOpenCommentId(null)
          }}
        />
      ))}
      {draft ? (
        <DraftBubble
          x={draft.x}
          y={draft.y}
          onCancel={() => setDraft(null)}
          onSubmit={commitDraft}
        />
      ) : null}
    </div>
  )
}

interface PinMarkerProps {
  index: number
  comment: Comment
  open: boolean
  onToggle: () => void
  onResolve: () => void
  onDelete: () => void
}

function PinMarker({ index, comment, open, onToggle, onResolve, onDelete }: PinMarkerProps) {
  const x = comment.position.xRatio * 100
  const y = comment.position.yRatio * 100
  const resolved = comment.status === 'resolved'
  return (
    <>
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation()
          onToggle()
        }}
        style={{
          position: 'absolute',
          left: `calc(${x}% - 12px)`,
          top: `calc(${y}% - 12px)`,
          width: 24,
          height: 24,
          borderRadius: '50%',
          background: resolved ? 'var(--text-muted)' : 'var(--accent)',
          color: '#fff',
          border: '2px solid #fff',
          fontSize: 11,
          fontWeight: 600,
          cursor: 'pointer',
          padding: 0,
          opacity: resolved ? 0.6 : 1,
          boxShadow: '0 2px 6px rgba(0,0,0,0.3)',
        }}
      >
        {index}
      </button>
      {open ? (
        <CommentBubble
          comment={comment}
          x={x}
          y={y}
          onClose={onToggle}
          onResolve={onResolve}
          onDelete={onDelete}
        />
      ) : null}
    </>
  )
}

interface CommentBubbleProps {
  comment: Comment
  x: number
  y: number
  onClose: () => void
  onResolve: () => void
  onDelete: () => void
}

function CommentBubble({ comment, x, y, onClose, onResolve, onDelete }: CommentBubbleProps) {
  return (
    <div
      onClick={(e) => e.stopPropagation()}
      style={{
        ...bubbleStyle,
        left: `calc(${x}% + 16px)`,
        top: `calc(${y}% - 4px)`,
      }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', gap: 6, marginBottom: 4 }}>
        <span style={{ fontWeight: 600, fontSize: 11, color: 'var(--text-primary)' }}>
          {comment.author}
        </span>
        <span style={{ fontSize: 10, color: 'var(--text-muted)' }}>
          {comment.status === 'resolved' ? '해결됨' : '오픈'}
        </span>
      </div>
      <div
        style={{
          fontSize: 12,
          color: 'var(--text-primary)',
          whiteSpace: 'pre-wrap',
          marginBottom: 8,
          textDecoration: comment.status === 'resolved' ? 'line-through' : 'none',
        }}
      >
        {comment.body}
      </div>
      <div style={{ display: 'flex', gap: 6 }}>
        <BubbleButton onClick={onResolve}>
          {comment.status === 'resolved' ? '재오픈' : '해결'}
        </BubbleButton>
        <BubbleButton onClick={onDelete}>삭제</BubbleButton>
        <div style={{ flex: 1 }} />
        <BubbleButton onClick={onClose}>닫기</BubbleButton>
      </div>
    </div>
  )
}

interface DraftBubbleProps {
  x: number
  y: number
  onCancel: () => void
  onSubmit: (body: string) => void
}

function DraftBubble({ x, y, onCancel, onSubmit }: DraftBubbleProps) {
  const [body, setBody] = useState('')
  return (
    <div
      onClick={(e) => e.stopPropagation()}
      style={{
        ...bubbleStyle,
        left: `calc(${x * 100}% + 16px)`,
        top: `calc(${y * 100}% - 4px)`,
      }}
    >
      <textarea
        autoFocus
        rows={3}
        placeholder="코멘트 입력..."
        value={body}
        onChange={(e) => setBody(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
            e.preventDefault()
            onSubmit(body)
          } else if (e.key === 'Escape') {
            onCancel()
          }
        }}
        style={{
          width: '100%',
          padding: 6,
          fontSize: 12,
          fontFamily: 'inherit',
          resize: 'vertical',
          background: 'var(--bg-elevated)',
          border: '1px solid var(--border)',
          borderRadius: 4,
          color: 'var(--text-primary)',
          marginBottom: 6,
        }}
      />
      <div style={{ display: 'flex', gap: 6 }}>
        <BubbleButton onClick={onCancel}>취소</BubbleButton>
        <div style={{ flex: 1 }} />
        <BubbleButton onClick={() => onSubmit(body)}>등록 (⌘↵)</BubbleButton>
      </div>
    </div>
  )
}

function BubbleButton({
  onClick,
  children,
}: {
  onClick: () => void
  children: React.ReactNode
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      style={{
        fontSize: 11,
        padding: '4px 8px',
        border: '1px solid var(--border)',
        background: 'var(--bg-surface)',
        color: 'var(--text-secondary)',
        borderRadius: 4,
        cursor: 'pointer',
      }}
    >
      {children}
    </button>
  )
}

const bubbleStyle: React.CSSProperties = {
  position: 'absolute',
  width: 220,
  padding: 10,
  background: 'var(--bg-surface)',
  border: '1px solid var(--border)',
  borderRadius: 'var(--radius-md)',
  boxShadow: '0 4px 16px rgba(0,0,0,0.25)',
  zIndex: 10,
}
