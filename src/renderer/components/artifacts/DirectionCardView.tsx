/**
 * Rich card UI for `direction-cards` questions — palette swatches,
 * font samples, mood blurb, and reference chips. Replaces the plain
 * radio fallback that 5b shipped.
 *
 * Adapted from open-design `src/components/QuestionForm.tsx`
 * (Apache-2.0) `DirectionCardView` (lines ~180-280).
 */
import type { DirectionCard } from '../../lib/artifacts/question-form'

interface Props {
  cards: DirectionCard[]
  /** Selected card id, or undefined for "no choice yet". */
  value: string | undefined
  onPick: (cardId: string) => void
  disabled?: boolean
}

export function DirectionCardView({ cards, value, onPick, disabled }: Props) {
  return (
    <div
      style={{
        display: 'grid',
        gridTemplateColumns: 'repeat(auto-fill, minmax(220px, 1fr))',
        gap: 10,
      }}
    >
      {cards.map((card) => {
        const on = value === card.id
        return (
          <button
            key={card.id}
            type="button"
            onClick={() => !disabled && onPick(card.id)}
            disabled={disabled}
            style={{
              textAlign: 'left',
              padding: 12,
              borderRadius: 'var(--radius-lg)',
              border: on ? '2px solid var(--accent)' : '1px solid var(--border)',
              background: on ? 'var(--accent-soft)' : 'var(--bg-elevated)',
              cursor: disabled ? 'not-allowed' : 'pointer',
              opacity: disabled && !on ? 0.5 : 1,
              transition: 'border-color 0.15s, background 0.15s',
              display: 'flex',
              flexDirection: 'column',
              gap: 8,
            }}
          >
            <div
              style={{
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'space-between',
              }}
            >
              <span
                style={{ fontWeight: 600, fontSize: 13, color: 'var(--text-primary)' }}
              >
                {card.label}
              </span>
              {on ? (
                <span
                  style={{
                    fontSize: 9,
                    padding: '2px 6px',
                    borderRadius: 99,
                    background: 'var(--accent)',
                    color: '#fff',
                    textTransform: 'uppercase',
                    letterSpacing: '0.06em',
                  }}
                >
                  선택됨
                </span>
              ) : null}
            </div>

            {/* Palette swatches */}
            <div style={{ display: 'flex', gap: 4 }}>
              {card.palette.map((c, i) => (
                <span
                  key={`${card.id}-c-${i}`}
                  title={c}
                  style={{
                    width: 24,
                    height: 24,
                    borderRadius: 4,
                    background: c,
                    border: '1px solid rgba(0,0,0,0.1)',
                    flexShrink: 0,
                  }}
                />
              ))}
            </div>

            {/* Font samples */}
            <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
              <span
                style={{
                  fontFamily: card.displayFont,
                  fontSize: 18,
                  lineHeight: 1.1,
                  color: 'var(--text-primary)',
                  letterSpacing: '-0.01em',
                }}
              >
                Display
              </span>
              <span
                style={{
                  fontFamily: card.bodyFont,
                  fontSize: 12,
                  color: 'var(--text-secondary)',
                  lineHeight: 1.4,
                }}
              >
                Body type sample — {card.bodyFont}
              </span>
            </div>

            {/* Mood blurb */}
            <div style={{ fontSize: 11, color: 'var(--text-muted)', lineHeight: 1.4 }}>
              {card.mood}
            </div>

            {/* Reference chips */}
            {card.references.length > 0 ? (
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 4 }}>
                {card.references.map((r) => (
                  <span
                    key={`${card.id}-r-${r}`}
                    style={{
                      fontSize: 10,
                      padding: '2px 6px',
                      borderRadius: 99,
                      background: 'var(--bg-surface)',
                      color: 'var(--text-muted)',
                      border: '1px solid var(--border)',
                    }}
                  >
                    {r}
                  </span>
                ))}
              </div>
            ) : null}
          </button>
        )
      })}
    </div>
  )
}
