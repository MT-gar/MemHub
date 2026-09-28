import { useEffect, useState } from 'react'
import { api, type EntryFull } from '../api/client'
import { useT } from '../i18n'
import { AgentBadge, CopyButton, KindBadge, Markdown, fmtDate, useAsync, useToast } from './common'

export function EntryDetail({ id, onChanged, onOpenEntry }: { id: string; onChanged?: () => void; onOpenEntry?: (id: string) => void }) {
  const { t } = useT()
  const toast = useToast()
  const { data, error, loading, reload, setData } = useAsync(() => api.entry(id), [id])
  const [editing, setEditing] = useState(false)
  const [body, setBody] = useState('')
  const [title, setTitle] = useState('')
  const [tags, setTags] = useState('')

  useEffect(() => { setEditing(false) }, [id])

  if (loading && !data) return <div className="detail muted">{t.common.loading}</div>
  if (error) return <div className="detail" style={{ color: 'var(--danger)' }}>{error}</div>
  if (!data) return null
  const e: EntryFull = data

  const startEdit = () => { setBody(e.body); setTitle(e.title); setTags(e.tags.join(', ')); setEditing(true) }
  const save = async () => {
    try {
      await api.updateEntry(e.id, { body, title, tags: tags.split(',').map((s) => s.trim()).filter(Boolean) })
      setEditing(false)
      toast(t.settings.saved)
      reload()
      onChanged?.()
    } catch (err) { toast(String((err as Error).message), true) }
  }
  const del = async () => {
    if (!confirm(t.common.confirmDelete)) return
    try {
      await api.deleteEntry(e.id)
      setData(null)
      onChanged?.()
    } catch (err) { toast(String((err as Error).message), true) }
  }

  return (
    <div className="detail">
      <div className="row" style={{ alignItems: 'flex-start' }}>
        <div className="grow">
          {editing ? (
            <input className="input" value={title} onChange={(ev) => setTitle(ev.target.value)} style={{ fontSize: 18, fontWeight: 700 }} />
          ) : (
            <h1 style={{ marginBottom: 6 }}>{e.title}</h1>
          )}
          <div className="row wrap small">
            <AgentBadge agent={e.agent} />
            <KindBadge kind={e.kind} />
            <span className="badge">{e.project}</span>
            {e.native ? <span className="badge ok">{t.memories.native}</span> : <span className="badge warn">{t.memories.mirrored}</span>}
            {e.frontmatter.redacted && <span className="badge err">{t.memories.redacted}</span>}
            {e.archived && <span className="badge">archived</span>}
          </div>
        </div>
        <div className="row">
          {editing ? (
            <>
              <button className="btn sm" onClick={() => setEditing(false)}>{t.common.cancel}</button>
              <button className="btn sm primary" onClick={save}>{t.common.save}</button>
            </>
          ) : (
            <>
              <CopyButton text={e.body} />
              <button className="btn sm" onClick={startEdit}>✎ {t.common.edit}</button>
              {e.native && <button className="btn sm danger" onClick={del}>{t.common.delete}</button>}
            </>
          )}
        </div>
      </div>

      <dl className="meta">
        <dt>ID</dt><dd className="mono">{e.id}</dd>
        <dt>{t.memories.vaultPath}</dt><dd className="mono">{e.vault_path}</dd>
        {e.origin && <><dt>{t.memories.origin}</dt><dd className="mono">{e.origin}</dd></>}
        {e.project_path && <><dt>project_path</dt><dd className="mono">{e.project_path}</dd></>}
        <dt>{t.common.updated}</dt><dd>{fmtDate(e.updated, true)}</dd>
        <dt>{t.common.created}</dt><dd>{fmtDate(e.created, true)}</dd>
        {e.tags.length > 0 && <><dt>tags</dt><dd>{e.tags.map((x) => <span key={x} className="badge" style={{ marginRight: 4 }}>{x}</span>)}</dd></>}
        {e.frontmatter.task && <><dt>{t.memories.relatedTask}</dt><dd className="mono">{e.frontmatter.task}</dd></>}
        {e.frontmatter.sources.length > 0 && (
          <><dt>{t.memories.sources}</dt>
            <dd className="row wrap">
              {e.frontmatter.sources.map((s) => (
                <button key={s} className="btn sm mono" onClick={() => onOpenEntry?.(s)}>{s.slice(-8)}</button>
              ))}
            </dd></>
        )}
      </dl>

      {editing ? (
        <>
          {!e.native && <div className="badge warn" style={{ marginBottom: 8 }}>{t.memories.editWarn}</div>}
          <label className="field"><span>{t.memories.tags}</span><input className="input" value={tags} onChange={(ev) => setTags(ev.target.value)} /></label>
          <label className="field"><span>{t.memories.body}</span>
            <textarea className="input" style={{ minHeight: 420 }} value={body} onChange={(ev) => setBody(ev.target.value)} />
          </label>
        </>
      ) : (
        <Markdown>{e.body}</Markdown>
      )}
    </div>
  )
}
