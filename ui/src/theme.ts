export type Theme = 'system' | 'light' | 'dark'
const KEY = 'memhub.theme'

/** `?theme=dark|light` in the URL wins (handy for screenshots/embedding), then the saved preference. */
export function getTheme(): Theme {
  const q = new URLSearchParams(location.search).get('theme')
  if (q === 'light' || q === 'dark') return q
  const s = localStorage.getItem(KEY)
  return s === 'light' || s === 'dark' ? s : 'system'
}

export function applyTheme(t: Theme) {
  if (t === 'system') document.documentElement.removeAttribute('data-theme')
  else document.documentElement.setAttribute('data-theme', t)
}

export function setTheme(t: Theme) {
  localStorage.setItem(KEY, t)
  applyTheme(t)
}
