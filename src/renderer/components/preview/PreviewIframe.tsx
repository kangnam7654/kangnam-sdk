/**
 * Sandboxed iframe preview for design artifacts.
 *
 * Two render paths:
 *
 * 1. **Artifact-store driven** — when `artifactId` prop is supplied,
 *    the component reads the in-flight body from the `artifacts`
 *    slice (Phase 5b-03), wraps it via `buildSrcdoc`, and renders
 *    the result into a sandboxed iframe with `srcdoc=`. Updates as
 *    the body grows.
 *
 * 2. **Tool-request driven** — listens for `cli.previewRequest`
 *    notifications from the host MCP server (Phase 4d
 *    `mcp__kangnam__preview`). When a request arrives, the host
 *    wants a screenshot + console capture of the artifact at
 *    `path`. Phase 5b doesn't ship a screenshot mechanism (Tauri
 *    webview-side; lands in 5c), so we capture the console buffer
 *    via the postMessage bridge in `srcdoc.ts` and POST it back via
 *    `previewResult(id, payload)`.
 */
import { useEffect, useRef, useState } from 'react'

import { cliApi } from '../../lib/cli-api'
import { buildSrcdoc } from '../../lib/runtime/srcdoc'
import { useAppStore } from '../../stores/app-store'
import { FrameToolbar, PhoneFrame, type FrameMode } from './PhoneFrame'
import { ExportChips } from './ExportChips'

interface ConsoleMsg {
  kind: 'console' | 'error' | 'unhandled-rejection' | 'ready'
  level?: 'log' | 'info' | 'warn' | 'error' | 'debug'
  message?: string
  source?: string
  line?: number
  col?: number
  reason?: string
}

const CHANNEL = 'design-artifact-iframe'

interface Props {
  /** Artifact id from the artifacts slice; iframe streams its body. */
  artifactId?: string
  /** Width / height in CSS pixels. Defaults sensibly to the parent. */
  width?: number | string
  height?: number | string
  /** Show a translucent header with the kind + a console-count badge. */
  showHeader?: boolean
  /** Initial device-frame mode. User can still toggle via the toolbar. */
  initialFrame?: FrameMode
  /** Hide the frame toggle (for embeds that pin a single mode). */
  hideFrameToggle?: boolean
}

export function PreviewIframe({
  artifactId,
  width = '100%',
  height = '100%',
  showHeader = true,
  initialFrame = 'desktop',
  hideFrameToggle = false,
}: Props) {
  const artifact = useAppStore((s) => (artifactId ? s.artifacts[artifactId] : undefined))
  const [consoleBuf, setConsoleBuf] = useState<ConsoleMsg[]>([])
  const [frame, setFrame] = useState<FrameMode>(initialFrame)
  const containerRef = useRef<HTMLDivElement | null>(null)
  const [containerSize, setContainerSize] = useState<{ w: number; h: number } | null>(null)

  useEffect(() => {
    if (!containerRef.current) return
    const el = containerRef.current
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const r = entry.contentRect
        setContainerSize({ w: r.width, h: r.height })
      }
    })
    ro.observe(el)
    return () => ro.disconnect()
  }, [])
  const [pendingRequest, setPendingRequest] = useState<
    | { id: string; path: string; viewport?: { width: number; height: number } }
    | null
  >(null)
  const iframeRef = useRef<HTMLIFrameElement | null>(null)

  // Subscribe to MCP `cli.previewRequest` notifications. When one
  // arrives we hold it in state and reply once the iframe has had a
  // moment to load + the bridge has fired any console messages.
  useEffect(() => {
    const unsub = cliApi.onPreviewRequest((req) => {
      setConsoleBuf([])
      setPendingRequest(req)
    })
    return unsub
  }, [])

  // Listen for the postMessage bridge from the iframe srcdoc shim.
  useEffect(() => {
    function handler(ev: MessageEvent) {
      const data = ev.data as { channel?: string } & ConsoleMsg
      if (!data || data.channel !== CHANNEL) return
      setConsoleBuf((prev) => [...prev, data as ConsoleMsg])
    }
    window.addEventListener('message', handler)
    return () => window.removeEventListener('message', handler)
  }, [])

  // After a tool-request preview has had ~600ms to render and emit
  // any console messages, post the response. We don't yet ship a
  // screenshot — Phase 5c adds the Tauri webview-side capture.
  useEffect(() => {
    if (!pendingRequest) return
    const id = pendingRequest.id
    const t = setTimeout(() => {
      void cliApi
        .previewResult(id, {
          screenshot: null,
          console: consoleBuf,
          errors: consoleBuf.filter(
            (m) => m.kind === 'error' || m.kind === 'unhandled-rejection',
          ),
        })
        .catch((e) => {
          console.error('[PreviewIframe] previewResult failed:', e)
        })
      setPendingRequest(null)
    }, 600)
    return () => clearTimeout(t)
  }, [pendingRequest, consoleBuf])

  const srcdoc = artifact?.body ? buildSrcdoc(artifact.body) : null

  return (
    <div
      style={{
        position: 'relative',
        width,
        height,
        background: 'var(--bg-elevated)',
        borderRadius: 'var(--radius-md)',
        overflow: 'hidden',
        border: '1px solid var(--border)',
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      {showHeader ? (
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            padding: '6px 10px',
            background: 'var(--bg-surface)',
            borderBottom: '1px solid var(--border)',
            fontSize: 11,
            color: 'var(--text-muted)',
            flexShrink: 0,
          }}
        >
          <span>
            {artifact ? `Preview · ${artifact.kind}` : 'Preview · idle'}
          </span>
          <span style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
            {artifact?.body ? (
              <ExportChips
                artifactBody={artifact.body}
                artifactKind={artifact.kind}
                iframeRef={iframeRef}
              />
            ) : null}
            {!hideFrameToggle ? (
              <FrameToolbar mode={frame} onChange={setFrame} />
            ) : null}
            {consoleBuf.length > 0 ? <span>console: {consoleBuf.length}</span> : null}
          </span>
        </div>
      ) : null}
      <div ref={containerRef} style={{ flex: 1, minHeight: 0 }}>
        {srcdoc ? (
          <PhoneFrame
            mode={frame}
            maxWidth={containerSize?.w}
            maxHeight={containerSize?.h}
          >
            <iframe
              ref={iframeRef}
              srcDoc={srcdoc}
              sandbox="allow-scripts"
              style={{ width: '100%', height: '100%', border: 'none' }}
              title="design artifact preview"
            />
          </PhoneFrame>
        ) : (
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              height: '100%',
              color: 'var(--text-muted)',
              fontSize: 12,
            }}
          >
            아직 표시할 아티팩트가 없습니다
          </div>
        )}
      </div>
    </div>
  )
}
