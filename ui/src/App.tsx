import { useEffect, useMemo, useState } from 'react'
import { api } from './api/client'
import { I18nContext, dicts, normalizeLang } from './i18n'
import { ToastProvider } from './components/common'
import { Overview } from './pages/Overview'
import { Memories } from './pages/Memories'
import { Knowledge } from './pages/Knowledge'
import { Tasks } from './pages/Tasks'
import { Sources } from './pages/Sources'
import { Settings } from './pages/Settings'

type Page = 'overview' | 'memories' | 'knowledge' | 'tasks' | 'sources' | 'settings'
const PAGES: Page[] = ['overview', 'memories', 'knowledge', 'tasks', 'sources', 'settings']
const NAV: { id: Page; ico: string }[] = [
  { id: 'overview', ico: '◫' }, { id: 'memories', ico: '▤' }, { id: 'knowledge', ico: '✦' },
  { id: 'tasks', ico: '☑' }, { id: 'sources', ico: '⇄' }, { id: 'settings', ico: '⚙' },
]

/** Routes look like `#/memories/<id>`; the id part is optional. */
function parseHash(): { page: Page; id: string | null } {
  const parts = location.hash.replace(/^#\/?/, '').split('/')
  const page = (PAGES as string[]).includes(parts[0]) ? (parts[0] as Page) : 'overview'
  return { page, id: parts[1] ? decodeURIComponent(parts[1]) : null }
}

export default function App() {
  const urlLang = new URLSearchParams(location.search).get('lang')
  const [lang, setLangState] = useState<string>(() => normalizeLang(urlLang ?? localStorage.getItem('memhub.lang')))
  const initial = parseHash()
  const [page, setPage] = useState<Page>(initial.page)
  const [entryId, setEntryId] = useState<string | null>(initial.page === 'memories' ? initial.id : null)
  const [knowledgeId, setKnowledgeId] = useState<string | null>(initial.page === 'knowledge' ? initial.id : null)
  const [taskId, setTaskId] = useState<string | null>(initial.page === 'tasks' ? initial.id : null)
  const [version, setVersion] = useState('')

  const setLang = (l: string) => { const n = normalizeLang(l); setLangState(n); localStorage.setItem('memhub.lang', n) }
  useEffect(() => {
    if (!urlLang) api.config().then((c) => setLang(c.language)).catch(() => {})
    api.overview().then((o) => setVersion(o.version)).catch(() => {})
  }, [])

  // keep the URL hash in sync (deep links, browser back/forward in browser mode)
  const currentId = page === 'memories' ? entryId : page === 'knowledge' ? knowledgeId : page === 'tasks' ? taskId : null
  useEffect(() => {
    const next = '#/' + page + (currentId ? '/' + encodeURIComponent(currentId) : '')
    if (location.hash !== next) history.replaceState(null, '', next)
  }, [page, currentId])
  useEffect(() => {
    const onHash = () => {
      const h = parseHash()
      setPage(h.page)
      if (h.page === 'memories') setEntryId(h.id)
      if (h.page === 'knowledge') setKnowledgeId(h.id)
      if (h.page === 'tasks') setTaskId(h.id)
    }
    window.addEventListener('hashchange', onHash)
    return () => window.removeEventListener('hashchange', onHash)
  }, [])
  const i18n = useMemo(() => ({ t: dicts[lang] ?? dicts['zh-CN'], lang, setLang }), [lang])
  const t = i18n.t

  const openEntry = (id: string) => { setEntryId(id); setPage('memories') }

  return (
    <I18nContext.Provider value={i18n}>
      <ToastProvider>
        <div className="app">
          <aside className="sidebar">
            <div className="brand">
              <div className="logo">M</div>
              <div><div className="name">{t.app}</div><div className="sub">{t.tagline}</div></div>
            </div>
            {NAV.map((n) => (
              <button key={n.id} className={'nav-item' + (page === n.id ? ' active' : '')} onClick={() => setPage(n.id)}>
                <span className="ico">{n.ico}</span>{t.nav[n.id]}
              </button>
            ))}
            <div className="spacer" />
            <div className="foot">v{version || '…'} · local-first</div>
          </aside>
          <main className="main">
            <div className="topbar"><h1>{t.nav[page]}</h1></div>
            <div className={'content' + (['memories', 'knowledge', 'tasks'].includes(page) ? ' flush' : '')}>
              {page === 'overview' && <Overview go={(p) => setPage(p as Page)} openEntry={openEntry} />}
              {page === 'memories' && <Memories selectedId={entryId} onSelect={setEntryId} />}
              {page === 'knowledge' && <Knowledge selectedId={knowledgeId} onSelect={setKnowledgeId} go={(p) => setPage(p as Page)} openEntry={openEntry} />}
              {page === 'tasks' && <Tasks selectedId={taskId} onSelect={setTaskId} openEntry={openEntry} />}
              {page === 'sources' && <Sources />}
              {page === 'settings' && <Settings />}
            </div>
          </main>
        </div>
      </ToastProvider>
    </I18nContext.Provider>
  )
}
