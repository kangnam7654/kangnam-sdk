/**
 * Device frame wrapper for the preview iframe — phone / tablet /
 * desktop viewport sizing with optional bezel chrome.
 *
 * Pure CSS, no Tauri / browser deps. Inspired by open-codesign's
 * `PhoneFrame.tsx` + `PreviewToolbar.tsx` (`PreviewPane.tsx:145-260`).
 *
 * Usage:
 *
 * ```tsx
 * <PhoneFrame mode="phone">
 *   <PreviewIframe artifactId={id} />
 * </PhoneFrame>
 * ```
 */
import type { ReactNode } from 'react'

export type FrameMode = 'desktop' | 'tablet' | 'phone'

interface FrameSpec {
  /** Inner viewport width — what the iframe sees. */
  width: number
  /** Inner viewport height. */
  height: number
  /** Visible bezel padding around the iframe. */
  bezel: number
  /** Border radius for the bezel + inner clip. */
  radius: number
  /** Outer chrome color. */
  chromeBg: string
}

const SPECS: Record<FrameMode, FrameSpec> = {
  desktop: {
    width: 1280,
    height: 800,
    bezel: 0,
    radius: 6,
    chromeBg: 'transparent',
  },
  tablet: {
    width: 820,
    height: 1180,
    bezel: 18,
    radius: 24,
    chromeBg: '#1a1a1a',
  },
  phone: {
    width: 390,
    height: 844,
    bezel: 14,
    radius: 36,
    chromeBg: '#0a0a0a',
  },
}

interface Props {
  mode: FrameMode
  children: ReactNode
  /** Maximum container width — for fitting into a parent without overflowing. */
  maxWidth?: number
  /** Maximum container height. */
  maxHeight?: number
}

export function PhoneFrame({ mode, children, maxWidth, maxHeight }: Props) {
  const spec = SPECS[mode]
  const outerW = spec.width + spec.bezel * 2
  const outerH = spec.height + spec.bezel * 2

  // Compute scale so the framed device fits within the container.
  const scaleW = maxWidth ? maxWidth / outerW : 1
  const scaleH = maxHeight ? maxHeight / outerH : 1
  const scale = Math.min(1, scaleW, scaleH)

  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        width: '100%',
        height: '100%',
        overflow: 'hidden',
      }}
    >
      <div
        style={{
          width: outerW,
          height: outerH,
          background: spec.chromeBg,
          borderRadius: spec.radius + spec.bezel,
          padding: spec.bezel,
          boxShadow:
            mode === 'desktop'
              ? 'none'
              : '0 12px 36px rgba(0, 0, 0, 0.35), 0 2px 8px rgba(0, 0, 0, 0.2)',
          transform: scale < 1 ? `scale(${scale})` : undefined,
          transformOrigin: 'center center',
          flexShrink: 0,
          boxSizing: 'border-box',
        }}
      >
        <div
          style={{
            width: spec.width,
            height: spec.height,
            borderRadius: spec.radius,
            overflow: 'hidden',
            background: 'var(--bg-elevated)',
          }}
        >
          {children}
        </div>
      </div>
    </div>
  )
}

interface ToolbarProps {
  mode: FrameMode
  onChange: (mode: FrameMode) => void
}

/**
 * Frame mode toggle — three pill buttons (desktop / tablet / phone).
 */
export function FrameToolbar({ mode, onChange }: ToolbarProps) {
  return (
    <div
      style={{
        display: 'inline-flex',
        gap: 2,
        padding: 2,
        background: 'var(--bg-surface)',
        border: '1px solid var(--border)',
        borderRadius: 'var(--radius-sm)',
      }}
    >
      {(['desktop', 'tablet', 'phone'] as const).map((m) => (
        <button
          key={m}
          type="button"
          onClick={() => onChange(m)}
          style={{
            padding: '4px 10px',
            fontSize: 11,
            border: 'none',
            borderRadius: 4,
            background: mode === m ? 'var(--bg-active)' : 'transparent',
            color: mode === m ? 'var(--text-primary)' : 'var(--text-muted)',
            cursor: 'pointer',
            textTransform: 'capitalize',
          }}
        >
          {m}
        </button>
      ))}
    </div>
  )
}
