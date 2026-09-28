import { api } from '../api/client'
import { useT } from '../i18n'
import { Empty, fmtDate, useAsync } from '../components/common'
import { EntryDetail } from '../components/EntryDetail'

export function Knowledge({ selectedId, onSelect, go, openEntry }: { selectedId: string | null; onSelect: (id: string | null) => void; go: (p: string) => void; openEntry: (id: string) => void }) {
  const { t } = useT()
  const list = useAsync(() => api.list({ kind: 'knowledge', limit: 500 }))
  const items = list.data ?? []
  const groups = new Map<string, typeof items>()
  for (const e of items) {
    const g = e.vault_path.split('/')[1] ?? 'general'
    groups.set(g, [...(groups.get(g) ?? []), e])
  }
  return (
    <div className="split2">
      <div className="pane">
        <div className="pane-head row" style={{ justifyContent: 'space-between' }}>
          <strong>{t.knowledge.title}</strong>
          <span className="muted small">{items.length}</span>
        </div>
        {items.length === 0 && !list.loading && (
          <Empty>
            <p>{t.knowledge.empty}</p>
            <button className="btn primary" onClick={() => go('tasks')}>{t.knowledge.goTasks}</button>
          </Empty>
        )}
        {[...groups.entries()].map(([g, es]) => (
          <div key={g}>
            <div className="tree-item" style={{ cursor: 'default' }}><strong className="small muted" style={{ textTransform: 'uppercase' }}>{g}</strong></div>
            {es.map((e) => (
              <div key={e.id} className={'list-item' + (selectedId === e.id ? ' active' : '')} onClick={() => onSelect(e.id)}>
                <div className="t">{e.title}</div>
                <div className="m"><span>{fmtDate(e.updated)}</span>{e.tags.map((x) => <span key={x} className="badge">{x}</span>)}</div>
              </div>
            ))}
          </div>
        ))}
      </div>
      <div className="pane">
        {selectedId ? <EntryDetail id={selectedId} onChanged={list.reload} onOpenEntry={openEntry} /> : <Empty>{t.memories.select}</Empty>}
      </div>
    </div>
  )
}
