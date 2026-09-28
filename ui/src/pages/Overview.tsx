import { useState } from 'react'
import { api } from '../api/client'
import { useT } from '../i18n'
import { AgentBadge, KindBadge, agentColor, fmtBytes, fmtDate, useAsync, useToast } from '../components/common'

export function Overview({ go, openEntry }: { go: (page: string) => void; openEntry: (id: string) => void }) {
  const { t } = useT()
  const toast = useToast()
  const { data, loading, error, reload } = useAsync(() => api.overview())
  const [syncing, setSyncing] = useState(false)

  const sync = async () => {
    setSyncing(true)
    try {
      const r = await api.sync()
      toast(t.overview.syncResult(r))
      reload()
    } catch (e) { toast(String((e as Error).message), true) } finally { setSyncing(false) }
  }

  if (loading && !data) return <div className="muted">{t.common.loading}</div>
  if (error) return <div style={{ color: 'var(--danger)' }}>{error}</div>
  if (!data) return null
  const { stats, sources, last_sync, recent } = data
  const installed = sources.filter((s) => s.installed && s.enabled)
  const maxAgent = Math.max(1, ...stats.by_agent.map(([, n]) => n))

  return (
    <div>
      <div className="row" style={{ justifyContent: 'space-between', marginBottom: 14 }}>
        <div className="muted small">
          {t.overview.lastSync}: {last_sync ? `${fmtDate(last_sync.at, true)} · ${t.overview.syncResult(last_sync)}` : t.overview.never}
        </div>
        <button className="btn primary" onClick={sync} disabled={syncing}>{syncing ? t.common.syncing : '⟳ ' + t.common.sync}</button>
      </div>

      <div className="cards">
        <div className="card stat"><div className="l">{t.overview.entries}</div><div className="v">{stats.total}</div><div className="muted small">{t.overview.archived}: {stats.archived}</div></div>
        <div className="card stat"><div className="l">{t.overview.agents}</div><div className="v">{stats.by_agent.length}</div><div className="muted small">{installed.map((s) => s.label).join(' · ') || '—'}</div></div>
        <div className="card stat"><div className="l">{t.overview.pendingTasks}</div><div className="v">{data.pending_tasks}</div><div className="muted small"><a onClick={() => go('tasks')} href="#">{t.nav.tasks} →</a></div></div>
        <div className="card stat"><div className="l">{t.overview.vault}</div><div className="v" style={{ fontSize: 18 }}>{fmtBytes(stats.vault_bytes)}</div><div className="muted small mono ellipsis" title={data.paths.vault}>{data.paths.vault_display}</div></div>
      </div>

      {stats.total === 0 && (
        <div className="card section" style={{ textAlign: 'center' }}>
          <p className="muted">{t.overview.emptyHint}</p>
          <button className="btn primary" onClick={() => go('sources')}>{t.overview.goSources}</button>
        </div>
      )}

      <div className="grid2 section">
        <div className="card">
          <h3>{t.overview.byAgent}</h3>
          {stats.by_agent.map(([agent, n]) => (
            <div key={agent} className="row" style={{ margin: '8px 0' }}>
              <div style={{ width: 120 }}><AgentBadge agent={agent} /></div>
              <div className="grow bar"><i style={{ width: `${(n / maxAgent) * 100}%`, background: agentColor(agent) }} /></div>
              <div style={{ width: 40, textAlign: 'right' }} className="small">{n}</div>
            </div>
          ))}
          {stats.by_agent.length === 0 && <div className="muted small">—</div>}
        </div>
        <div className="card">
          <h3>{t.overview.byKind}</h3>
          <div className="chips" style={{ marginTop: 6 }}>
            {stats.by_kind.map(([k, n]) => (
              <span key={k} className="chip"><KindBadge kind={k} /> &nbsp;{n}</span>
            ))}
          </div>
          <h3 style={{ marginTop: 18 }}>{t.overview.detected}</h3>
          <div className="chips">
            {sources.filter((s) => s.builtin).map((s) => (
              <span key={s.name} className={'chip' + (s.installed && s.enabled ? ' on' : '')} title={s.root ?? ''}>
                {s.installed ? '●' : '○'} {s.label}
              </span>
            ))}
          </div>
        </div>
      </div>

      <div className="card section">
        <h3>{t.overview.recent}</h3>
        {recent.map((e) => (
          <div key={e.id} className="row" style={{ padding: '6px 0', borderBottom: '1px solid var(--border)', cursor: 'pointer' }} onClick={() => openEntry(e.id)}>
            <AgentBadge agent={e.agent} />
            <span className="grow ellipsis">{e.title}</span>
            <span className="badge">{e.project}</span>
            <KindBadge kind={e.kind} />
            <span className="muted small" style={{ width: 130, textAlign: 'right' }}>{fmtDate(e.updated, true)}</span>
          </div>
        ))}
      </div>

      {last_sync && last_sync.errors.length > 0 && (
        <div className="card section">
          <h3>{t.overview.errors}</h3>
          <pre className="small">{last_sync.errors.join('\n')}</pre>
        </div>
      )}
    </div>
  )
}
