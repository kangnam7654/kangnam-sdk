/**
 * TweakPanel — surfaces editmode `@tweak` blocks from the active
 * artifact body as live controls (color picker / slider / select /
 * text input). Edits flush back into the artifact body via
 * `replaceTweakValue`.
 *
 * Adapted from open-codesign `components/TweakPanel.tsx` (MIT).
 *
 * Two write paths exist:
 * 1. **Local-only edit** (default) — updates the artifact slice's
 *    body in place so PreviewIframe re-renders immediately. The
 *    underlying file isn't modified.
 * 2. **Persisted edit** (when `workingDir` + `path` props supplied)
 *    — also writes the new body to disk via the project_file_write
 *    Tauri command. Requires the artifact to live in the project
 *    workspace.
 */
import { useMemo } from 'react'

import { parseEditmodeBlocks, replaceTweakValue, type TweakBlock } from '../../lib/editmode/parser'
import { useAppStore } from '../../stores/app-store'

interface Props {
  artifactId: string | null | undefined
  /** When supplied, writes the new body to disk after each edit. */
  workingDir?: string | null
  /** Project-relative path for the artifact file. Required to persist. */
  path?: string | null
}

interface ProjectApi {
  fileWrite: (workingDir: string, relPath: string, body: string) => Promise<void>
}

function getProjectApi(): ProjectApi | null {
  const api = (window as unknown as {
    api?: { project?: { fileWrite?: ProjectApi['fileWrite'] } }
  }).api?.project
  if (!api?.fileWrite) return null
  return { fileWrite: api.fileWrite }
}

export function TweakPanel({ artifactId, workingDir, path }: Props) {
  const artifact = useAppStore((s) => (artifactId ? s.artifacts[artifactId] : undefined))
  const setArtifactBody = useAppStore((s) => s.setArtifactBody)

  const blocks = useMemo<TweakBlock[]>(
    () => (artifact?.body ? parseEditmodeBlocks(artifact.body) : []),
    [artifact?.body],
  )

  function handleChange(id: string, next: string) {
    if (!artifact || !artifactId) return
    const updated = replaceTweakValue(artifact.body, id, next)
    setArtifactBody(artifactId, updated)
    if (workingDir && path) {
      const api = getProjectApi()
      api?.fileWrite(workingDir, path, updated).catch((e) => {
        console.warn('[TweakPanel] persistence failed:', e)
      })
    }
  }

  if (!artifact) {
    return (
      <Empty>
        활성 아티팩트가 없습니다. 미리보기가 로드된 후 컨트롤이 표시됩니다.
      </Empty>
    )
  }
  if (blocks.length === 0) {
    return (
      <Empty>
        이 아티팩트에 정의된 <code>@tweak</code> 블록이 없습니다.
      </Empty>
    )
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 10, padding: 10 }}>
      {blocks.map((b) => (
        <Row key={b.id} block={b} onChange={(v) => handleChange(b.id, v)} />
      ))}
    </div>
  )
}

interface RowProps {
  block: TweakBlock
  onChange: (next: string) => void
}

function Row({ block, onChange }: RowProps) {
  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 4,
        padding: '8px 10px',
        background: 'var(--bg-elevated)',
        border: '1px solid var(--border)',
        borderRadius: 'var(--radius-md)',
      }}
    >
      <div style={{ fontSize: 11, color: 'var(--text-secondary)', fontWeight: 500 }}>
        {block.label}
      </div>
      {block.type === 'color' ? (
        <ColorControl value={block.value} onChange={onChange} />
      ) : block.type === 'slider' ? (
        <SliderControl
          value={block.value}
          min={block.min}
          max={block.max}
          step={block.step}
          onChange={onChange}
        />
      ) : block.type === 'select' ? (
        <SelectControl value={block.value} options={block.options ?? []} onChange={onChange} />
      ) : (
        <TextControl value={block.value} onChange={onChange} />
      )}
    </div>
  )
}

function ColorControl({
  value,
  onChange,
}: {
  value: string
  onChange: (v: string) => void
}) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
      <input
        type="color"
        value={normalizeColor(value)}
        onChange={(e) => onChange(e.target.value)}
        style={{
          width: 32,
          height: 24,
          border: '1px solid var(--border)',
          borderRadius: 4,
          background: 'transparent',
          padding: 0,
          cursor: 'pointer',
        }}
      />
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        style={textInputStyle}
      />
    </div>
  )
}

function SliderControl({
  value,
  min,
  max,
  step,
  onChange,
}: {
  value: string
  min?: number
  max?: number
  step?: number
  onChange: (v: string) => void
}) {
  const numeric = Number(value)
  const safeMin = min ?? 0
  const safeMax = max ?? 100
  const safeStep = step ?? 1
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
      <input
        type="range"
        min={safeMin}
        max={safeMax}
        step={safeStep}
        value={Number.isFinite(numeric) ? numeric : safeMin}
        onChange={(e) => onChange(e.target.value)}
        style={{ flex: 1 }}
      />
      <span style={{ fontSize: 11, color: 'var(--text-muted)', minWidth: 32, textAlign: 'right' }}>
        {value}
      </span>
    </div>
  )
}

function SelectControl({
  value,
  options,
  onChange,
}: {
  value: string
  options: string[]
  onChange: (v: string) => void
}) {
  return (
    <select value={value} onChange={(e) => onChange(e.target.value)} style={textInputStyle}>
      {options.map((opt) => (
        <option key={opt} value={opt}>
          {opt}
        </option>
      ))}
    </select>
  )
}

function TextControl({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  return (
    <input
      type="text"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      style={textInputStyle}
    />
  )
}

function Empty({ children }: { children: React.ReactNode }) {
  return (
    <div
      style={{
        padding: 16,
        fontSize: 12,
        color: 'var(--text-muted)',
        textAlign: 'center',
      }}
    >
      {children}
    </div>
  )
}

function normalizeColor(v: string): string {
  // <input type="color"> only accepts 6-digit hex; fall back to black if
  // the value isn't parseable so the picker UI doesn't disappear.
  const m = v.match(/^#([0-9a-fA-F]{6})$/)
  return m ? `#${m[1]}` : '#000000'
}

const textInputStyle: React.CSSProperties = {
  width: '100%',
  padding: '4px 8px',
  borderRadius: 4,
  border: '1px solid var(--border)',
  background: 'var(--bg-surface)',
  color: 'var(--text-primary)',
  fontSize: 12,
  fontFamily: 'inherit',
}
