import { useEffect, useState } from 'react'
import { api, type Config } from '../api/client'
import { useT } from '../i18n'
import { CopyBox, Tabs, useAsync, useToast } from '../components/common'

export function Settings() {
  const { t, setLang } = useT()
  const toast = useToast()
  const cfg = useAsync(() => api.config())
  const paths = useAsync(() => api.paths())
  const [form, setForm] = useState<Config | null>(null)
  const [bin, setBin] = useState('')
  const snippets = useAsync(() => api.snippets(bin || undefined), [bin])
  const [snip, setSnip] = useState('claude-code')

  useEffect(() => { if (cfg.data) setForm(cfg.data) }, [cfg.data])
  useEffect(() => { api.sidecarPath().then((p) => { if (p) setBin(p) }).catch(() => {}) }, [])

  const save = async () => {
    if (!form) return
    try {
      const r = await api.saveConfig(form)
      setLang(form.language)
      toast(r.restart_required ? t.settings.restart : t.settings.saved)
      cfg.reload()
    } catch (e) { toast(String((e as Error).message), true) }
  }
  const reindex = async () => {
    try { const r = await api.reindex(); toast(t.settings.reindexed(r.indexed)) } catch (e) { toast(String((e as Error).message), true) }
  }

  if (!form) return <div className="muted">{t.common.loading}</div>
  const set = <K extends keyof Config>(k: K, v: Config[K]) => setForm({ ...form, [k]: v })
  const current = snippets.data?.find((s) => s.id === snip) ?? snippets.data?.[0]

  return (
    <div style={{ maxWidth: 980 }}>
      <div className="grid2">
        <div className="card">
          <h3>{t.settings.general}</h3>
          <label className="field"><span>{t.settings.language}</span>
            <select className="input" value={form.language.startsWith('zh') ? 'zh-CN' : 'en'} onChange={(e) => set('language', e.target.value)}>
              <option value="zh-CN">中文</option><option value="en">English</option>
            </select>
          </label>
          <label className="check"><input type="checkbox" checked={form.git_snapshot} onChange={(e) => set('git_snapshot', e.target.checked)} />{t.settings.gitSnapshot}</label>
          <label className="check"><input type="checkbox" checked={form.redact_secrets} onChange={(e) => set('redact_secrets', e.target.checked)} />{t.settings.redact}</label>
          <div className="row">
            <label className="field grow"><span>{t.settings.maxSize}</span><input className="input" type="number" value={form.max_file_size_kb} onChange={(e) => set('max_file_size_kb', Number(e.target.value) || 2048)} /></label>
            <label className="field grow"><span>{t.settings.budget}</span><input className="input" type="number" value={form.task_context_budget_kb} onChange={(e) => set('task_context_budget_kb', Number(e.target.value) || 200)} /></label>
          </div>
          <label className="field"><span>{t.settings.vault}</span><input className="input mono" value={form.vault} onChange={(e) => set('vault', e.target.value)} /><span className="muted" style={{ marginTop: 4 }}>{t.settings.vaultHint}</span></label>
          <div className="row" style={{ justifyContent: 'flex-end' }}>
            <button className="btn" onClick={reindex}>{t.settings.reindex}</button>
            <button className="btn primary" onClick={save}>{t.common.save}</button>
          </div>
        </div>
        <div className="card">
          <h3>{t.settings.paths}</h3>
          {paths.data && (
            <div className="kv mono small">
              {(['home', 'config', 'vault', 'index', 'tasks', 'templates'] as const).map((k) => (
                <div key={k} style={{ display: 'contents' }}><span className="k">{k}</span><span style={{ wordBreak: 'break-all' }}>{paths.data![k]}</span></div>
              ))}
            </div>
          )}
        </div>
      </div>

      <div className="card section">
        <h3>{t.settings.mcp}</h3>
        <p className="muted small">{t.settings.mcpHint}</p>
        <label className="field"><span>{t.settings.binPath}</span><input className="input mono" value={bin} onChange={(e) => setBin(e.target.value)} placeholder={snippets.data ? '' : 'memhub'} /></label>
        {snippets.data && (
          <>
            <Tabs tabs={snippets.data.map((s) => ({ id: s.id, label: s.label }))} value={current?.id ?? ''} onChange={setSnip} />
            {current && (
              <div>
                <div className="small muted" style={{ marginBottom: 6 }}>{current.file} · {current.hint}</div>
                <CopyBox text={current.snippet} />
              </div>
            )}
          </>
        )}
      </div>
    </div>
  )
}
