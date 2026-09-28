import { useEffect, useState } from 'react'
import { api, KINDS, type EntryFilter, type EntrySummary, type Kind } from '../api/client'
import { useT } from '../i18n'
import { AgentBadge, Empty, KindBadge, Snippet, fmtDate, useAsync, useDebounced, useToast } from '../components/common'
import { EntryDetail } from '../components/EntryDetail'

export interface MemoriesProps {
  selectedId: string | null
  onSelect: (id: string | null) => void
  presetKind?: Kind
}

export function Memories({ selectedId, onSelect, presetKind }: MemoriesProps) {
  const { t } = useT()
  const toast = useToast()
  const [agent, setAgent] = useState<string>('')
  const [project, setProject] = useState<string>('')
  const [kind, setKind] = useState<Kind | ''>(presetKind ?? '')
  const [q, setQ] = useState('')
  const [archived, setArchived] = useState(false)
  const [noteOpen, setNoteOpen] = useState(false)
  const dq = useDebounced(q, 250)

  const tree = useAsync(() => api.tree())
  const filter: EntryFilter = { agent: agent || undefined, project: project || undefined, kind: kind || undefined, include_archived: archived, limit: 300 }
  const list = useAsync(() => (dq.trim() ? api.search(dq, filter) : api.list(filter)), [agent, project, kind, dq, archived])

  useEffect(() => { if (presetKind) setKind(presetKind) }, [presetKind])

  const items: EntrySummary[] = list.data ?? []
  const refreshAll = () => { tree.reload(); list.reload() }

  return (
    <div className="split3">
      {/* tree */}
      <div className="pane">
        <div className="pane-head row" style={{ justifyContent: 'space-between' }}>
          <strong>{t.nav.memories}</strong>
          <button className="btn sm" onClick={() => setNoteOpen(true)}>＋ {t.memories.newNote}</button>
        </div>
        <div className={'tree-item' + (!agent ? ' active' : '')} onClick={() => { setAgent(''); setProject('') }}>
          <span>{t.memories.allAgents}</span>
          <span className="count">{tree.data?.reduce((a, n) => a + n.count, 0) ?? ''}</span>
        </div>
        {tree.data?.map((n) => (
          <div key={n.agent}>
            <div className={'tree-item' + (agent === n.agent && !project ? ' active' : '')} onClick={() => { setAgent(n.agent); setProject('') }}>
              <span className="row"><span style={{ width: 8, height: 8, borderRadius: 4, background: `var(--accent)`, display: 'inline-block' }} />{n.agent}</span>
              <span className="count">{n.count}</span>
            </div>
            {agent === n.agent && n.projects.map((p) => (
              <div key={p.project} className={'tree-item child' + (project === p.project ? ' active' : '')} onClick={() => setProject(p.project)} title={p.project_path ?? ''}>
                <span className="ellipsis">{p.project}</span><span className="count">{p.count}</span>
              </div>
            ))}
          </div>
        ))}
      </div>

      {/* list */}
      <div className="pane">
        <div className="pane-head">
          <input className="input" placeholder={t.common.search} value={q} onChange={(e) => setQ(e.target.value)} />
          <div className="chips" style={{ marginTop: 8 }}>
            <button className={'chip' + (!kind ? ' on' : '')} onClick={() => setKind('')}>{t.common.all}</button>
            {KINDS.map((k) => (
              <button key={k} className={'chip' + (kind === k ? ' on' : '')} onClick={() => setKind(kind === k ? '' : k)}>{t.kinds[k]}</button>
            ))}
          </div>
          <div className="row small muted" style={{ marginTop: 8, justifyContent: 'space-between' }}>
            <span>{t.memories.entriesCount(items.length)}</span>
            <label className="row" style={{ cursor: 'pointer' }}><input type="checkbox" checked={archived} onChange={(e) => setArchived(e.target.checked)} />{t.memories.showArchived}</label>
          </div>
        </div>
        {list.loading && !list.data && <div className="empty">{t.common.loading}</div>}
        {list.error && <div className="empty" style={{ color: 'var(--danger)' }}>{list.error}</div>}
        {items.length === 0 && !list.loading && <Empty>{t.memories.noResults}</Empty>}
        {items.map((e) => (
          <div key={e.id} className={'list-item' + (selectedId === e.id ? ' active' : '')} onClick={() => onSelect(e.id)}>
            <div className="t ellipsis">{e.archived ? '⌫ ' : ''}{e.title}</div>
            <div className="m">
              <AgentBadge agent={e.agent} />
              <span>{e.project}</span>
              <KindBadge kind={e.kind} />
              <span>{fmtDate(e.updated)}</span>
            </div>
            {e.snippet && <div className="s"><Snippet text={e.snippet} /></div>}
          </div>
        ))}
      </div>

      {/* detail */}
      <div className="pane">
        {selectedId ? (
          <EntryDetail id={selectedId} onChanged={refreshAll} onOpenEntry={(id) => onSelect(id)} />
        ) : (
          <Empty>{t.memories.select}</Empty>
        )}
      </div>

      {noteOpen && (
        <NoteDialog
          onClose={() => setNoteOpen(false)}
          onCreated={(id) => { setNoteOpen(false); refreshAll(); onSelect(id); toast(t.settings.saved) }}
        />
      )}
    </div>
  )
}

function NoteDialog({ onClose, onCreated }: { onClose: () => void; onCreated: (id: string) => void }) {
  const { t } = useT()
  const toast = useToast()
  const [agent, setAgent] = useState('me')
  const [title, setTitle] = useState('')
  const [content, setContent] = useState('')
  const [tags, setTags] = useState('')
  const create = async () => {
    try {
      const e = await api.createNote({ agent, title, content, tags: tags.split(',').map((s) => s.trim()).filter(Boolean) })
      onCreated(e.id)
    } catch (err) { toast(String((err as Error).message), true) }
  }
  return (
    <div style={{ position: 'fixed', inset: 0, background: 'rgba(0,0,0,.35)', display: 'grid', placeItems: 'center', zIndex: 40 }} onClick={onClose}>
      <div className="card" style={{ width: 560, maxWidth: '92vw' }} onClick={(e) => e.stopPropagation()}>
        <h2>{t.memories.newNote}</h2>
        <label className="field"><span>{t.memories.noteAgent}</span><input className="input" value={agent} onChange={(e) => setAgent(e.target.value)} /></label>
        <label className="field"><span>{t.memories.noteTitle}</span><input className="input" value={title} onChange={(e) => setTitle(e.target.value)} /></label>
        <label className="field"><span>{t.memories.tags}</span><input className="input" value={tags} onChange={(e) => setTags(e.target.value)} /></label>
        <label className="field"><span>{t.memories.noteContent}</span><textarea className="input" style={{ minHeight: 180 }} value={content} onChange={(e) => setContent(e.target.value)} /></label>
        <div className="row" style={{ justifyContent: 'flex-end' }}>
          <button className="btn" onClick={onClose}>{t.common.cancel}</button>
          <button className="btn primary" onClick={create} disabled={!content.trim()}>{t.common.create}</button>
        </div>
      </div>
    </div>
  )
}
