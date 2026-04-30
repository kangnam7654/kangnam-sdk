/**
 * Inline `<question-form>` renderer for the chat pane.
 *
 * Adapted from open-design `src/components/QuestionForm.tsx`
 * (Apache-2.0). Differences from the upstream component:
 *
 * - i18n stripped — kangnam-sdk uses Korean strings inline.
 * - `direction-cards` falls back to a plain radio list for v1; Phase
 *   5c will plug in the swatch/font/mood card UI.
 * - Styling moved to inline CSS so the component drops into the
 *   chat pane without a stylesheet dependency.
 *
 * Submission flow:
 *   - User picks answers
 *   - Submit fires both:
 *     1. `cli.questionFormResponse(formId, answers)` → resolves the
 *        parked agent task on the backend (Phase 4c)
 *     2. `cli.sendMessage(sessionId, formattedAnswers)` → adds a
 *        user message to the chat so the agent's next turn includes
 *        the answers in context
 */
import { useMemo, useState } from 'react'

import { cliApi } from '../../lib/cli-api'
import {
  formatFormAnswers,
  type QuestionForm,
} from '../../lib/artifacts/question-form'
import { useAppStore } from '../../stores/app-store'
import { DirectionCardView } from './DirectionCardView'

interface Props {
  form: QuestionForm
  /** When false, the form renders in a locked "answered" state. */
  interactive?: boolean
  /** Pre-existing answers (for re-rendering an already-submitted form). */
  submittedAnswers?: Record<string, string | string[]>
}

export function QuestionFormView({
  form,
  interactive = true,
  submittedAnswers,
}: Props) {
  const initial = useMemo(() => buildInitialState(form, submittedAnswers), [form, submittedAnswers])
  const [answers, setAnswers] = useState<Record<string, string | string[]>>(initial)
  const [submitting, setSubmitting] = useState(false)
  const locked = !interactive || submittedAnswers !== undefined

  function update(id: string, value: string | string[]) {
    if (locked) return
    setAnswers((prev) => ({ ...prev, [id]: value }))
  }

  function toggleCheckbox(id: string, option: string, maxSelections?: number) {
    if (locked) return
    setAnswers((prev) => {
      const current = Array.isArray(prev[id]) ? (prev[id] as string[]) : []
      const has = current.includes(option)
      if (!has && maxSelections !== undefined && current.length >= maxSelections) {
        return prev
      }
      const next = has ? current.filter((v) => v !== option) : [...current, option]
      return { ...prev, [id]: next }
    })
  }

  const required = form.questions.filter((q) => q.required)
  const withinSelectionLimits = form.questions.every((q) => {
    if (q.type !== 'checkbox' || q.maxSelections === undefined) return true
    const v = answers[q.id]
    return !Array.isArray(v) || v.length <= q.maxSelections
  })
  const ready =
    withinSelectionLimits &&
    required.every((q) => {
      const v = answers[q.id]
      return Array.isArray(v) ? v.length > 0 : typeof v === 'string' && v.trim().length > 0
    })

  async function handleSubmit() {
    if (locked || !ready || submitting) return
    setSubmitting(true)
    const text = formatFormAnswers(form, answers)
    const sessionId = useAppStore.getState().currentSessionId
    try {
      // Resolve the parked agent task first; the chat-server's
      // `cli.questionFormResponse` will fire the oneshot::Sender so
      // the design `ask` tool returns and the model continues.
      await cliApi.questionFormResponse(form.id, { answers })
      // Then post the user-readable message into the chat so the
      // model's next turn sees the answers in conversational form.
      if (sessionId) {
        await cliApi.sendMessage(sessionId, text)
      }
    } catch (e) {
      console.error('[QuestionFormView] submit failed:', e)
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div
      style={{
        border: '1px solid var(--border)',
        borderRadius: 'var(--radius-lg)',
        padding: '14px 16px',
        margin: '8px 0',
        background: 'var(--bg-surface)',
        opacity: locked ? 0.7 : 1,
      }}
    >
      <div style={{ display: 'flex', alignItems: 'baseline', gap: 8, marginBottom: 8 }}>
        <span style={{ fontSize: 14, fontWeight: 600, color: 'var(--text-primary)' }}>
          {form.title}
        </span>
        {locked ? (
          <span
            style={{
              fontSize: 10,
              padding: '2px 6px',
              borderRadius: 99,
              background: 'var(--accent-soft)',
              color: 'var(--accent)',
              textTransform: 'uppercase',
              letterSpacing: '0.06em',
            }}
          >
            답변됨
          </span>
        ) : null}
      </div>
      {form.description ? (
        <div style={{ fontSize: 12, color: 'var(--text-muted)', marginBottom: 12 }}>
          {form.description}
        </div>
      ) : null}

      <div style={{ display: 'flex', flexDirection: 'column', gap: 14 }}>
        {form.questions.map((q) => (
          <FieldView
            key={q.id}
            q={q}
            value={answers[q.id]}
            locked={locked}
            formId={form.id}
            onUpdate={update}
            onToggleCheckbox={toggleCheckbox}
          />
        ))}
      </div>

      <div
        style={{
          marginTop: 14,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: 12,
        }}
      >
        {locked ? (
          <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>이전 답변 표시 중</span>
        ) : (
          <span style={{ fontSize: 11, color: 'var(--text-muted)' }}>
            * 표시는 필수 답변 항목입니다.
          </span>
        )}
        {!locked ? (
          <button
            type="button"
            onClick={handleSubmit}
            disabled={!ready || submitting}
            style={{
              border: 'none',
              borderRadius: 'var(--radius-md)',
              padding: '7px 14px',
              fontSize: 13,
              fontWeight: 500,
              background: 'var(--accent)',
              color: '#fff',
              cursor: ready ? 'pointer' : 'not-allowed',
              opacity: ready && !submitting ? 1 : 0.5,
              transition: 'opacity 0.15s',
            }}
          >
            {submitting ? '전송 중…' : (form.submitLabel ?? '답변 보내기')}
          </button>
        ) : null}
      </div>
    </div>
  )
}

interface FieldProps {
  q: import('../../lib/artifacts/question-form').FormQuestion
  value: string | string[] | undefined
  locked: boolean
  formId: string
  onUpdate: (id: string, value: string | string[]) => void
  onToggleCheckbox: (id: string, option: string, maxSelections?: number) => void
}

function FieldView({ q, value, locked, formId, onUpdate, onToggleCheckbox }: FieldProps) {
  return (
    <div>
      <label
        style={{
          display: 'block',
          fontSize: 12,
          fontWeight: 500,
          color: 'var(--text-primary)',
          marginBottom: 6,
        }}
      >
        <span>{q.label}</span>
        {q.required ? (
          <span style={{ color: 'var(--danger)', marginLeft: 4 }} aria-label="필수">
            *
          </span>
        ) : null}
      </label>
      {q.help ? (
        <div style={{ fontSize: 11, color: 'var(--text-muted)', marginBottom: 6 }}>{q.help}</div>
      ) : null}

      {q.type === 'radio' && q.options ? (
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
          {q.options.map((opt) => (
            <Chip key={opt} on={value === opt} disabled={locked} onClick={() => onUpdate(q.id, opt)}>
              {opt}
            </Chip>
          ))}
        </div>
      ) : null}

      {q.type === 'checkbox' && q.options ? (
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
          {q.options.map((opt) => {
            const arr = Array.isArray(value) ? value : []
            const on = arr.includes(opt)
            const maxed =
              q.maxSelections !== undefined && !on && arr.length >= q.maxSelections
            return (
              <Chip
                key={opt}
                on={on}
                disabled={locked || maxed}
                onClick={() => onToggleCheckbox(q.id, opt, q.maxSelections)}
              >
                {opt}
              </Chip>
            )
          })}
        </div>
      ) : null}

      {q.type === 'select' && q.options ? (
        <select
          disabled={locked}
          value={typeof value === 'string' ? value : ''}
          onChange={(e) => onUpdate(q.id, e.target.value)}
          style={{
            width: '100%',
            padding: '7px 10px',
            borderRadius: 'var(--radius-md)',
            border: '1px solid var(--border)',
            background: 'var(--bg-elevated)',
            color: 'var(--text-primary)',
            fontSize: 13,
          }}
        >
          <option value="" disabled>
            선택하세요
          </option>
          {q.options.map((opt) => (
            <option key={opt} value={opt}>
              {opt}
            </option>
          ))}
        </select>
      ) : null}

      {q.type === 'text' ? (
        <input
          type="text"
          value={typeof value === 'string' ? value : ''}
          placeholder={q.placeholder}
          disabled={locked}
          onChange={(e) => onUpdate(q.id, e.target.value)}
          style={{
            width: '100%',
            padding: '7px 10px',
            borderRadius: 'var(--radius-md)',
            border: '1px solid var(--border)',
            background: 'var(--bg-elevated)',
            color: 'var(--text-primary)',
            fontSize: 13,
          }}
        />
      ) : null}

      {q.type === 'textarea' ? (
        <textarea
          rows={3}
          value={typeof value === 'string' ? value : ''}
          placeholder={q.placeholder}
          disabled={locked}
          onChange={(e) => onUpdate(q.id, e.target.value)}
          style={{
            width: '100%',
            padding: '7px 10px',
            borderRadius: 'var(--radius-md)',
            border: '1px solid var(--border)',
            background: 'var(--bg-elevated)',
            color: 'var(--text-primary)',
            fontSize: 13,
            resize: 'vertical',
            fontFamily: 'inherit',
          }}
        />
      ) : null}

      {q.type === 'direction-cards' && q.cards && q.cards.length > 0 ? (
        <DirectionCardView
          cards={q.cards}
          value={typeof value === 'string' ? value : undefined}
          onPick={(id) => onUpdate(q.id, id)}
          disabled={locked}
        />
      ) : q.type === 'direction-cards' && q.options ? (
        // Fallback when the form supplies plain `options[]` instead of
        // rich `cards[]` — e.g. legacy skills that haven't been
        // upgraded yet. Renders chip radios as in 5b.
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
          {q.options.map((opt) => (
            <Chip
              key={opt}
              on={value === opt}
              disabled={locked}
              onClick={() => onUpdate(q.id, opt)}
            >
              {opt}
            </Chip>
          ))}
        </div>
      ) : null}

      {/* unused: formId — kept in props for parity with the upstream component */}
      <span hidden>{formId}</span>
    </div>
  )
}

function Chip({
  children,
  on,
  disabled,
  onClick,
}: {
  children: React.ReactNode
  on: boolean
  disabled: boolean
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      style={{
        padding: '6px 10px',
        borderRadius: 999,
        border: '1px solid var(--border)',
        fontSize: 12,
        cursor: disabled ? 'not-allowed' : 'pointer',
        background: on ? 'var(--accent)' : 'transparent',
        color: on ? '#fff' : 'var(--text-secondary)',
        opacity: disabled && !on ? 0.4 : 1,
        transition: 'background 0.15s, color 0.15s, opacity 0.15s',
      }}
    >
      {children}
    </button>
  )
}

function buildInitialState(
  form: QuestionForm,
  submitted: Record<string, string | string[]> | undefined,
): Record<string, string | string[]> {
  const out: Record<string, string | string[]> = {}
  for (const q of form.questions) {
    if (submitted && submitted[q.id] !== undefined) {
      out[q.id] = submitted[q.id]!
      continue
    }
    if (q.defaultValue !== undefined) {
      out[q.id] = q.defaultValue
      continue
    }
    if (q.type === 'checkbox') {
      out[q.id] = []
    } else {
      out[q.id] = ''
    }
  }
  return out
}
