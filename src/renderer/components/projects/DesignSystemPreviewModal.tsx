/**
 * Modal preview for a single design-system DESIGN.md.
 *
 * Inspired by open-design `components/DesignSystemPreviewModal.tsx`.
 * Surfaces the full DESIGN.md body + extracted color tokens so a user
 * can browse the catalog from NewProjectPanel before choosing.
 *
 * Data source: `design.system_get(id)` Tauri command (Phase 5c-06).
 * In dev/web (no Tauri host) we render a "preview unavailable" stub.
 */
import { useEffect, useState } from 'react'

interface DesignSystemDetail {
  id: string
  name: string
  description: string
  body: string
  colors: string[]
}

interface Props {
  systemId: string
  onClose: () => void
}

export function DesignSystemPreviewModal({ systemId, onClose }: Props) {
  const [detail, setDetail] = useState<DesignSystemDetail | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    let cancelled = false
    const api = (window as unknown as {
      api?: {
        design?: {
          systemGet?: (id: string) => Promise<DesignSystemDetail>
        }
      }
    }).api?.design
    if (!api?.systemGet) {
      setError('호스트가 디자인 시스템 카탈로그를 제공하지 않습니다 (web-only 빌드).')
      setLoading(false)
      return
    }
    api
      .systemGet(systemId)
      .then((d) => {
        if (!cancelled) {
          setDetail(d)
          setLoading(false)
        }
      })
      .catch((e) => {
        if (!cancelled) {
          setError(String(e))
          setLoading(false)
        }
      })
    return () => {
      cancelled = true
    }
  }, [systemId])

  return (
    <div
      style={{
        position: 'fixed',
        inset: 0,
        zIndex: 220,
        background: 'rgba(0,0,0,0.55)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
      }}
      onClick={onClose}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          width: 720,
          maxWidth: 'calc(100vw - 48px)',
          maxHeight: 'calc(100vh - 48px)',
          background: 'var(--bg-surface)',
          borderRadius: 'var(--radius-lg)',
          padding: 24,
          boxShadow: '0 12px 48px rgba(0,0,0,0.45)',
          display: 'flex',
          flexDirection: 'column',
          gap: 14,
          overflow: 'hidden',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'baseline', justifyContent: 'space-between', gap: 12 }}>
          <div>
            <div style={{ fontSize: 18, fontWeight: 600, color: 'var(--text-primary)' }}>
              {detail?.name ?? systemId}
            </div>
            {detail?.description ? (
              <div style={{ fontSize: 12, color: 'var(--text-muted)', marginTop: 4 }}>
                {detail.description}
              </div>
            ) : null}
          </div>
          <button
            onClick={onClose}
            style={{
              padding: '6px 12px',
              borderRadius: 'var(--radius-md)',
              border: '1px solid var(--border)',
              background: 'transparent',
              color: 'var(--text-secondary)',
              fontSize: 12,
              cursor: 'pointer',
            }}
          >
            닫기
          </button>
        </div>

        {loading ? (
          <div style={{ color: 'var(--text-muted)', fontSize: 12 }}>로딩 중…</div>
        ) : error ? (
          <div
            style={{
              fontSize: 12,
              color: 'var(--danger)',
              padding: 8,
              borderRadius: 6,
              background: 'rgba(255,0,0,0.05)',
            }}
          >
            {error}
          </div>
        ) : detail ? (
          <>
            {detail.colors.length > 0 ? (
              <div>
                <div
                  style={{
                    fontSize: 11,
                    fontWeight: 500,
                    color: 'var(--text-secondary)',
                    marginBottom: 6,
                    textTransform: 'uppercase',
                    letterSpacing: '0.06em',
                  }}
                >
                  Color tokens
                </div>
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
                  {detail.colors.map((c) => (
                    <div
                      key={c}
                      title={c}
                      style={{
                        display: 'inline-flex',
                        alignItems: 'center',
                        gap: 6,
                        padding: '4px 8px',
                        background: 'var(--bg-elevated)',
                        border: '1px solid var(--border)',
                        borderRadius: 99,
                        fontSize: 11,
                        fontFamily: 'monospace',
                        color: 'var(--text-secondary)',
                      }}
                    >
                      <span
                        style={{
                          width: 14,
                          height: 14,
                          borderRadius: 3,
                          background: c,
                          border: '1px solid rgba(0,0,0,0.1)',
                        }}
                      />
                      {c}
                    </div>
                  ))}
                </div>
              </div>
            ) : null}

            <div
              style={{
                flex: 1,
                minHeight: 0,
                overflow: 'auto',
                padding: 14,
                background: 'var(--bg-elevated)',
                borderRadius: 'var(--radius-md)',
                border: '1px solid var(--border)',
                fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
                fontSize: 12,
                color: 'var(--text-primary)',
                whiteSpace: 'pre-wrap',
                lineHeight: 1.5,
              }}
            >
              {detail.body}
            </div>
          </>
        ) : null}
      </div>
    </div>
  )
}
