/**
 * Type shapes + answer-formatting helpers for inline `<question-form>`
 * blocks. Ported from open-design `src/artifacts/question-form.ts`
 * (Apache-2.0); the streaming-stripping helpers (splitOnQuestionForms,
 * parseSubmittedAnswers) aren't needed in v1 because the chat-server
 * already emits typed `question_form_posted` UnifiedMessage variants
 * (Phase 4c).
 */
export type QuestionType =
  | 'radio'
  | 'checkbox'
  | 'select'
  | 'text'
  | 'textarea'
  | 'direction-cards'

/**
 * Rich card metadata for a single `direction-cards` option (5b
 * scaffolds the type but the renderer treats them as plain radios
 * — Phase 5c will wire the swatch/font/mood UI).
 */
export interface DirectionCard {
  id: string
  label: string
  mood: string
  references: string[]
  palette: string[]
  displayFont: string
  bodyFont: string
}

export interface FormQuestion {
  id: string
  label: string
  type: QuestionType
  options?: string[]
  placeholder?: string
  required?: boolean
  help?: string
  defaultValue?: string | string[]
  /** Only applies when `type === 'checkbox'`. Caps selected count. */
  maxSelections?: number
  /** Only present when `type === 'direction-cards'`. */
  cards?: DirectionCard[]
}

export interface QuestionForm {
  id: string
  title: string
  description?: string
  questions: FormQuestion[]
  submitLabel?: string
}

/**
 * Format the user's answers into a markdown chunk that the agent can
 * read back as the user's reply. The shape (`[form answers — <id>]`
 * header + `- Label: value` lines) matches the open-design contract
 * so an upstream skill that expects this format works unchanged.
 */
export function formatFormAnswers(
  form: QuestionForm,
  answers: Record<string, string | string[]>,
): string {
  const lines: string[] = [`[form answers — ${form.id}]`]
  for (const q of form.questions) {
    const v = answers[q.id]
    let text: string
    if (Array.isArray(v)) {
      text = v.length > 0 ? v.join(', ') : '(skipped)'
    } else if (typeof v === 'string' && v.trim().length > 0) {
      text = v.trim()
    } else {
      text = '(skipped)'
    }
    lines.push(`- ${q.label}: ${text}`)
  }
  return lines.join('\n')
}
