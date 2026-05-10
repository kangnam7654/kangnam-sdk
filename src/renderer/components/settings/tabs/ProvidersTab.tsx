import { useEffect, useState } from 'react'
import { cliApi, type ProviderMeta } from '../../../lib/cli-api'
import type { CliStatus } from '../../../stores/app-store'

export function ProvidersTab() {
  const [providers, setProviders] = useState<ProviderMeta[]>([])
  const [statuses, setStatuses] = useState<Record<string, CliStatus>>({})
  const [installing, setInstalling] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    loadProviders()
  }, [])

  const loadProviders = async () => {
    try {
      const metas = await cliApi.listProviders()
      setProviders(metas)
      const statusMap: Record<string, CliStatus> = {}
      for (const meta of metas) {
        statusMap[meta.name] = await cliApi.checkInstalled(meta.name)
      }
      setStatuses(statusMap)
    } catch (err) {
      console.error('Failed to load CLI providers:', err)
    }
  }

  const handleInstall = async (provider: string) => {
    setInstalling(provider)
    setError(null)
    try {
      await cliApi.install(provider)
      const status = await cliApi.checkInstalled(provider)
      setStatuses((prev) => ({ ...prev, [provider]: status }))
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    } finally {
      setInstalling(null)
    }
  }

  return (
    <div>
      <SectionTitle>CLI Providers</SectionTitle>
      <SectionDesc>AI coding agent CLIs installed on your system.</SectionDesc>

      {error && (
        <div style={{ padding: '10px 14px', borderRadius: 10, background: 'rgba(239,68,68,0.1)', color: 'var(--danger)', fontSize: 13, marginBottom: 16 }}>
          {error}
        </div>
      )}

      <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
        {providers.map(meta => {
          const status = statuses[meta.name]
          const isInstalling = installing === meta.name
          const installed = status?.installed ?? false
          const authLabel = status?.auth_status === 'ready'
            ? '로그인됨'
            : status?.auth_status === 'not_logged_in'
              ? '로그인 필요'
              : status?.auth_status === 'failed'
                ? '인증 확인 실패'
                : meta.subscription_based
                  ? '인증 확인 중'
                  : null

          return (
            <div
              key={meta.name}
              style={{
                display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 16,
                padding: '14px 16px', borderRadius: 12,
                background: 'var(--bg-surface)', border: '1px solid var(--border)'
              }}
            >
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                  <span style={{ width: 8, height: 8, borderRadius: '50%', background: installed ? 'var(--success)' : 'var(--text-muted)', flexShrink: 0 }} />
                  <span style={{ fontSize: 14, fontWeight: 500, color: 'var(--text-primary)' }}>{meta.display_name}</span>
                  {installed && (
                    <span style={{ fontSize: 11, color: 'var(--success)', fontWeight: 500, padding: '2px 8px', borderRadius: 6, background: 'rgba(16,185,129,0.1)' }}>
                      v{status?.version ?? '?'}
                    </span>
                  )}
                  {installed && authLabel && (
                    <span style={{ fontSize: 11, color: status?.auth_status === 'not_logged_in' ? 'var(--warning)' : 'var(--text-muted)', fontWeight: 500, padding: '2px 8px', borderRadius: 6, background: 'rgba(255,255,255,0.05)' }}>
                      {authLabel}
                    </span>
                  )}
                </div>
                <p style={{ fontSize: 12.5, color: 'var(--text-muted)', marginLeft: 18, marginTop: 3 }}>{meta.description}</p>
                {!installed && (
                  <p style={{ fontSize: 11.5, color: 'var(--text-muted)', marginLeft: 18, marginTop: 4, fontFamily: 'monospace' }}>
                    {meta.install_hint}
                  </p>
                )}
                {installed && status?.path && (
                  <p style={{ fontSize: 11.5, color: 'var(--text-muted)', marginLeft: 18, marginTop: 2, fontFamily: 'monospace' }}>
                    {status.path}
                  </p>
                )}
                {installed && status?.auth_status === 'not_logged_in' && meta.login_hint && (
                  <p style={{ fontSize: 11.5, color: 'var(--warning)', marginLeft: 18, marginTop: 4, fontFamily: 'monospace' }}>
                    {meta.login_hint}
                  </p>
                )}
              </div>
              {!installed && (
                <button
                  onClick={() => handleInstall(meta.name)}
                  disabled={isInstalling}
                  style={{
                    padding: '7px 16px', borderRadius: 8, border: 'none',
                    fontSize: 13, fontWeight: 500, cursor: isInstalling ? 'wait' : 'pointer',
                    transition: 'all 0.15s', flexShrink: 0,
                    background: 'var(--accent)', color: 'white',
                    opacity: isInstalling ? 0.6 : 1
                  }}
                  className="hover:opacity-85"
                >
                  {isInstalling ? 'Installing…' : 'Install'}
                </button>
              )}
            </div>
          )
        })}
      </div>
    </div>
  )
}

function SectionTitle({ children }: { children: React.ReactNode }) {
  return <h3 style={{ fontSize: 15, fontWeight: 600, color: 'var(--text-primary)', marginBottom: 6, letterSpacing: '-0.01em' }}>{children}</h3>
}

function SectionDesc({ children }: { children: React.ReactNode }) {
  return <p style={{ fontSize: 13, color: 'var(--text-muted)', marginBottom: 20, lineHeight: 1.5 }}>{children}</p>
}
