/**
 * FileTree — collapsible directory tree for the design-mode
 * FileWorkspace. Adapted from open-design `components/FileWorkspace.tsx`
 * (Apache-2.0) but trimmed to the rendering essentials; the data layer
 * is supplied by the parent (FileWorkspace).
 *
 * Receives a flat list of `ProjectFileEntry` rows from the
 * `project_files_list` Tauri command (5c-07) and folds them into a
 * tree at render time so the parent can swap data without coordinating
 * a tree-building step.
 */
import { useMemo, useState } from 'react'

export interface ProjectFileEntry {
  path: string
  kind: 'dir' | 'file'
  size?: number | null
}

interface TreeNode {
  name: string
  path: string
  kind: 'dir' | 'file'
  size?: number | null
  children: TreeNode[]
}

function buildTree(entries: ProjectFileEntry[]): TreeNode {
  const root: TreeNode = { name: '', path: '', kind: 'dir', children: [] }
  // Map path → node so children can locate parents in O(1).
  const lookup: Map<string, TreeNode> = new Map([['', root]])
  for (const e of entries) {
    const parts = e.path.split('/')
    const name = parts[parts.length - 1] ?? e.path
    const parentPath = parts.slice(0, -1).join('/')
    const parent = lookup.get(parentPath) ?? root
    const node: TreeNode = {
      name,
      path: e.path,
      kind: e.kind,
      size: e.size,
      children: [],
    }
    parent.children.push(node)
    if (e.kind === 'dir') lookup.set(e.path, node)
  }
  return root
}

interface Props {
  entries: ProjectFileEntry[]
  selectedPath?: string | null
  onSelect: (path: string) => void
}

export function FileTree({ entries, selectedPath, onSelect }: Props) {
  const root = useMemo(() => buildTree(entries), [entries])
  return (
    <div style={{ padding: 4, fontSize: 12 }}>
      {root.children.map((c) => (
        <TreeNodeView
          key={c.path}
          node={c}
          depth={0}
          selectedPath={selectedPath}
          onSelect={onSelect}
        />
      ))}
      {root.children.length === 0 ? (
        <div style={{ color: 'var(--text-muted)', padding: 8, fontSize: 11 }}>
          파일이 없습니다.
        </div>
      ) : null}
    </div>
  )
}

interface NodeProps {
  node: TreeNode
  depth: number
  selectedPath: string | null | undefined
  onSelect: (path: string) => void
}

function TreeNodeView({ node, depth, selectedPath, onSelect }: NodeProps) {
  const [open, setOpen] = useState(depth < 1)
  const isSelected = selectedPath === node.path
  const indent = depth * 12

  if (node.kind === 'dir') {
    return (
      <div>
        <button
          type="button"
          onClick={() => setOpen((v) => !v)}
          style={{
            ...rowStyle,
            paddingLeft: 6 + indent,
            color: 'var(--text-secondary)',
            fontWeight: 500,
          }}
        >
          <span style={{ width: 12, display: 'inline-block', textAlign: 'center' }}>
            {open ? '▾' : '▸'}
          </span>
          <span style={{ marginLeft: 4 }}>{node.name}</span>
        </button>
        {open ? (
          <div>
            {node.children.map((c) => (
              <TreeNodeView
                key={c.path}
                node={c}
                depth={depth + 1}
                selectedPath={selectedPath}
                onSelect={onSelect}
              />
            ))}
          </div>
        ) : null}
      </div>
    )
  }

  return (
    <button
      type="button"
      onClick={() => onSelect(node.path)}
      style={{
        ...rowStyle,
        paddingLeft: 22 + indent,
        background: isSelected ? 'var(--bg-active)' : 'transparent',
        color: isSelected ? 'var(--text-primary)' : 'var(--text-secondary)',
      }}
    >
      <span>{node.name}</span>
    </button>
  )
}

const rowStyle: React.CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  width: '100%',
  textAlign: 'left',
  border: 'none',
  background: 'transparent',
  cursor: 'pointer',
  padding: '3px 6px',
  borderRadius: 3,
  fontSize: 12,
  fontFamily: 'inherit',
}
