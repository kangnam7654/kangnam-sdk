/**
 * FileWorkspace — split-pane: tree on the left, viewer on the right.
 *
 * Uses the active project's `workingDir` from the projects slice. When
 * no project is active or the project has no workingDir yet, shows a
 * gentle empty state.
 *
 * Lists are refreshed on:
 * - Initial mount
 * - When `workingDir` changes
 * - Manual "새로고침" button
 *
 * Phase 5c-10 v1 doesn't yet hook the chat-server's file-write
 * notifications — the model can write files via `project_file_write`
 * but the tree won't refresh until the user clicks the button. A later
 * commit can subscribe to `cli.fileWrote` events.
 */
import { useCallback, useEffect, useState } from 'react'

import { useAppStore } from '../../stores/app-store'
import { FileTree, type ProjectFileEntry } from './FileTree'
import { FileViewer } from './FileViewer'

interface ProjectApi {
  filesList: (workingDir: string) => Promise<ProjectFileEntry[]>
}

function getProjectApi(): ProjectApi | null {
  const api = (window as unknown as {
    api?: { project?: { filesList?: ProjectApi['filesList'] } }
  }).api?.project
  if (!api?.filesList) return null
  return { filesList: api.filesList }
}

export function FileWorkspace() {
  const project = useAppStore((s) =>
    s.activeProjectId ? s.projects[s.activeProjectId] : undefined,
  )
  const [entries, setEntries] = useState<ProjectFileEntry[]>([])
  const [selected, setSelected] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  const refresh = useCallback(() => {
    const wd = project?.workingDir
    if (!wd) {
      setEntries([])
      setError(null)
      return
    }
    const api = getProjectApi()
    if (!api) {
      setError('호스트가 file API를 제공하지 않습니다 (web-only 빌드).')
      return
    }
    setLoading(true)
    api
      .filesList(wd)
      .then((rows) => {
        setEntries(rows)
        setError(null)
      })
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false))
  }, [project?.workingDir])

  useEffect(() => {
    refresh()
  }, [refresh])

  if (!project) {
    return (
      <Empty>아직 활성 프로젝트가 없습니다. 새 프로젝트를 시작하면 파일 트리가 표시됩니다.</Empty>
    )
  }
  if (!project.workingDir) {
    return <Empty>이 프로젝트에 워킹 디렉터리가 설정되지 않았습니다.</Empty>
  }

  return (
    <div style={{ display: 'flex', height: '100%', minHeight: 0 }}>
      {/* Left: tree */}
      <div
        style={{
          width: 240,
          minWidth: 200,
          borderRight: '1px solid var(--border)',
          display: 'flex',
          flexDirection: 'column',
          overflow: 'hidden',
        }}
      >
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            padding: '6px 8px',
            borderBottom: '1px solid var(--border)',
            fontSize: 11,
            color: 'var(--text-muted)',
            flexShrink: 0,
          }}
        >
          <span>Files</span>
          <button
            type="button"
            onClick={refresh}
            disabled={loading}
            style={{
              fontSize: 10,
              padding: '2px 6px',
              border: '1px solid var(--border)',
              borderRadius: 4,
              background: 'transparent',
              color: 'var(--text-secondary)',
              cursor: 'pointer',
              opacity: loading ? 0.5 : 1,
            }}
          >
            새로고침
          </button>
        </div>
        <div style={{ flex: 1, overflow: 'auto' }}>
          {error ? (
            <div
              style={{
                padding: 8,
                fontSize: 11,
                color: 'var(--danger)',
              }}
            >
              {error}
            </div>
          ) : (
            <FileTree
              entries={entries}
              selectedPath={selected}
              onSelect={setSelected}
            />
          )}
        </div>
      </div>

      {/* Right: viewer */}
      <div style={{ flex: 1, minWidth: 0, padding: 8 }}>
        {selected ? (
          <FileViewer key={selected} workingDir={project.workingDir} path={selected} />
        ) : (
          <Empty>왼쪽에서 파일을 선택해 미리보기 / 편집하세요.</Empty>
        )}
      </div>
    </div>
  )
}

function Empty({ children }: { children: React.ReactNode }) {
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        height: '100%',
        color: 'var(--text-muted)',
        fontSize: 12,
        padding: 16,
        textAlign: 'center',
      }}
    >
      {children}
    </div>
  )
}
