// One transport for two hosts: inside Tauri we call `invoke("rpc", …)`, in the
// browser (memhub serve) we POST /api/<cmd>. Both go through core::api::dispatch.

export type Kind =
  | 'instruction' | 'memory' | 'daily-log' | 'session-summary' | 'profile' | 'note' | 'knowledge'

export const KINDS: Kind[] = ['instruction', 'memory', 'daily-log', 'session-summary', 'profile', 'note', 'knowledge']

export interface EntrySummary {
  id: string; agent: string; source: string; project: string; project_path?: string | null
  kind: Kind; title: string; vault_path: string; origin?: string | null
  created: string; updated: string; tags: string[]; archived: boolean; native: boolean
  size: number; snippet?: string
}
export interface Frontmatter extends Omit<EntrySummary, 'vault_path' | 'native' | 'size' | 'snippet'> {
  origin_hash?: string | null; redacted: boolean; sources: string[]; task?: string | null; template?: string | null
}
export interface EntryFull extends EntrySummary { frontmatter: Frontmatter; body: string }

export interface Stats {
  total: number; archived: number; by_agent: [string, number][]; by_kind: [string, number][]
  by_project: [string, number][]; last_updated?: string | null; vault_bytes: number
}
export interface TreeProject { project: string; project_path?: string | null; count: number }
export interface TreeNode { agent: string; count: number; projects: TreeProject[] }
export interface SyncReport {
  at: string; duration_ms: number; scanned: number; added: number; updated: number
  archived: number; unchanged: number; errors: string[]; git_commit: boolean
}
export interface DetectedSource {
  type: string; name: string; label: string; description: string; root?: string | null
  installed: boolean; enabled: boolean; configured: boolean; builtin: boolean
}
export interface SourceConfig {
  type: string; name?: string | null; root?: string | null; enabled: boolean
  include: string[]; exclude: string[]; kind?: Kind | null
}
export interface ProjectConfig { path: string; name?: string | null }
export interface Config {
  vault: string; language: string; git_snapshot: boolean; max_file_size_kb: number
  redact_secrets: boolean; task_context_budget_kb: number; sources: SourceConfig[]; projects: ProjectConfig[]
}
export interface Paths { home: string; config: string; vault: string; vault_display: string; index: string; tasks: string; templates: string }
export interface Overview {
  version: string; stats: Stats; sources: DetectedSource[]; last_sync?: SyncReport | null
  recent: EntrySummary[]; pending_tasks: number; paths: Paths
}
export interface TemplateInfo { id: string; name: string; description: string; path: string; builtin: boolean }
export interface TaskScope {
  agents: string[]; projects: string[]; kinds: string[]; query?: string | null; since?: string | null
  limit?: number | null; include_knowledge: boolean; include_archived: boolean
}
export interface TaskMeta {
  id: string; template: string; title: string; status: 'pending' | 'done' | 'accepted'
  created: string; updated: string; entry_ids: string[]; inlined: number; task_bytes: number
  scope: TaskScope; knowledge_id?: string | null; language: string
}
export interface TaskDetail extends TaskMeta { task_md: string; result_md?: string | null; entries: EntrySummary[]; dir: string }
export interface Snippet { id: string; label: string; file: string; language: string; snippet: string; hint: string }

export interface EntryFilter {
  agent?: string; project?: string; kind?: Kind; tag?: string; since?: string
  include_archived?: boolean; limit?: number; offset?: number
}

declare global {
  interface Window { __TAURI_INTERNALS__?: unknown }
}

export const isTauri = () => typeof window !== 'undefined' && !!window.__TAURI_INTERNALS__

import { invoke as tauriInvoke } from '@tauri-apps/api/core'

export class ApiError extends Error {}

export async function call<T = unknown>(cmd: string, params: Record<string, unknown> = {}): Promise<T> {
  if (isTauri()) {
    try {
      return (await tauriInvoke('rpc', { cmd, params })) as T
    } catch (e) {
      throw new ApiError(typeof e === 'string' ? e : (e as Error)?.message ?? String(e))
    }
  }
  const res = await fetch(`/api/${cmd}`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(params),
  })
  const data = await res.json().catch(() => ({}))
  if (!res.ok) throw new ApiError((data as { error?: string }).error ?? `HTTP ${res.status}`)
  return data as T
}

export const api = {
  overview: () => call<Overview>('get_overview'),
  config: () => call<Config>('get_config'),
  saveConfig: (config: Config) => call<{ ok: boolean; restart_required: boolean }>('save_config', { config }),
  sources: () => call<DetectedSource[]>('list_sources'),
  addSource: (p: { type: string; name?: string; root?: string; include?: string[]; exclude?: string[]; kind?: Kind }) =>
    call<SyncReport>('add_source', p),
  setSourceEnabled: (type: string, enabled: boolean, name?: string) =>
    call<SyncReport>('set_source_enabled', { type, enabled, name }),
  removeSource: (name: string) => call<SyncReport>('remove_source', { name }),
  addProject: (path: string, name?: string) => call<SyncReport>('add_project', { path, name }),
  removeProject: (path: string) => call<SyncReport>('remove_project', { path }),
  sync: () => call<SyncReport>('sync_now'),
  reindex: () => call<{ indexed: number }>('reindex'),
  tree: () => call<TreeNode[]>('get_tree'),
  list: (f: EntryFilter) => call<EntrySummary[]>('list_entries', f as Record<string, unknown>),
  search: (q: string, f: EntryFilter) => call<EntrySummary[]>('search_entries', { q, ...f }),
  entry: (id: string) => call<EntryFull>('get_entry', { id }),
  updateEntry: (id: string, p: { body?: string; title?: string; tags?: string[] }) =>
    call<EntrySummary>('update_entry', { id, ...p }),
  deleteEntry: (id: string) => call<{ ok: boolean }>('delete_entry', { id }),
  createNote: (p: { agent: string; title: string; content: string; tags?: string[]; project?: string }) =>
    call<EntrySummary>('create_note', p),
  templates: () => call<TemplateInfo[]>('list_templates'),
  createTask: (template: string, scope: TaskScope, title?: string) =>
    call<TaskMeta>('create_task', { template, scope, title }),
  tasks: () => call<TaskMeta[]>('list_tasks'),
  task: (id: string) => call<TaskDetail>('get_task', { id }),
  submitResult: (id: string, result: string) => call<TaskMeta>('submit_task_result', { id, result }),
  acceptTask: (id: string) => call<EntrySummary>('accept_task', { id }),
  deleteTask: (id: string) => call<{ ok: boolean }>('delete_task', { id }),
  snippets: (bin?: string) => call<Snippet[]>('get_snippets', { bin }),
  /** Desktop only: absolute path of the bundled `memhub` CLI (empty string in browser mode). */
  sidecarPath: async (): Promise<string> => (isTauri() ? ((await tauriInvoke('sidecar_path')) as string) : ''),
  paths: () => call<Paths>('get_paths'),
}
