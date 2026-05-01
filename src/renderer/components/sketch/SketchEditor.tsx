/**
 * SketchEditor — minimal HTML5 canvas sketching surface so the user
 * can rough out a layout, capture it as a base64 PNG, and attach it
 * to a chat message.
 *
 * Adapted from open-design `components/SketchEditor.tsx` (Apache-2.0)
 * but trimmed: pen + eraser + clear in v1; layers / undo stack /
 * smoothing land later if needed.
 *
 * The component owns its canvas; on save it calls `onSave(base64)`
 * with a `data:image/png;base64,...` URI the caller can stuff into a
 * message attachment or a tool input.
 */
import { useCallback, useEffect, useRef, useState } from 'react'

interface Props {
  width?: number
  height?: number
  onSave: (dataUri: string) => void
  onCancel: () => void
}

type Tool = 'pen' | 'eraser'

export function SketchEditor({ width = 640, height = 420, onSave, onCancel }: Props) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null)
  const [tool, setTool] = useState<Tool>('pen')
  const [strokeColor, setStrokeColor] = useState('#1f2937')
  const [strokeWidth, setStrokeWidth] = useState(3)
  const drawingRef = useRef(false)
  const lastPosRef = useRef<{ x: number; y: number } | null>(null)

  // Initialize white background once on mount so saved PNGs aren't
  // transparent (LLMs handle white-bg sketches more reliably).
  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')
    if (!ctx) return
    ctx.fillStyle = '#ffffff'
    ctx.fillRect(0, 0, canvas.width, canvas.height)
  }, [])

  const localPos = useCallback(
    (e: React.MouseEvent | React.TouchEvent) => {
      const canvas = canvasRef.current
      if (!canvas) return null
      const rect = canvas.getBoundingClientRect()
      const point =
        'touches' in e
          ? e.touches[0] ?? e.changedTouches?.[0] ?? null
          : e
      if (!point) return null
      const cx = (point as { clientX: number; clientY: number }).clientX
      const cy = (point as { clientX: number; clientY: number }).clientY
      return {
        x: ((cx - rect.left) / rect.width) * canvas.width,
        y: ((cy - rect.top) / rect.height) * canvas.height,
      }
    },
    [],
  )

  function start(e: React.MouseEvent | React.TouchEvent) {
    e.preventDefault()
    drawingRef.current = true
    lastPosRef.current = localPos(e)
  }

  function move(e: React.MouseEvent | React.TouchEvent) {
    if (!drawingRef.current) return
    e.preventDefault()
    const canvas = canvasRef.current
    const ctx = canvas?.getContext('2d')
    const last = lastPosRef.current
    const next = localPos(e)
    if (!canvas || !ctx || !last || !next) return
    ctx.lineCap = 'round'
    ctx.lineJoin = 'round'
    ctx.lineWidth = tool === 'eraser' ? Math.max(strokeWidth * 4, 18) : strokeWidth
    ctx.strokeStyle = tool === 'eraser' ? '#ffffff' : strokeColor
    ctx.beginPath()
    ctx.moveTo(last.x, last.y)
    ctx.lineTo(next.x, next.y)
    ctx.stroke()
    lastPosRef.current = next
  }

  function end() {
    drawingRef.current = false
    lastPosRef.current = null
  }

  function clear() {
    const canvas = canvasRef.current
    const ctx = canvas?.getContext('2d')
    if (!canvas || !ctx) return
    ctx.fillStyle = '#ffffff'
    ctx.fillRect(0, 0, canvas.width, canvas.height)
  }

  function save() {
    const canvas = canvasRef.current
    if (!canvas) return
    onSave(canvas.toDataURL('image/png'))
  }

  return (
    <div
      style={{
        position: 'fixed',
        inset: 0,
        zIndex: 240,
        background: 'rgba(0,0,0,0.55)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
      }}
      onClick={onCancel}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          background: 'var(--bg-surface)',
          padding: 16,
          borderRadius: 'var(--radius-lg)',
          display: 'flex',
          flexDirection: 'column',
          gap: 10,
          boxShadow: '0 12px 48px rgba(0,0,0,0.45)',
        }}
      >
        {/* Toolbar */}
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          <ToolBtn active={tool === 'pen'} onClick={() => setTool('pen')}>
            Pen
          </ToolBtn>
          <ToolBtn active={tool === 'eraser'} onClick={() => setTool('eraser')}>
            Eraser
          </ToolBtn>
          <input
            type="color"
            value={strokeColor}
            onChange={(e) => setStrokeColor(e.target.value)}
            disabled={tool === 'eraser'}
            style={{
              width: 28,
              height: 24,
              border: '1px solid var(--border)',
              borderRadius: 4,
              padding: 0,
              cursor: 'pointer',
              opacity: tool === 'eraser' ? 0.4 : 1,
            }}
          />
          <input
            type="range"
            min={1}
            max={20}
            step={1}
            value={strokeWidth}
            onChange={(e) => setStrokeWidth(Number(e.target.value))}
            style={{ width: 100 }}
          />
          <span style={{ fontSize: 11, color: 'var(--text-muted)', minWidth: 24 }}>
            {strokeWidth}px
          </span>
          <div style={{ flex: 1 }} />
          <ToolBtn onClick={clear}>비우기</ToolBtn>
          <ToolBtn onClick={onCancel}>취소</ToolBtn>
          <ToolBtn primary onClick={save}>
            저장
          </ToolBtn>
        </div>

        {/* Canvas */}
        <canvas
          ref={canvasRef}
          width={width}
          height={height}
          onMouseDown={start}
          onMouseMove={move}
          onMouseUp={end}
          onMouseLeave={end}
          onTouchStart={start}
          onTouchMove={move}
          onTouchEnd={end}
          style={{
            background: '#fff',
            borderRadius: 'var(--radius-md)',
            border: '1px solid var(--border)',
            cursor: tool === 'eraser' ? 'cell' : 'crosshair',
            touchAction: 'none',
            display: 'block',
          }}
        />
      </div>
    </div>
  )
}

function ToolBtn({
  active,
  primary,
  onClick,
  children,
}: {
  active?: boolean
  primary?: boolean
  onClick: () => void
  children: React.ReactNode
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      style={{
        padding: '6px 12px',
        fontSize: 12,
        border: '1px solid var(--border)',
        borderRadius: 'var(--radius-md)',
        background: primary
          ? 'var(--accent)'
          : active
            ? 'var(--bg-active)'
            : 'transparent',
        color: primary ? '#fff' : 'var(--text-primary)',
        fontWeight: active || primary ? 500 : 400,
        cursor: 'pointer',
      }}
    >
      {children}
    </button>
  )
}
