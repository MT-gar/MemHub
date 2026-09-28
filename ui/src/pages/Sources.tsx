import { useState } from 'react'
import { api, KINDS, type Kind, type SyncReport } from '../api/client'
import { useT } from '../i18n'
import { Switch, useAsync, useToast } from '../components/common'

export function Sources() {
  const { t } = useT()
  const toast = useToast()
  const sources = useAsync(() => api.sources())
  const config = useAsync(() => api.config())
  const [report, setReport] = useState<SyncReport | null>(null)
  const [busy, setBusy] = useState(false)

  const run = async (fn: () => Promise<SyncReport>) => {
    setBusy(true)
    try {
      const r = await fn()
      setReport(r)
      toast(t.overview.syncResult(r))
      sources.reload(); config.reload()
    } catch (e) { toast(String((e as Error).message), true) } finally { setBusy(false) }
  }

  const builtin = (sources.data ?? []).filter((s) => s.builtin)
  const generic = (sources.data ?? []).filter((s) => !s.builtin)

  return (
    <div>
      <div className="row" style={{ justifyContent: 'space-between', marginBottom: 14 }}>
        <div className="muted small">{report ? `${t.sources.lastReport}: ${t.overview.syncResult(report)}` : ''}</div>
        <button className="btn primary" disabled={busy} onClick={() => run(api.sync)}>{busy ? t.common.syncing : '⟳ ' + t.common.sync}</button>
      </div>

      <h3>{t.sources.builtin}</h3>
      <div className="cards" style={{ gridTemplateColumns: 'repeat(auto-fill, minmax(330px, 1fr))' }}>
        {builtin.map((s) => (
          <div key={s.name} className="card source-card">
            <div className="body">
              <div className="row"><strong>{s.label}</strong>
                <span className={'badge ' + (s.installed ? 'ok' : '')}>{s.installed ? t.common.installed : t.common.notInstalled}</span>
              </div>
              <div className="desc">{s.description}</div>
              <div className="mono small muted ellipsis" title={s.root ?? ''}>{s.root}</div>
            </div>
            <Switch on={s.enabled} disabled={busy} onChange={(v) => run(() => api.setSourceEnabled(s.type, v))} />
          </div>
        ))}
      </div>

      <div className="grid2 section">
        <div className="card">
          <h3>{t.sources.generic}</h3>
          <p className="muted small">{t.sources.genericHint}</p>
          {generic.length === 0 && <div className="muted small">{t.sources.noGeneric}</div>}
          {generic.map((s) => (
            <div key={s.name} className="row" style={{ padding: '6px 0', borderBottom: '1px solid var(--border)' }}>
              <div className="grow">
                <div><strong>{s.name}</strong> <span className={'badge ' + (s.installed ? 'ok' : 'err')}>{s.installed ? t.common.installed : t.common.notInstalled}</span></div>
                <div className="mono small muted">{s.root} · {s.description}</div>
              </div>
              <Switch on={s.enabled} disabled={busy} onChange={(v) => run(() => api.setSourceEnabled('generic', v, s.name))} />
              <button className="btn sm danger" disabled={busy} onClick={() => run(() => api.removeSource(s.name))}>{t.common.remove}</button>
            </div>
          ))}
          <AddGeneric busy={busy} onAdd={(p) => run(() => api.addSource({ type: 'generic', ...p }))} />
        </div>

        <div className="card">
          <h3>{t.sources.projects}</h3>
          <p className="muted small">{t.sources.projectHint}</p>
          {(config.data?.projects ?? []).length === 0 && <div className="muted small">{t.sources.noProjects}</div>}
          {config.data?.projects.map((p) => (
            <div key={p.path} className="row" style={{ padding: '6px 0', borderBottom: '1px solid var(--border)' }}>
              <div className="grow"><strong>{p.name ?? p.path.split(/[\\/]/).pop()}</strong><div className="mono small muted">{p.path}</div></div>
              <button className="btn sm danger" disabled={busy} onClick={() => run(() => api.removeProject(p.path))}>{t.common.remove}</button>
            </div>
          ))}
          <AddProject busy={busy} onAdd={(path, name) => run(() => api.addProject(path, name))} />
        </div>
      </div>
    </div>
  )
}

function AddGeneric({ busy, onAdd }: { busy: boolean; onAdd: (p: { name: string; root: string; include?: string[]; kind?: Kind }) => void }) {
  const { t } = useT()
  const [name, setName] = useState('')
  const [root, setRoot] = useState('')
  const [include, setInclude] = useState('')
  const [kind, setKind] = useState<Kind>('memory')
  return (
    <div style={{ marginTop: 12 }}>
      <div className="row">
        <label className="field grow"><span>{t.sources.name}</span><input className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder="my-agent" /></label>
        <label className="field" style={{ width: 140 }}><span>{t.sources.kind}</span>
          <select className="input" value={kind} onChange={(e) => setKind(e.target.value as Kind)}>{KINDS.filter((k) => k !== 'knowledge').map((k) => <option key={k} value={k}>{t.kinds[k]}</option>)}</select>
        </label>
      </div>
      <label className="field"><span>{t.sources.root}</span><input className="input mono" value={root} onChange={(e) => setRoot(e.target.value)} placeholder="~/bots/my-agent/memory" /></label>
      <label className="field"><span>{t.sources.include}</span><input className="input mono" value={include} onChange={(e) => setInclude(e.target.value)} placeholder="**/*.md" /></label>
      <div className="row" style={{ justifyContent: 'flex-end' }}>
        <button className="btn primary sm" disabled={busy || !name.trim() || !root.trim()} onClick={() => { onAdd({ name: name.trim(), root: root.trim(), include: include.split(',').map((s) => s.trim()).filter(Boolean), kind }); setName(''); setRoot(''); setInclude('') }}>＋ {t.sources.addGeneric}</button>
      </div>
    </div>
  )
}

function AddProject({ busy, onAdd }: { busy: boolean; onAdd: (path: string, name?: string) => void }) {
  const { t } = useT()
  const [path, setPath] = useState('')
  const [name, setName] = useState('')
  return (
    <div style={{ marginTop: 12 }}>
      <label className="field"><span>{t.sources.path}</span><input className="input mono" value={path} onChange={(e) => setPath(e.target.value)} placeholder="~/dev/my-app" /></label>
      <div className="row">
        <label className="field grow"><span>{t.sources.name}</span><input className="input" value={name} onChange={(e) => setName(e.target.value)} placeholder="(optional)" /></label>
        <button className="btn primary sm" style={{ marginTop: 8 }} disabled={busy || !path.trim()} onClick={() => { onAdd(path.trim(), name.trim() || undefined); setPath(''); setName('') }}>＋ {t.sources.addProject}</button>
      </div>
    </div>
  )
}
