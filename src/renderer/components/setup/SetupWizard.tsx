import { useEffect, useState } from 'react'
import { useAppStore } from '../../stores/app-store'
import { cliApi, type ProviderMeta } from '../../lib/cli-api'
import type { CliStatus } from '../../stores/app-store'

const PROVIDER_ICONS: Record<string, string> = {
  claude: 'C',
  codex: 'X',
  codex_cli: 'X',
}

export function SetupWizard() {
  const { setSetupComplete, setCurrentProvider } = useAppStore()
  const [providers, setProviders] = useState<ProviderMeta[]>([])
  const [statuses, setStatuses] = useState<Record<string, CliStatus>>({})
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [installing, setInstalling] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    setLoading(true)
    cliApi.listProviders()
      .then(async (metas) => {
        setProviders(metas)
        const statusMap: Record<string, CliStatus> = {}
        for (const meta of metas) {
          try {
            statusMap[meta.name] = await cliApi.checkInstalled(meta.name)
          } catch {
            statusMap[meta.name] = {
              provider: meta.name,
              installed: false,
              version: null,
              path: null,
              authenticated: false,
              auth_status: 'unknown',
              auth_source: meta.auth_source,
              auth_message: null,
            }
          }
        }
        setStatuses(statusMap)
        const installedSet = new Set<string>()
        for (const [name, status] of Object.entries(statusMap)) {
          if (status.installed) installedSet.add(name)
        }
        setSelected(installedSet)
      })
      .catch((e) => {
        setError('CLI 프로바이더를 불러올 수 없습니다. Tauri 앱에서 실행해주세요.')
        console.error('listProviders failed:', e)
      })
      .finally(() => setLoading(false))
  }, [])

  const handleInstall = async (provider: string) => {
    setInstalling(provider)
    setError(null)
    try {
      await cliApi.install(provider)
      const status = await cliApi.checkInstalled(provider)
      setStatuses((prev) => ({ ...prev, [provider]: status }))
      if (status.installed) {
        setSelected((prev) => new Set([...prev, provider]))
      }
    } catch (e) {
      setError(`${provider} 설치에 실패했습니다. 터미널에서 직접 설치해주세요.`)
      console.error('Install failed:', e)
    }
    setInstalling(null)
  }

  const handleToggle = (provider: string) => {
    setSelected((prev) => {
      const next = new Set(prev)
      if (next.has(provider)) next.delete(provider)
      else next.add(provider)
      return next
    })
  }

  const handleStart = () => {
    if (selected.size > 0) {
      const firstProvider = Array.from(selected)[0]
      setCurrentProvider(firstProvider)
      setSetupComplete(true)
    }
  }

  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        height: '100vh',
        width: '100vw',
        background: 'var(--bg-main)',
        fontFamily: "'Pretendard', -apple-system, system-ui, sans-serif",
      }}
    >
      <div style={{ width: '100%', maxWidth: 520, padding: '0 24px' }}>
        {/* Header */}
        <div style={{ textAlign: 'center', marginBottom: 40 }}>
          <div style={{
            width: 48, height: 48, borderRadius: 12,
            background: 'var(--accent)', display: 'inline-flex',
            alignItems: 'center', justifyContent: 'center',
            marginBottom: 20, fontSize: 22, fontWeight: 700, color: 'white',
          }}>K</div>
          <h1 style={{
            fontSize: 22, fontWeight: 700,
            color: 'var(--text-primary)', marginBottom: 8, letterSpacing: '-0.02em',
          }}>
            사용할 AI 도구를 선택하세요
          </h1>
          <p style={{ fontSize: 14, color: 'var(--text-muted)', lineHeight: 1.5 }}>
            설치되지 않은 도구는 설치를 도와드립니다
          </p>
        </div>

        {/* Error */}
        {error && (
          <div style={{
            padding: '12px 16px', borderRadius: 10, marginBottom: 20,
            background: 'rgba(239, 68, 68, 0.08)', border: '1px solid rgba(239, 68, 68, 0.2)',
            fontSize: 13, color: 'var(--danger-text)', lineHeight: 1.5,
          }}>
            {error}
          </div>
        )}

        {/* Loading */}
        {loading && (
          <div style={{ textAlign: 'center', padding: 40, color: 'var(--text-muted)', fontSize: 14 }}>
            CLI 상태 확인 중...
          </div>
        )}

        {/* Provider Cards */}
        {!loading && (
          <div style={{ display: 'flex', gap: 12, marginBottom: 24 }}>
            {providers.map((meta) => {
              const status = statuses[meta.name]
              const installed = status?.installed ?? false
              const isSelected = selected.has(meta.name)
              const isInstalling = installing === meta.name

              return (
                <button
                  key={meta.name}
                  onClick={installed ? () => handleToggle(meta.name) : () => handleInstall(meta.name)}
                  disabled={isInstalling}
                  style={{
                    flex: 1, padding: 20, borderRadius: 14,
                    border: isSelected
                      ? '2px solid var(--success)'
                      : '2px solid var(--border-light)',
                    background: isSelected
                      ? 'rgba(16, 185, 129, 0.06)'
                      : 'var(--bg-surface)',
                    cursor: isInstalling ? 'wait' : 'pointer',
                    opacity: isInstalling ? 0.6 : 1,
                    transition: 'all 0.2s ease',
                    textAlign: 'center',
                  }}
                >
                  {/* Icon */}
                  <div style={{
                    width: 44, height: 44, borderRadius: 11,
                    background: isSelected ? 'var(--success)' : 'var(--bg-hover)',
                    display: 'inline-flex', alignItems: 'center', justifyContent: 'center',
                    marginBottom: 12, fontSize: 18, fontWeight: 700,
                    color: isSelected ? 'white' : 'var(--text-secondary)',
                    transition: 'all 0.2s ease',
                  }}>
                    {PROVIDER_ICONS[meta.name] ?? meta.name[0].toUpperCase()}
                  </div>

                  {/* Name */}
                  <div style={{
                    fontSize: 15, fontWeight: 650,
                    color: 'var(--text-primary)', marginBottom: 4,
                  }}>
                    {meta.display_name}
                  </div>

                  {/* Description */}
                  <div style={{
                    fontSize: 12, color: 'var(--text-muted)',
                    marginBottom: 12, lineHeight: 1.4,
                  }}>
                    {meta.description}
                  </div>

                  {/* Status */}
                  {installed ? (
                    <div style={{
                      display: 'inline-flex', alignItems: 'center', gap: 6,
                      padding: '5px 12px', borderRadius: 20,
                      background: status?.auth_status === 'not_logged_in'
                        ? 'rgba(251, 191, 36, 0.12)'
                        : isSelected ? 'rgba(16, 185, 129, 0.15)' : 'rgba(255,255,255,0.05)',
                      fontSize: 12, fontWeight: 550,
                      color: status?.auth_status === 'not_logged_in'
                        ? 'var(--warning)'
                        : isSelected ? 'var(--success-text)' : 'var(--text-muted)',
                    }}>
                      {isSelected ? '\u2713 ' : ''}
                      {status?.auth_status === 'not_logged_in' ? '로그인 필요' : `v${status?.version ?? '?'}`}
                    </div>
                  ) : isInstalling ? (
                    <div style={{
                      display: 'inline-flex', alignItems: 'center', gap: 6,
                      padding: '5px 12px', borderRadius: 20,
                      background: 'rgba(251, 191, 36, 0.1)',
                      fontSize: 12, color: 'var(--warning)',
                    }}>
                      설치 중...
                    </div>
                  ) : (
                    <div style={{
                      display: 'inline-flex', alignItems: 'center', gap: 4,
                      padding: '5px 12px', borderRadius: 20,
                      background: 'var(--accent-soft)',
                      fontSize: 12, fontWeight: 550, color: 'var(--accent)',
                    }}>
                      설치하기
                    </div>
                  )}
                </button>
              )
            })}
          </div>
        )}

        {/* No providers fallback */}
        {!loading && providers.length === 0 && !error && (
          <div style={{
            textAlign: 'center', padding: '32px 20px', borderRadius: 14,
            border: '1px dashed var(--border-light)',
            color: 'var(--text-muted)', fontSize: 13, marginBottom: 24,
          }}>
            프로바이더 정보를 불러오지 못했습니다.
          </div>
        )}

        {/* Start button */}
        {!loading && (
          <div style={{ textAlign: 'center' }}>
            <button
              onClick={handleStart}
              disabled={selected.size === 0}
              style={{
                padding: '12px 40px', borderRadius: 10,
                background: selected.size > 0 ? 'var(--accent)' : 'var(--bg-hover)',
                color: selected.size > 0 ? 'white' : 'var(--text-muted)',
                fontSize: 15, fontWeight: 650, border: 'none',
                cursor: selected.size > 0 ? 'pointer' : 'not-allowed',
                transition: 'all 0.2s ease',
                letterSpacing: '-0.01em',
              }}
            >
              시작하기
            </button>
          </div>
        )}
      </div>
    </div>
  )
}
