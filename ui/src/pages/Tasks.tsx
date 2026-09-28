import { useEffect, useMemo, useState } from 'react'
import { api, KINDS, type TaskDetail, type TaskMeta, type TaskScope } from '../api/client'
import { useT } from '../i18n'
import { AgentBadge, CopyBox, CopyButton, Empty, KindBadge, Markdown, Tabs, fmtBytes, fmtDate, useAsync, useToast } from '../components/common'

export function Tasks({ selectedId, onSelect, openEntry }: { selectedId: string | null; onSelect: (id: string | null) => void; openEntry: (id: string) => void }) {
  const { t } = useT()
  const tasks = useAsync(() => api.tasks())
  const items = tasks.data ?? []
  const [creating, setCreating] = useState(false)

  useEffect(() => { if (!selectedId && items.length === 0 && !tasks.loading) setCreating(true) }, [items.length, selectedId, tasks.loading])

  return (
    <div className="split2">
      <div className="pane">
        <div className="pane-head row" style={{ justifyContent: 'space-between' }}>
          <strong>{t.tasks.list}</strong>
          <button className="btn sm primary" onClick={() => { setCreating(true); onSelect(null) }}>＋ {t.tasks.new}</button>
        </div>
        {items.length === 0 && !tasks.loading && <Empty>{t.tasks.empty}</Empty>}
        {items.map((m) => (
          <div key={m.id} className={'list-item' + (selectedId === m.id ? ' active' : '')} onClick={() => { setCreating(false); onSelect(m.id) }}>
            <div className="t ellipsis">{m.title}</div>
            <div className="m">
              <StatusBadge status={m.status} />
              <span>{m.template}</span>
              <span>{m.entry_ids.length} ✦</span>
              <span>{fmtDate(m.created, true)}</span>
            </div>
          </div>
        ))}
      </div>
      <div className="pane">
        {creating || !selectedId ? (
          <NewTask onCreated={(m) => { setCreating(false); tasks.reload(); onSelect(m.id) }} />
        ) : (
          <TaskView id={selectedId} onChanged={tasks.reload} onDeleted={() => { tasks.reload(); onSelect(null) }} openEntry={openEntry} />
        )}
      </div>
    </div>
  )
}

function StatusBadge({ status }: { status: string }) {
  const { t } = useT()
  const cls = status === 'accepted' ? 'ok' : status === 'done' ? 'warn' : ''
  return <span className={'badge ' + cls}>{t.tasks.status[status] ?? status}</span>
}

function NewTask({ onCreated }: { onCreated: (m: TaskMeta) => void }) {
  const { t } = useT()
  const toast = useToast()
  const templates = useAsync(() => api.templates())
  const tree = useAsync(() => api.tree())
  const [template, setTemplate] = useState('')
  const [title, setTitle] = useState('')
  const [agents, setAgents] = useState<string[]>([])
  const [projects, setProjects] = useState<string[]>([])
  const [kinds, setKinds] = useState<string[]>([])
  const [query, setQuery] = useState('')
  const [since, setSince] = useState('')
  const [limit, setLimit] = useState(100)
  const [includeKnowledge, setIncludeKnowledge] = useState(false)
  const [busy, setBusy] = useState(false)

  useEffect(() => { if (!template && templates.data?.length) setTemplate(templates.data[0].id) }, [templates.data, template])

  const scope: TaskScope = useMemo(() => ({ agents, projects, kinds, query: query || null, since: since || null, limit, include_knowledge: includeKnowledge, include_archived: false }), [agents, projects, kinds, query, since, limit, includeKnowledge])
  const allProjects = useMemo(() => {
    const set = new Map<string, number>()
    for (const n of tree.data ?? []) if (!agents.length || agents.includes(n.agent)) for (const p of n.projects) set.set(p.project, (set.get(p.project) ?? 0) + p.count)
    return [...set.entries()]
  }, [tree.data, agents])

  const toggle = (arr: string[], set: (v: string[]) => void, v: string) => set(arr.includes(v) ? arr.filter((x) => x !== v) : [...arr, v])
  const create = async () => {
    setBusy(true)
    try {
      const m = await api.createTask(template, scope, title || undefined)
      onCreated(m)
    } catch (e) { toast(String((e as Error).message), true) } finally { setBusy(false) }
  }
  const tpl = templates.data?.find((x) => x.id === template)

  return (
    <div className="detail">
      <h1>{t.tasks.new}</h1>
      <p className="muted small">{t.tasks.howtoManual}</p>
      <div className="grid2">
        <div>
          <label className="field"><span>{t.tasks.template}</span>
            <select className="input" value={template} onChange={(e) => setTemplate(e.target.value)}>
              {templates.data?.map((x) => <option key={x.id} value={x.id}>{x.name}</option>)}
            </select>
          </label>
          {tpl && <div className="muted small" style={{ marginTop: -6, marginBottom: 10 }}>{tpl.description}</div>}
          <label className="field"><span>{t.tasks.titleField}</span><input className="input" value={title} onChange={(e) => setTitle(e.target.value)} /></label>
          <label className="field"><span>{t.tasks.query}</span><input className="input" value={query} onChange={(e) => setQuery(e.target.value)} /></label>
          <div className="row">
            <label className="field grow"><span>{t.tasks.since}</span><input className="input" type="date" value={since} onChange={(e) => setSince(e.target.value)} /></label>
            <label className="field" style={{ width: 120 }}><span>{t.tasks.limit}</span><input className="input" type="number" min={1} max={2000} value={limit} onChange={(e) => setLimit(Number(e.target.value) || 100)} /></label>
          </div>
          <label className="check"><input type="checkbox" checked={includeKnowledge} onChange={(e) => setIncludeKnowledge(e.target.checked)} />{t.tasks.includeKnowledge}</label>
        </div>
        <div>
          <label className="field"><span>{t.tasks.agents}</span>
            <div className="chips">{tree.data?.map((n) => <button key={n.agent} className={'chip' + (agents.includes(n.agent) ? ' on' : '')} onClick={() => toggle(agents, setAgents, n.agent)}>{n.agent} · {n.count}</button>)}</div>
          </label>
          <label className="field"><span>{t.tasks.projects}</span>
            <div className="chips">{allProjects.map(([p, n]) => <button key={p} className={'chip' + (projects.includes(p) ? ' on' : '')} onClick={() => toggle(projects, setProjects, p)}>{p} · {n}</button>)}</div>
          </label>
          <label className="field"><span>{t.tasks.kinds}</span>
            <div className="chips">{KINDS.filter((k) => k !== 'knowledge').map((k) => <button key={k} className={'chip' + (kinds.includes(k) ? ' on' : '')} onClick={() => toggle(kinds, setKinds, k)}>{t.kinds[k]}</button>)}</div>
          </label>
        </div>
      </div>
      <div className="row" style={{ justifyContent: 'flex-end', marginTop: 8 }}>
        <button className="btn primary" onClick={create} disabled={busy || !template}>{busy ? t.common.loading : '✦ ' + t.tasks.createTask}</button>
      </div>
    </div>
  )
}

function TaskView({ id, onChanged, onDeleted, openEntry }: { id: string; onChanged: () => void; onDeleted: () => void; openEntry: (id: string) => void }) {
  const { t } = useT()
  const toast = useToast()
  const { data, error, loading, reload } = useAsync(() => api.task(id), [id])
  const [tab, setTab] = useState<'howto' | 'task' | 'result'>('howto')
  const [draft, setDraft] = useState('')
  useEffect(() => { setTab(data?.result_md ? 'result' : 'howto'); setDraft('') }, [id, data?.result_md])

  if (loading && !data) return <div className="detail muted">{t.common.loading}</div>
  if (error) return <div className="detail" style={{ color: 'var(--danger)' }}>{error}</div>
  if (!data) return null
  const d: TaskDetail = data
  const win = /^[A-Za-z]:\\/.test(d.dir)
  const sep = win ? '\\' : '/'
  const taskFile = `${d.dir}${sep}TASK.md`
  const resultFile = `${d.dir}${sep}result.md`
  const q = (s: string) => `'${s.replace(/'/g, win ? "''" : `'\\''`)}'`
  // bash on macOS/Linux, PowerShell on Windows (result.md must be UTF-8)
  const cmd = (bin: string, flag: string) => win
    ? `${bin} ${flag} (Get-Content -Raw ${q(taskFile)}) | Out-File -Encoding utf8 ${q(resultFile)}`
    : `${bin} ${flag} "$(cat ${q(taskFile)})" > ${q(resultFile)}`
  const cli = [
    { label: 'Claude Code', cmd: cmd('claude', '-p') },
    { label: 'Codex CLI', cmd: cmd('codex', 'exec') },
    { label: 'Gemini CLI', cmd: cmd('gemini', '-p') },
  ]

  const submit = async () => {
    try { await api.submitResult(d.id, draft); toast(t.settings.saved); setDraft(''); reload(); onChanged() } catch (e) { toast(String((e as Error).message), true) }
  }
  const accept = async () => {
    try { await api.acceptTask(d.id); toast(t.tasks.accepted); reload(); onChanged() } catch (e) { toast(String((e as Error).message), true) }
  }
  const del = async () => {
    if (!confirm(t.common.confirmDelete)) return
    try { await api.deleteTask(d.id); onDeleted() } catch (e) { toast(String((e as Error).message), true) }
  }

  return (
    <div className="detail">
      <div className="row" style={{ alignItems: 'flex-start' }}>
        <div className="grow">
          <h1 style={{ marginBottom: 6 }}>{d.title}</h1>
          <div className="row wrap small">
            <StatusBadge status={d.status} />
            <span className="badge">{d.template}</span>
            <span className="muted">{t.tasks.entries(d.entry_ids.length, d.inlined)}</span>
            <span className="muted">· {t.tasks.bytes} {fmtBytes(d.task_bytes)}</span>
          </div>
        </div>
        <div className="row">
          {d.knowledge_id && <button className="btn sm" onClick={() => openEntry(d.knowledge_id!)}>{t.tasks.viewKnowledge}</button>}
          <button className="btn sm danger" onClick={del}>{t.tasks.deleteTask}</button>
        </div>
      </div>
      <dl className="meta">
        <dt>ID</dt><dd className="mono">{d.id}</dd>
        <dt>{t.tasks.taskDir}</dt><dd className="mono">{d.dir}</dd>
        <dt>{t.common.created}</dt><dd>{fmtDate(d.created, true)}</dd>
      </dl>

      <Tabs tabs={[{ id: 'howto', label: t.tasks.howto }, { id: 'task', label: t.tasks.taskMd }, { id: 'result', label: t.tasks.result }]} value={tab} onChange={setTab} />

      {tab === 'howto' && (
        <div>
          <p>{t.tasks.howtoMcp}</p>
          <CopyBox text={t.tasks.mcpPrompt(d.id)} />
          <p style={{ marginTop: 16 }}>{t.tasks.howtoCli}</p>
          {cli.map((c) => (
            <div key={c.label} style={{ marginBottom: 8 }}>
              <div className="small muted">{c.label}</div>
              <CopyBox text={c.cmd} />
            </div>
          ))}
          <p style={{ marginTop: 16 }}>{t.tasks.howtoManual}</p>
          <div className="row"><CopyButton text={d.task_md} label={`${t.common.copy} TASK.md`} className="btn" /></div>
          <h3 style={{ marginTop: 20 }}>{t.tasks.preview}</h3>
          {d.entries.map((e) => (
            <div key={e.id} className="row small" style={{ padding: '4px 0', borderBottom: '1px solid var(--border)', cursor: 'pointer' }} onClick={() => openEntry(e.id)}>
              <AgentBadge agent={e.agent} /><span className="grow ellipsis">{e.title}</span><span className="muted">{e.project}</span><KindBadge kind={e.kind} />
            </div>
          ))}
        </div>
      )}
      {tab === 'task' && (
        <div>
          <div className="row" style={{ justifyContent: 'flex-end', marginBottom: 8 }}><CopyButton text={d.task_md} /></div>
          <Markdown>{d.task_md}</Markdown>
        </div>
      )}
      {tab === 'result' && (
        <div>
          {d.result_md ? (
            <>
              <div className="row" style={{ justifyContent: 'flex-end', marginBottom: 8, gap: 8 }}>
                <CopyButton text={d.result_md} />
                {d.status !== 'accepted' && <button className="btn primary" onClick={accept}>✓ {t.tasks.accept}</button>}
              </div>
              <Markdown>{d.result_md}</Markdown>
            </>
          ) : (
            <p className="muted">{t.tasks.noResultYet}</p>
          )}
          <h3 style={{ marginTop: 20 }}>{t.tasks.pasteResult}</h3>
          <textarea className="input" style={{ minHeight: 200 }} value={draft} onChange={(e) => setDraft(e.target.value)} placeholder={t.tasks.pasteResult} />
          <div className="row" style={{ justifyContent: 'flex-end', marginTop: 8 }}>
            <button className="btn primary" onClick={submit} disabled={!draft.trim()}>{t.tasks.submit}</button>
          </div>
        </div>
      )}
    </div>
  )
}
