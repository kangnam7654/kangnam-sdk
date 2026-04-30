/**
 * NewProjectPanel — minimal v1 of the design-mode project creation flow.
 *
 * Inspired by open-design `src/components/NewProjectPanel.tsx` but
 * trimmed heavily: skill picker → optional design-system picker →
 * brief textarea → "Start project" button. The skill / DS lists are
 * fetched via `window.api.design.skills()` / `design.systems()`
 * (Phase 5b-14 lands those Tauri commands; until then this component
 * shows hard-coded fallbacks so the flow is exercisable).
 *
 * On submit: create a new Project with the chosen ids + brief in
 * `pendingPrompt`, set it active, and close the panel. App.tsx
 * (5b-15) sees the activeProjectId change, flips MainView to
 * 'project', and ProjectView's chat pane consumes the pendingPrompt.
 */
import { useEffect, useMemo, useState } from 'react'

import { useAppStore } from '../../stores/app-store'

interface CatalogEntry {
  id: string
  name: string
  description?: string
}

const FALLBACK_SKILLS: CatalogEntry[] = [
  { id: 'web-prototype', name: 'Web Prototype', description: '단일 페이지 HTML 프로토타입' },
  { id: 'saas-landing', name: 'SaaS Landing', description: '랜딩 페이지' },
  { id: 'simple-deck', name: 'Simple Deck', description: '발표용 슬라이드 덱' },
]

const FALLBACK_DESIGN_SYSTEMS: CatalogEntry[] = [
  { id: 'linear', name: 'Linear' },
  { id: 'stripe', name: 'Stripe' },
  { id: 'vercel', name: 'Vercel' },
  { id: 'notion', name: 'Notion' },
]

interface Props {
  onClose: () => void
}

export function NewProjectPanel({ onClose }: Props) {
  const upsertProject = useAppStore((s) => s.upsertProject)
  const setActiveProjectId = useAppStore((s) => s.setActiveProjectId)
  const setActiveMainView = useAppStore((s) => s.setActiveMainView)

  const [skills, setSkills] = useState<CatalogEntry[]>(FALLBACK_SKILLS)
  const [designSystems, setDesignSystems] = useState<CatalogEntry[]>(FALLBACK_DESIGN_SYSTEMS)
  const [name, setName] = useState('')
  const [skillId, setSkillId] = useState<string>(FALLBACK_SKILLS[0]!.id)
  const [designSystemId, setDesignSystemId] = useState<string | null>(null)
  const [brief, setBrief] = useState('')
  const [submitting, setSubmitting] = useState(false)

  // Fetch skill/DS catalogs from the host. Phase 5b-14 lands the
  // Tauri commands; until then `window.api.design` is undefined and
  // we keep the hardcoded fallbacks.
  useEffect(() => {
    const api = (window as unknown as {
      api?: { design?: { skills?: () => Promise<CatalogEntry[]>; systems?: () => Promise<CatalogEntry[]> } }
    }).api?.design
    if (!api) return
    let cancelled = false
    if (api.skills) {
      api
        .skills()
        .then((rows) => {
          if (!cancelled && rows.length > 0) setSkills(rows)
        })
        .catch((e) => console.warn('[NewProjectPanel] skills fetch failed:', e))
    }
    if (api.systems) {
      api
        .systems()
        .then((rows) => {
          if (!cancelled && rows.length > 0) setDesignSystems(rows)
        })
        .catch((e) => console.warn('[NewProjectPanel] systems fetch failed:', e))
    }
    return () => {
      cancelled = true
    }
  }, [])

  const ready = useMemo(
    () => name.trim().length > 0 && brief.trim().length > 0 && Boolean(skillId),
    [name, brief, skillId],
  )

  function handleSubmit() {
    if (!ready || submitting) return
    setSubmitting(true)
    const id = crypto.randomUUID()
    const now = Date.now()
    upsertProject({
      id,
      name: name.trim(),
      skillId,
      designSystemId,
      workingDir: null,
      conversationId: null,
      pendingPrompt: brief.trim(),
      createdAt: now,
      updatedAt: now,
    })
    setActiveProjectId(id)
    setActiveMainView('project')
    onClose()
  }

  return (
    <div
      style={{
        position: 'fixed',
        inset: 0,
        zIndex: 200,
        background: 'rgba(0,0,0,0.5)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
      }}
      onClick={onClose}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          width: 560,
          maxWidth: 'calc(100vw - 48px)',
          maxHeight: 'calc(100vh - 48px)',
          overflow: 'auto',
          background: 'var(--bg-surface)',
          borderRadius: 'var(--radius-lg)',
          padding: 24,
          boxShadow: '0 12px 48px rgba(0,0,0,0.4)',
          display: 'flex',
          flexDirection: 'column',
          gap: 16,
        }}
      >
        <div>
          <div style={{ fontSize: 18, fontWeight: 600, color: 'var(--text-primary)' }}>
            새 프로젝트
          </div>
          <div style={{ fontSize: 12, color: 'var(--text-muted)', marginTop: 4 }}>
            스킬 + 디자인 시스템을 골라 새 디자인 작업을 시작하세요.
          </div>
        </div>

        <Field label="이름" required>
          <input
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="예: 모바일 가입 플로우"
            style={inputStyle}
          />
        </Field>

        <Field label="스킬" required>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
            {skills.map((s) => (
              <Pill key={s.id} on={skillId === s.id} onClick={() => setSkillId(s.id)}>
                {s.name}
              </Pill>
            ))}
          </div>
        </Field>

        <Field label="디자인 시스템 (선택)">
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
            <Pill on={designSystemId === null} onClick={() => setDesignSystemId(null)}>
              없음
            </Pill>
            {designSystems.map((d) => (
              <Pill
                key={d.id}
                on={designSystemId === d.id}
                onClick={() => setDesignSystemId(d.id)}
              >
                {d.name}
              </Pill>
            ))}
          </div>
        </Field>

        <Field label="브리프" required>
          <textarea
            rows={5}
            value={brief}
            onChange={(e) => setBrief(e.target.value)}
            placeholder="만들고 싶은 디자인을 한 단락 정도로 설명해주세요. 사용자, 톤, 핵심 메시지 등을 포함하면 좋습니다."
            style={{ ...inputStyle, resize: 'vertical', fontFamily: 'inherit' }}
          />
        </Field>

        <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8 }}>
          <button onClick={onClose} style={ghostButtonStyle}>
            취소
          </button>
          <button
            onClick={handleSubmit}
            disabled={!ready || submitting}
            style={{
              ...primaryButtonStyle,
              opacity: ready && !submitting ? 1 : 0.5,
              cursor: ready && !submitting ? 'pointer' : 'not-allowed',
            }}
          >
            {submitting ? '시작 중…' : '프로젝트 시작'}
          </button>
        </div>
      </div>
    </div>
  )
}

function Field({
  label,
  required,
  children,
}: {
  label: string
  required?: boolean
  children: React.ReactNode
}) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
      <label
        style={{
          fontSize: 12,
          fontWeight: 500,
          color: 'var(--text-secondary)',
        }}
      >
        {label}
        {required ? <span style={{ color: 'var(--danger)', marginLeft: 4 }}>*</span> : null}
      </label>
      {children}
    </div>
  )
}

function Pill({
  children,
  on,
  onClick,
}: {
  children: React.ReactNode
  on: boolean
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      style={{
        padding: '6px 12px',
        borderRadius: 999,
        fontSize: 12,
        border: '1px solid var(--border)',
        background: on ? 'var(--accent)' : 'transparent',
        color: on ? '#fff' : 'var(--text-secondary)',
        cursor: 'pointer',
        transition: 'background 0.15s, color 0.15s',
      }}
    >
      {children}
    </button>
  )
}

const inputStyle: React.CSSProperties = {
  padding: '8px 12px',
  borderRadius: 'var(--radius-md)',
  border: '1px solid var(--border)',
  background: 'var(--bg-elevated)',
  color: 'var(--text-primary)',
  fontSize: 13,
  width: '100%',
}

const primaryButtonStyle: React.CSSProperties = {
  padding: '7px 14px',
  borderRadius: 'var(--radius-md)',
  border: 'none',
  fontSize: 13,
  fontWeight: 500,
  background: 'var(--accent)',
  color: '#fff',
}

const ghostButtonStyle: React.CSSProperties = {
  padding: '7px 14px',
  borderRadius: 'var(--radius-md)',
  border: '1px solid var(--border)',
  fontSize: 13,
  background: 'transparent',
  color: 'var(--text-secondary)',
  cursor: 'pointer',
}
