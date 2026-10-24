import { getAppSetting } from '../../shared/api'

// The backend swaps the whole database, so every cached query and every setting held in component state belongs to the other
// database afterwards. Reloading the window is the only way to drop all of it.
// localStorage holds the theme for the first paint after the reload, so it is refreshed from the new database first. A failure
// there must not stop the reload, because the swap has already happened.
export const reloadAfterDbSwap = async () => {
  try {
    localStorage.setItem('theme', (await getAppSetting('theme')) ?? 'system')
  } catch (err) {
    console.error('Failed to cache the theme of the new database:', err)
  }
  window.location.reload()
}
