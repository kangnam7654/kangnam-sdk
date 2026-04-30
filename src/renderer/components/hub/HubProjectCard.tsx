/**
 * HubProjectCard — single project tile in the Designs Hub grid.
 *
 * Shows project name + skill + DS labels + a sandbox preview iframe
 * sized down. The iframe is rendered with `sandbox="allow-scripts"`
 * and a static placeholder body (no live agent stream); the Hub's
 * job is to give the user a visual switcher across recent projects.
 *
 * Inspired by open-codesign `views/HubView.tsx` HubProjectCard.
 */
import type { Project } from '../../stores/slices/projects'

interface Props {
  project: Project
  active: boolean
  onOpen: () => void
}

const PLACEHOLDER_BODY = `<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <style>
    html, body { margin: 0; padding: 0; height: 100%; font-family: ui-sans-serif, system-ui; }
    body {
      display: grid; place-items: center;
      background: linear-gradient(135deg, #f5f7fa, #c3cfe2);
      color: #2d3748;
      font-size: 13px;
    }
    .name { font-weight: 600; font-size: 18px; margin-bottom: 4px; }
    .meta { font-size: 11px; color: #4a5568; }
  </style>
</head>
<body>
  <div style="text-align:center">
    <div class="name">__NAME__</div>
    <div class="meta">__META__</div>
  </div>
</body>
</html>
`

function buildPlaceholder(p: Project): string {
  const meta = [p.skillId, p.designSystemId].filter(Boolean).join(' · ') || 'design project'
  return PLACEHOLDER_BODY.replace('__NAME__', escapeHtml(p.name)).replace(
    '__META__',
    escapeHtml(meta),
  )
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

export function HubProjectCard({ project, active, onOpen }: Props) {
  const meta = [project.skillId, project.designSystemId].filter(Boolean).join(' · ')
  return (
    <button
      type="button"
      onClick={onOpen}
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 0,
        textAlign: 'left',
        border: active ? '2px solid var(--accent)' : '1px solid var(--border)',
        borderRadius: 'var(--radius-md)',
        background: 'var(--bg-elevated)',
        cursor: 'pointer',
        overflow: 'hidden',
        transition: 'border-color 0.15s, transform 0.15s',
        height: 220,
      }}
    >
      <div
        style={{
          flex: 1,
          minHeight: 0,
          background: 'var(--bg-surface)',
          position: 'relative',
        }}
      >
        <iframe
          srcDoc={buildPlaceholder(project)}
          sandbox="allow-scripts"
          style={{
            position: 'absolute',
            inset: 0,
            width: '100%',
            height: '100%',
            border: 'none',
            pointerEvents: 'none',
          }}
          title={`preview · ${project.name}`}
        />
      </div>
      <div
        style={{
          padding: '8px 10px',
          borderTop: '1px solid var(--border)',
          flexShrink: 0,
        }}
      >
        <div
          style={{ fontSize: 13, fontWeight: 600, color: 'var(--text-primary)' }}
        >
          {project.name}
        </div>
        {meta ? (
          <div style={{ fontSize: 11, color: 'var(--text-muted)', marginTop: 2 }}>
            {meta}
          </div>
        ) : null}
      </div>
    </button>
  )
}
