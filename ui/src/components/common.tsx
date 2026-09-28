import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { useT } from '../i18n'
import type { Kind } from '../api/client'

// ---------- toast ----------
type Toast = { id: number; text: string; err?: boolean }
const ToastCtx = createContext<(text: string, err?: boolean) => void>(() => {})
export const useToast = () => useContext(ToastCtx)

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([])
  const push = useCallback((text: string, err = false) => {
    const id = Date.now() + Math.random()
    setToasts((t) => [...t, { id, text, err }])
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), err ? 6000 : 2500)
  }, [])
  return (
    <ToastCtx.Provider value={push}>
      {children}
      {toasts.map((t, i) => (
        <div key={t.id} className={'toast' + (t.err ? ' err' : '')} style={{ bottom: 18 + i * 48 }}>{t.text}</div>
      ))}
    </ToastCtx.Provider>
  )
}

// ---------- helpers ----------
export function agentColor(agent: string): string {
  const fixed: Record<string, string> = {
    'claude-code': '#c96a2b', codex: '#111827', gemini: '#2f6fed', openclaw: '#d23f5f', cursor: '#7c3aed',
    windsurf: '#0d9488', cline: '#4b5563', copilot: '#1f2937', 'agents-md': '#6b7280', memhub: '#5b6cff', kiro: '#9333ea',
  }
  if (fixed[agent]) return fixed[agent]
  let h = 0
  for (const c of agent) h = (h * 31 + c.charCodeAt(0)) % 360
  return `hsl(${h} 55% 45%)`
}

export function AgentBadge({ agent }: { agent: string }) {
  return <span className="badge agent" style={{ background: agentColor(agent) }}>{agent}</span>
}

export function KindBadge({ kind }: { kind: Kind | string }) {
  const { t } = useT()
  return <span className="badge kind">{t.kinds[kind] ?? kind}</span>
}

/** Local time as `YYYY-MM-DD` (+ ` HH:mm`) — stable across browser locales. */
export function fmtDate(s?: string | null, withTime = false): string {
  if (!s) return '—'
  const d = new Date(s)
  if (isNaN(d.getTime())) return s
  const p = (n: number) => String(n).padStart(2, '0')
  const date = `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`
  return withTime ? `${date} ${p(d.getHours())}:${p(d.getMinutes())}` : date
}

export function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  return `${(n / 1024 / 1024).toFixed(1)} MB`
}

export function Snippet({ text }: { text?: string }) {
  if (!text) return null
  const parts = text.split(/\[\[(.+?)\]\]/g)
  return <span>{parts.map((p, i) => (i % 2 ? <mark key={i}>{p}</mark> : <span key={i}>{p}</span>))}</span>
}

export function Markdown({ children }: { children: string }) {
  return (
    <div className="md">
      <ReactMarkdown remarkPlugins={[remarkGfm]}>{children}</ReactMarkdown>
    </div>
  )
}

export function CopyButton({ text, label, className = 'btn sm' }: { text: string; label?: string; className?: string }) {
  const { t } = useT()
  const toast = useToast()
  return (
    <button
      className={className}
      onClick={async () => {
        try {
          await navigator.clipboard.writeText(text)
          toast(t.common.copied)
        } catch {
          const ta = document.createElement('textarea')
          ta.value = text
          document.body.appendChild(ta)
          ta.select()
          document.execCommand('copy')
          ta.remove()
          toast(t.common.copied)
        }
      }}
    >
      ⧉ {label ?? t.common.copy}
    </button>
  )
}

export function CopyBox({ text }: { text: string }) {
  return (
    <div className="copybox">
      <pre>{text}</pre>
      <CopyButton text={text} />
    </div>
  )
}

export function Switch({ on, onChange, disabled }: { on: boolean; onChange: (v: boolean) => void; disabled?: boolean }) {
  return <button className={'switch' + (on ? ' on' : '')} disabled={disabled} onClick={() => onChange(!on)} aria-pressed={on} />
}

export function Tabs<T extends string>({ tabs, value, onChange }: { tabs: { id: T; label: string }[]; value: T; onChange: (v: T) => void }) {
  return (
    <div className="tabs">
      {tabs.map((tb) => (
        <button key={tb.id} className={'tab' + (tb.id === value ? ' active' : '')} onClick={() => onChange(tb.id)}>{tb.label}</button>
      ))}
    </div>
  )
}

export function Empty({ children }: { children: ReactNode }) {
  return <div className="empty">{children}</div>
}

/** Small async-data hook with manual reload. */
export function useAsync<T>(fn: () => Promise<T>, deps: unknown[] = []) {
  const [data, setData] = useState<T | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)
  const [tick, setTick] = useState(0)
  const reload = useCallback(() => setTick((x) => x + 1), [])
  useEffect(() => {
    let alive = true
    setLoading(true)
    fn().then(
      (d) => { if (alive) { setData(d); setError(null); setLoading(false) } },
      (e) => { if (alive) { setError(String(e?.message ?? e)); setLoading(false) } },
    )
    return () => { alive = false }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...deps, tick])
  return useMemo(() => ({ data, error, loading, reload, setData }), [data, error, loading, reload])
}

export function useDebounced<T>(value: T, ms = 250): T {
  const [v, setV] = useState(value)
  useEffect(() => {
    const h = setTimeout(() => setV(value), ms)
    return () => clearTimeout(h)
  }, [value, ms])
  return v
}
