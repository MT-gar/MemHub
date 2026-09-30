import { useMemo, useState } from 'react'
import { api, type Rule, type RuleStatus } from '../api/client'
import { useT } from '../i18n'
import { AgentBadge, CopyButton, Empty, Tabs, fmtDate, useAsync, useDebounced, useToast } from '../components/common'

type Tab = RuleStatus | 'all'

const scopeLabel = (scope: string, globalLabel: string) => (scope === 'global' ? globalLabel : scope.replace('project:', '▸ '))

export function Rules({ openEntry }: { openEntry: (id: string) => void }) {
  const { t } = useT()
  const toast = useToast()
  const all = useAsync(() => api.rules())
  const [tab, setTab] = useState<Tab>('draft')
  const [scope, setScope] = useState('')
  const [previewProject, setPreviewProject] = useState('')
  const debouncedProject = useDebounced(previewProject, 300)
  const preview = useAsync(() => api.previewRules(debouncedProject.trim() || undefined), [debouncedProject, all.data])

  const rules = all.data ?? []
  const counts = useMemo(() => {
    const c: Record<string, number> = { draft: 0, approved: 0, retired: 0, all: rules.length }
    rules.forEach((r) => { c[r.status]++ })
    return c
  }, [rules])
  const scopes = useMemo(() => Array.from(new Set(rules.map((r) => r.scope))).sort(), [rules])
  const shown = rules.filter((r) => (tab === 'all' || r.status === tab) && (!scope || r.scope === scope))

  const act = async (fn: () => Promise<unknown>, okMsg?: string) => {
    try { await fn(); if (okMsg) toast(okMsg); all.reload() } catch (e) { toast(String((e as Error).message), true) }
  }
  const approve = (r: Rule) => {
    if (r.warnings.length && !confirm(t.rules.confirmWarn(r.warnings.join('\n• ')))) return
    return act(() => api.setRuleStatus(r.id, 'approved'))
  }
  const remove = (r: Rule) => { if (confirm(t.common.confirmDelete)) act(() => api.deleteEntry(r.id)) }

  return (
    <div>
      <p className="muted" style={{ marginTop: 0, maxWidth: 820 }}>{t.rules.intro}</p>

      <div className="grid2" style={{ alignItems: 'start' }}>
        <div>
          <NewRule scopes={scopes} onAdd={async (p) => {
            try {
              const r = await api.addRule(p)
              toast(r.created ? t.rules.added : t.rules.exists)
              if (r.created) setTab(r.rule.status)
              all.reload()
              return true
            } catch (e) { toast(String((e as Error).message), true); return false }
          }} />

          <div className="row" style={{ justifyContent: 'space-between', marginTop: 16 }}>
            <Tabs<Tab> value={tab} onChange={setTab}
              tabs={(['draft', 'approved', 'retired', 'all'] as Tab[]).map((k) => ({ id: k, label: `${t.rules.tabs[k]} ${counts[k]}` }))} />
            <select className="input" style={{ width: 190, marginBottom: 14 }} value={scope} onChange={(e) => setScope(e.target.value)}>
              <option value="">{t.rules.scopeAll}</option>
              {scopes.map((s) => <option key={s} value={s}>{scopeLabel(s, t.rules.globalShort)}</option>)}
            </select>
          </div>

          {all.error && <div className="muted">{all.error}</div>}
          {!all.loading && shown.length === 0 && <Empty>{tab === 'draft' ? t.rules.emptyDraft : t.rules.empty}</Empty>}
          {shown.map((r) => (
            <RuleCard key={r.id} r={r}
              onApprove={() => approve(r)}
              onStatus={(s) => act(() => api.setRuleStatus(r.id, s))}
              onDelete={() => remove(r)}
              onSave={(text, detail) => act(() => api.editRule(r.id, { text, detail }))}
              openEntry={openEntry} />
          ))}
        </div>

        <div>
          <div className="card">
            <h3 style={{ marginTop: 0 }}>{t.rules.preview}</h3>
            <label className="field"><span>{t.rules.previewFor}</span>
              <input className="input mono" value={previewProject} onChange={(e) => setPreviewProject(e.target.value)} placeholder={t.rules.previewHint} />
            </label>
            {preview.data && (
              <>
                <pre className="mono small" style={{ whiteSpace: 'pre-wrap', background: 'var(--bg-sub)', padding: 10, borderRadius: 8, margin: '8px 0', maxHeight: 320, overflow: 'auto' }}>
                  {preview.data.markdown || t.rules.noneApproved}
                </pre>
                <div className="row" style={{ justifyContent: 'space-between' }}>
                  <span className="muted small">
                    {t.rules.bytes(preview.data.bytes)}
                    {preview.data.truncated > 0 && <> · <span style={{ color: 'var(--warn)' }}>{t.rules.budget(preview.data.truncated, preview.data.total_approved)}</span></>}
                  </span>
                  {preview.data.markdown && <CopyButton text={preview.data.markdown} label={t.common.copy} />}
                </div>
              </>
            )}
          </div>

          <div className="card" style={{ marginTop: 16 }}>
            <h3 style={{ marginTop: 0 }}>{t.rules.connect}</h3>
            <p className="muted small">{t.rules.connectHint}</p>
            <div className="mono small" style={{ background: 'var(--bg-sub)', padding: 10, borderRadius: 8, marginBottom: 8 }}>{t.rules.connectLine}</div>
            <div className="row" style={{ justifyContent: 'flex-end' }}><CopyButton text={t.rules.connectLine} label={t.common.copy} /></div>
          </div>
        </div>
      </div>
    </div>
  )
}

function RuleCard({ r, onApprove, onStatus, onDelete, onSave, openEntry }: {
  r: Rule; onApprove: () => void; onStatus: (s: RuleStatus) => void; onDelete: () => void
  onSave: (text: string, detail: string) => void; openEntry: (id: string) => void
}) {
  const { t } = useT()
  const [editing, setEditing] = useState(false)
  const [text, setText] = useState(r.text)
  const [detail, setDetail] = useState(r.detail)
  const cls = r.status === 'approved' ? 'ok' : r.status === 'draft' ? 'warn' : ''
  return (
    <div className="card" style={{ marginBottom: 10, opacity: r.status === 'retired' ? 0.7 : 1 }}>
      {editing ? (
        <>
          <input className="input" value={text} onChange={(e) => setText(e.target.value)} />
          <textarea className="input" style={{ minHeight: 60, marginTop: 8, fontFamily: 'inherit' }} value={detail} onChange={(e) => setDetail(e.target.value)} placeholder={t.rules.detail} />
          <div className="row" style={{ justifyContent: 'flex-end', marginTop: 8 }}>
            <button className="btn sm" onClick={() => { setEditing(false); setText(r.text); setDetail(r.detail) }}>{t.common.cancel}</button>
            <button className="btn sm primary" disabled={!text.trim()} onClick={() => { onSave(text, detail); setEditing(false) }}>{t.rules.saveEdit}</button>
          </div>
        </>
      ) : (
        <>
          <div style={{ fontSize: 15, lineHeight: 1.45 }}>{r.text}</div>
          {r.detail && <div className="muted small" style={{ marginTop: 4 }}>{r.detail}</div>}
        </>
      )}
      {r.warnings.length > 0 && (
        <div className="small" style={{ color: 'var(--warn)', marginTop: 6 }}>⚠ {t.rules.warnings}: {r.warnings.join(' · ')}</div>
      )}
      <div className="row" style={{ marginTop: 8, gap: 6, flexWrap: 'wrap' }}>
        <span className={'badge ' + cls}>{t.rules.tabs[r.status]}</span>
        <span className="badge kind">{scopeLabel(r.scope, t.rules.globalShort)}</span>
        <span className="muted small">{t.rules.by}</span><AgentBadge agent={r.agent} />
        {r.verified && <span className="muted small">{t.rules.verified} {r.verified}</span>}
        {r.status === 'draft' && <span className="muted small">{fmtDate(r.created)}</span>}
        {r.sources.length > 0 && (
          <span className="muted small">{t.rules.sources}: {r.sources.slice(0, 3).map((s) => (
            <a key={s} href="#" style={{ marginRight: 6 }} onClick={(e) => { e.preventDefault(); openEntry(s) }}>{s.slice(-6)}</a>
          ))}</span>
        )}
        <div className="grow" />
        {!editing && r.status !== 'approved' && <button className="btn sm primary" onClick={onApprove}>✓ {r.status === 'retired' ? t.rules.reapprove : t.rules.approve}</button>}
        {!editing && r.status === 'approved' && <button className="btn sm" onClick={() => onStatus('retired')}>{t.rules.retire}</button>}
        {!editing && r.status === 'retired' && <button className="btn sm" onClick={() => onStatus('draft')}>{t.rules.toDraft}</button>}
        {!editing && <button className="btn sm" onClick={() => setEditing(true)}>{t.common.edit}</button>}
        {!editing && r.status !== 'approved' && <button className="btn sm danger" onClick={onDelete}>{t.common.delete}</button>}
      </div>
    </div>
  )
}

function NewRule({ scopes, onAdd }: {
  scopes: string[]; onAdd: (p: { text: string; detail?: string; scope: string; status: RuleStatus }) => Promise<boolean>
}) {
  const { t } = useT()
  const [text, setText] = useState('')
  const [detail, setDetail] = useState('')
  const [scopeSel, setScopeSel] = useState('global')
  const [project, setProject] = useState('')
  const [draft, setDraft] = useState(false)
  const projects = scopes.filter((s) => s !== 'global')
  const scope = scopeSel === '__new' ? (project.trim() ? `project:${project.trim()}` : '') : scopeSel
  return (
    <div className="card">
      <h3 style={{ marginTop: 0 }}>{t.rules.newRule}</h3>
      <label className="field"><span>{t.rules.text}</span>
        <input className="input" value={text} maxLength={300} onChange={(e) => setText(e.target.value)} placeholder={t.rules.textPh} />
      </label>
      <div className="row">
        <label className="field grow"><span>{t.tasks.scope}</span>
          <select className="input" value={scopeSel} onChange={(e) => setScopeSel(e.target.value)}>
            <option value="global">{t.rules.global}</option>
            {projects.map((p) => <option key={p} value={p}>{scopeLabel(p, t.rules.globalShort)}</option>)}
            <option value="__new">＋ {t.rules.project}…</option>
          </select>
        </label>
        {scopeSel === '__new' && (
          <label className="field grow"><span>{t.rules.projectName}</span>
            <input className="input" value={project} onChange={(e) => setProject(e.target.value)} placeholder="my-app" />
          </label>
        )}
      </div>
      <label className="field"><span>{t.rules.detail}</span>
        <input className="input" value={detail} onChange={(e) => setDetail(e.target.value)} />
      </label>
      <div className="row" style={{ justifyContent: 'space-between', marginTop: 4 }}>
        <label className="row small" style={{ gap: 6 }}><input type="checkbox" checked={draft} onChange={(e) => setDraft(e.target.checked)} />{t.rules.asDraft}</label>
        <button className="btn primary sm" disabled={!text.trim() || !scope} onClick={async () => {
          if (await onAdd({ text, detail: detail.trim() || undefined, scope, status: draft ? 'draft' : 'approved' })) { setText(''); setDetail('') }
        }}>＋ {t.rules.add}</button>
      </div>
    </div>
  )
}
