import { createContext, useContext } from 'react'

const zh = {
  app: 'MemHub',
  tagline: 'Agent 记忆中枢',
  nav: { overview: '概览', memories: '记忆库', knowledge: '知识', tasks: '总结任务', sources: '来源', settings: '设置' },
  common: {
    loading: '加载中…', error: '出错了', save: '保存', cancel: '取消', delete: '删除', edit: '编辑', copy: '复制', copied: '已复制',
    refresh: '刷新', sync: '立即同步', syncing: '同步中…', close: '关闭', create: '创建', add: '添加', remove: '移除', all: '全部',
    none: '无', enabled: '已启用', disabled: '已停用', installed: '已检测到', notInstalled: '未检测到', search: '搜索…',
    updated: '更新', created: '创建', open: '打开', yes: '是', no: '否', back: '返回', confirmDelete: '确定删除？此操作不可撤销。',
  },
  kinds: {
    instruction: '指令', memory: '记忆', 'daily-log': '日志', 'session-summary': '会话摘要', profile: '画像', note: '笔记', knowledge: '知识',
  } as Record<string, string>,
  overview: {
    title: '概览', entries: '记忆条目', archived: '已归档', agents: '已接入 Agent', pendingTasks: '待处理任务',
    lastSync: '上次同步', never: '尚未同步', syncResult: (r: { added: number; updated: number; archived: number; scanned: number; duration_ms: number }) =>
      `扫描 ${r.scanned} · 新增 ${r.added} · 更新 ${r.updated} · 归档 ${r.archived} · ${r.duration_ms} ms`,
    byAgent: '按 Agent', byKind: '按类型', recent: '最近更新', detected: '本机检测到的 Agent', vault: '统一记忆库',
    emptyHint: '还没有记忆。到「来源」页启用 Agent 或添加目录，然后点「立即同步」。', goSources: '去配置来源',
    errors: '同步错误',
  },
  memories: {
    title: '记忆库', allAgents: '所有 Agent', noResults: '没有匹配的记忆', select: '从左侧选择一条记忆', showArchived: '显示已归档',
    mirrored: '镜像（源文件只读）', native: 'Vault 原生', origin: '来源文件', vaultPath: 'Vault 路径', redacted: '已打码',
    editWarn: '这是镜像条目：源文件下次变动时你的修改会被覆盖。', body: '正文', tags: '标签（逗号分隔）',
    entriesCount: (n: number) => `${n} 条`, newNote: '新建笔记', noteTitle: '标题', noteContent: '内容（Markdown）', noteAgent: '归属 Agent',
    sources: '来源记忆', relatedTask: '来自任务',
  },
  knowledge: {
    title: '知识', empty: '还没有知识笔记。创建一个总结任务，让你的 Agent 来提炼。', goTasks: '去创建总结任务',
  },
  tasks: {
    title: '总结任务', new: '新建任务', list: '任务列表', empty: '还没有任务', template: '模板', titleField: '标题（可选）',
    scope: '范围', agents: 'Agent', projects: '项目', kinds: '类型', query: '关键词（可选）', since: '更新时间晚于', limit: '最多条目',
    includeKnowledge: '包含已有知识笔记', preview: '将包含的条目', createTask: '生成任务包',
    status: { pending: '待执行', done: '待采纳', accepted: '已采纳' } as Record<string, string>,
    howto: '怎么执行', taskMd: 'TASK.md', result: '结果',
    howtoMcp: '方式 A · MCP：在任意已接入 MemHub 的 Agent（Claude Code / Codex / OpenClaw…）里说：',
    mcpPrompt: (id: string) => `请执行 MemHub 总结任务 ${id}：先调用 summary_task_get 获取任务，完成后用 summary_task_submit 提交结果。`,
    howtoCli: '方式 B · 命令行：直接用本机已安装的 CLI Agent 跑一遍（结果会自动写回任务目录）：',
    howtoManual: '方式 C · 手动：复制 TASK.md 到任意聊天窗口，把结果粘贴到「结果」标签页。',
    pasteResult: '把 Agent 的输出粘贴到这里…', submit: '提交结果', accept: '采纳为知识笔记', accepted: '已写入知识库',
    entries: (n: number, inlined: number) => `${n} 条记忆（${inlined} 条已内联到任务包）`, bytes: '任务包大小',
    taskDir: '任务目录', deleteTask: '删除任务', viewKnowledge: '查看知识笔记', noResultYet: '还没有结果。',
  },
  sources: {
    title: '来源', builtin: '内置适配器', generic: '通用目录', projects: '项目目录', addGeneric: '添加目录',
    addProject: '添加项目', name: '名称', root: '目录', include: '包含（glob，逗号分隔，默认 **/*.md）', kind: '类型',
    path: '路径', projectHint: '项目目录里的 CLAUDE.md / AGENTS.md / GEMINI.md / .cursor/rules / memory-bank 等会被采集。',
    genericHint: '任何目录都可以作为记忆源，适合自研 Agent。', noProjects: '还没有登记项目目录。', noGeneric: '还没有通用目录。',
    lastReport: '上次同步结果',
  },
  settings: {
    title: '设置', general: '常规', language: '界面与总结语言', gitSnapshot: 'Git 快照（每次同步后自动 commit）',
    redact: '采集时对疑似密钥打码', maxSize: '单文件上限 (KB)', budget: '任务包上下文预算 (KB)', vault: 'Vault 路径',
    vaultHint: '修改后需要重启应用。', paths: '路径', saved: '已保存', restart: '已保存，重启后生效。',
    mcp: 'MCP 接入', mcpHint: '把 MemHub 接入你的 Agent（复制对应片段）。二进制路径：', binPath: 'memhub 可执行文件路径',
    reindex: '重建索引', reindexed: (n: number) => `已重建索引：${n} 条`,
  },
}

type Dict = typeof zh

const en: Dict = {
  app: 'MemHub',
  tagline: 'Agent memory hub',
  nav: { overview: 'Overview', memories: 'Memories', knowledge: 'Knowledge', tasks: 'Summary tasks', sources: 'Sources', settings: 'Settings' },
  common: {
    loading: 'Loading…', error: 'Something went wrong', save: 'Save', cancel: 'Cancel', delete: 'Delete', edit: 'Edit', copy: 'Copy', copied: 'Copied',
    refresh: 'Refresh', sync: 'Sync now', syncing: 'Syncing…', close: 'Close', create: 'Create', add: 'Add', remove: 'Remove', all: 'All',
    none: 'None', enabled: 'Enabled', disabled: 'Disabled', installed: 'Detected', notInstalled: 'Not found', search: 'Search…',
    updated: 'Updated', created: 'Created', open: 'Open', yes: 'Yes', no: 'No', back: 'Back', confirmDelete: 'Delete? This cannot be undone.',
  },
  kinds: {
    instruction: 'instruction', memory: 'memory', 'daily-log': 'daily log', 'session-summary': 'session summary', profile: 'profile', note: 'note', knowledge: 'knowledge',
  },
  overview: {
    title: 'Overview', entries: 'Memory entries', archived: 'Archived', agents: 'Connected agents', pendingTasks: 'Open tasks',
    lastSync: 'Last sync', never: 'Never synced', syncResult: (r) =>
      `scanned ${r.scanned} · added ${r.added} · updated ${r.updated} · archived ${r.archived} · ${r.duration_ms} ms`,
    byAgent: 'By agent', byKind: 'By kind', recent: 'Recently updated', detected: 'Agents on this machine', vault: 'Unified vault',
    emptyHint: 'No memories yet. Enable an agent or add a folder on the Sources page, then Sync.', goSources: 'Configure sources',
    errors: 'Sync errors',
  },
  memories: {
    title: 'Memories', allAgents: 'All agents', noResults: 'No matching memories', select: 'Select a memory on the left', showArchived: 'Show archived',
    mirrored: 'Mirror (source is read-only)', native: 'Vault native', origin: 'Origin file', vaultPath: 'Vault path', redacted: 'Redacted',
    editWarn: 'This is a mirrored entry: your edit will be overwritten the next time the source file changes.', body: 'Body', tags: 'Tags (comma separated)',
    entriesCount: (n) => `${n} entries`, newNote: 'New note', noteTitle: 'Title', noteContent: 'Content (Markdown)', noteAgent: 'Agent',
    sources: 'Source memories', relatedTask: 'From task',
  },
  knowledge: {
    title: 'Knowledge', empty: 'No knowledge notes yet. Create a summary task and let your agent distil one.', goTasks: 'Create a summary task',
  },
  tasks: {
    title: 'Summary tasks', new: 'New task', list: 'Tasks', empty: 'No tasks yet', template: 'Template', titleField: 'Title (optional)',
    scope: 'Scope', agents: 'Agents', projects: 'Projects', kinds: 'Kinds', query: 'Keywords (optional)', since: 'Updated since', limit: 'Max entries',
    includeKnowledge: 'Include existing knowledge notes', preview: 'Entries that will be included', createTask: 'Build task pack',
    status: { pending: 'Pending', done: 'Review', accepted: 'Accepted' },
    howto: 'How to run', taskMd: 'TASK.md', result: 'Result',
    howtoMcp: 'Option A · MCP: in any agent connected to MemHub (Claude Code / Codex / OpenClaw…) say:',
    mcpPrompt: (id) => `Run MemHub summary task ${id}: call summary_task_get to fetch it, then submit with summary_task_submit.`,
    howtoCli: 'Option B · CLI: run it with a locally installed CLI agent (the result lands in the task directory):',
    howtoManual: 'Option C · Manual: copy TASK.md into any chat, paste the answer in the Result tab.',
    pasteResult: 'Paste the agent output here…', submit: 'Submit result', accept: 'Accept as knowledge note', accepted: 'Saved to knowledge',
    entries: (n, inlined) => `${n} memories (${inlined} inlined into the pack)`, bytes: 'Pack size',
    taskDir: 'Task directory', deleteTask: 'Delete task', viewKnowledge: 'View knowledge note', noResultYet: 'No result yet.',
  },
  sources: {
    title: 'Sources', builtin: 'Built-in adapters', generic: 'Generic folders', projects: 'Project directories', addGeneric: 'Add folder',
    addProject: 'Add project', name: 'Name', root: 'Directory', include: 'Include (globs, comma separated, default **/*.md)', kind: 'Kind',
    path: 'Path', projectHint: 'CLAUDE.md / AGENTS.md / GEMINI.md / .cursor/rules / memory-bank … inside the project are collected.',
    genericHint: 'Any directory can be a memory source — ideal for your own agents.', noProjects: 'No project directories yet.', noGeneric: 'No generic folders yet.',
    lastReport: 'Last sync',
  },
  settings: {
    title: 'Settings', general: 'General', language: 'UI & summary language', gitSnapshot: 'Git snapshots (auto-commit after each sync)',
    redact: 'Redact likely secrets while mirroring', maxSize: 'Max file size (KB)', budget: 'Task context budget (KB)', vault: 'Vault path',
    vaultHint: 'Requires a restart after changing.', paths: 'Paths', saved: 'Saved', restart: 'Saved. Restart to apply.',
    mcp: 'Connect via MCP', mcpHint: 'Connect MemHub to your agents (copy the matching snippet). Binary path:', binPath: 'Path of the memhub executable',
    reindex: 'Rebuild index', reindexed: (n) => `Index rebuilt: ${n} entries`,
  },
}

export const dicts: Record<string, Dict> = { 'zh-CN': zh, en }
export type Lang = keyof typeof dicts

export const I18nContext = createContext<{ t: Dict; lang: string; setLang: (l: string) => void }>({
  t: zh, lang: 'zh-CN', setLang: () => {},
})
export const useT = () => useContext(I18nContext)

export function normalizeLang(l?: string | null): string {
  if (!l) return 'zh-CN'
  if (l.startsWith('zh')) return 'zh-CN'
  return 'en'
}
