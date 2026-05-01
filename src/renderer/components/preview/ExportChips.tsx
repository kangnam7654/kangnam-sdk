/**
 * ExportChips — 4-format download chips (HTML / MD / PDF) for the
 * active artifact. ZIP arrives in Phase 6a; PPTX wires after Phase 6b.
 *
 * Behavior:
 * - HTML / MD chips invoke the Tauri commands (Phase 5c-17), wrap the
 *   result in a Blob, and trigger a[download].
 * - PDF chip post-messages the iframe ('print' command) and falls
 *   back to the host's `window.print()` against the iframe via the
 *   contentWindow handle when available.
 *
 * Inspired by open-design `runtime/exports.ts` chip surface.
 */
import { useState } from 'react'

interface Props {
  artifactBody: string
  artifactKind?: string
  /** Optional iframe ref so PDF can target the rendered content. */
  iframeRef?: React.RefObject<HTMLIFrameElement | null>
}

interface ExportApi {
  html: (body: string) => Promise<string>
  markdown: (body: string) => Promise<string>
}

function getApi(): ExportApi | null {
  const api = (window as unknown as { api?: { artifactExport?: ExportApi } }).api?.artifactExport
  return api ?? null
}

function downloadBlob(content: string, mime: string, filename: string) {
  const blob = new Blob([content], { type: mime })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.style.display = 'none'
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
  // Defer revoke so the browser can finish the download read.
  setTimeout(() => URL.revokeObjectURL(url), 1000)
}

export function ExportChips({ artifactBody, artifactKind, iframeRef }: Props) {
  const [busy, setBusy] = useState<null | 'html' | 'md' | 'pdf'>(null)
  const [error, setError] = useState<string | null>(null)

  async function exportHtml() {
    const api = getApi()
    if (!api) {
      setError('호스트가 export API를 제공하지 않습니다.')
      return
    }
    setBusy('html')
    setError(null)
    try {
      const html = await api.html(artifactBody)
      downloadBlob(html, 'text/html', `${artifactKind ?? 'artifact'}.html`)
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(null)
    }
  }

  async function exportMd() {
    const api = getApi()
    if (!api) {
      setError('호스트가 export API를 제공하지 않습니다.')
      return
    }
    setBusy('md')
    setError(null)
    try {
      const md = await api.markdown(artifactBody)
      downloadBlob(md, 'text/markdown', `${artifactKind ?? 'artifact'}.md`)
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(null)
    }
  }

  function exportPdf() {
    setBusy('pdf')
    setError(null)
    try {
      const win = iframeRef?.current?.contentWindow
      if (win) {
        win.focus()
        win.print()
      } else {
        window.print()
      }
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(null)
    }
  }

  return (
    <div style={{ display: 'inline-flex', gap: 4, alignItems: 'center' }}>
      <Chip onClick={exportHtml} busy={busy === 'html'}>
        HTML
      </Chip>
      <Chip onClick={exportMd} busy={busy === 'md'}>
        MD
      </Chip>
      <Chip onClick={exportPdf} busy={busy === 'pdf'}>
        PDF
      </Chip>
      {error ? (
        <span style={{ fontSize: 11, color: 'var(--danger)', marginLeft: 6 }}>{error}</span>
      ) : null}
    </div>
  )
}

function Chip({
  children,
  onClick,
  busy,
}: {
  children: React.ReactNode
  onClick: () => void
  busy: boolean
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={busy}
      style={{
        fontSize: 10,
        padding: '3px 8px',
        borderRadius: 4,
        border: '1px solid var(--border)',
        background: busy ? 'var(--bg-active)' : 'var(--bg-surface)',
        color: 'var(--text-secondary)',
        cursor: busy ? 'wait' : 'pointer',
        textTransform: 'uppercase',
        letterSpacing: '0.04em',
        fontWeight: 500,
        transition: 'background 0.15s',
      }}
    >
      {children}
    </button>
  )
}
