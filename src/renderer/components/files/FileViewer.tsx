/**
 * FileViewer — read/write text editor for the design-mode FileWorkspace.
 *
 * Adapted from open-design `components/FileViewer.tsx` (Apache-2.0)
 * but with:
 * - HTTP fetch / write replaced by Tauri `project_file_*` invokes
 * - Monaco swapped for a plain `<textarea>` (Monaco is heavy and
 *   already pulled in by Studio mode; FileWorkspace gets a plain
 *   editor in v1, Monaco upgrade can land later)
 * - shiki highlighting deferred to a future commit; v1 shows the raw
 *   body in a monospaced container
 *
 * Save is debounced (auto-save 1.2s after last edit) and shows a
 * dirty indicator. Errors surface in a red banner.
 */
import { useCallback, useEffect, useRef, useState } from 'react'

interface Props {
  /** Project workspace root. */
  workingDir: string
  /** POSIX-style path relative to workingDir. */
  path: string
  /** Read-only when true. */
  readOnly?: boolean
}

interface DesignFilesApi {
  read: (workingDir: string, relPath: string) => Promise<string>
  write: (workingDir: string, relPath: string, body: string) => Promise<void>
}

function getApi(): DesignFilesApi | null {
  const api = (window as unknown as {
    api?: { project?: { fileRead?: DesignFilesApi['read']; fileWrite?: DesignFilesApi['write'] } }
  }).api?.project
  if (!api?.fileRead || !api.fileWrite) return null
  return { read: api.fileRead, write: api.fileWrite }
}

export function FileViewer({ workingDir, path, readOnly }: Props) {
  const [body, setBody] = useState<string>('')
  const [original, setOriginal] = useState<string>('')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const saveTimer = useRef<number | null>(null)

  // Load body on (workingDir, path) change.
  useEffect(() => {
    let cancelled = false
    const api = getApi()
    setLoading(true)
    setError(null)
    if (!api) {
      setError('호스트가 file API를 제공하지 않습니다 (web-only 빌드).')
      setLoading(false)
      return
    }
    api
      .read(workingDir, path)
      .then((b) => {
        if (cancelled) return
        setBody(b)
        setOriginal(b)
        setLoading(false)
      })
      .catch((e) => {
        if (cancelled) return
        setError(String(e))
        setLoading(false)
      })
    return () => {
      cancelled = true
      if (saveTimer.current !== null) {
        window.clearTimeout(saveTimer.current)
        saveTimer.current = null
      }
    }
  }, [workingDir, path])

  const flushSave = useCallback(async () => {
    const api = getApi()
    if (!api || readOnly) return
    setSaving(true)
    try {
      await api.write(workingDir, path, body)
      setOriginal(body)
      setError(null)
    } catch (e) {
      setError(String(e))
    } finally {
      setSaving(false)
    }
  }, [workingDir, path, body, readOnly])

  const onChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      const next = e.target.value
      setBody(next)
      if (readOnly) return
      if (saveTimer.current !== null) window.clearTimeout(saveTimer.current)
      saveTimer.current = window.setTimeout(() => {
        void flushSave()
      }, 1200)
    },
    [flushSave, readOnly],
  )

  const dirty = body !== original

  return (
    <div
      style={{
        display: 'flex',
        flexDirection: 'column',
        height: '100%',
        background: 'var(--bg-elevated)',
        border: '1px solid var(--border)',
        borderRadius: 'var(--radius-md)',
        overflow: 'hidden',
      }}
    >
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '6px 10px',
          borderBottom: '1px solid var(--border)',
          fontSize: 11,
          color: 'var(--text-muted)',
          flexShrink: 0,
        }}
      >
        <span style={{ fontFamily: 'monospace' }}>{path}</span>
        <span>
          {readOnly
            ? 'read-only'
            : saving
              ? '저장 중…'
              : dirty
                ? '편집됨 (자동저장 대기)'
                : '저장됨'}
        </span>
      </div>
      {error ? (
        <div
          style={{
            padding: 8,
            fontSize: 12,
            color: 'var(--danger)',
            background: 'rgba(255,0,0,0.05)',
            borderBottom: '1px solid var(--border)',
          }}
        >
          {error}
        </div>
      ) : null}
      {loading ? (
        <div style={{ padding: 12, color: 'var(--text-muted)', fontSize: 12 }}>로딩 중…</div>
      ) : (
        <textarea
          value={body}
          readOnly={readOnly}
          onChange={onChange}
          spellCheck={false}
          style={{
            flex: 1,
            minHeight: 0,
            border: 'none',
            outline: 'none',
            padding: 12,
            fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
            fontSize: 12,
            lineHeight: 1.5,
            background: 'transparent',
            color: 'var(--text-primary)',
            resize: 'none',
            tabSize: 2,
          }}
        />
      )}
    </div>
  )
}
