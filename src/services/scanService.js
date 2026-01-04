import { invoke, convertFileSrc } from '@tauri-apps/api/tauri'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/api/dialog'

export const isDesktop = typeof window !== 'undefined' && !!window.__TAURI__

export async function pickFolderDialog() {
  if (!isDesktop) return null
  const choice = await open({ directory: true })
  return typeof choice === 'string' ? choice : null
}

export async function scanFolder(folder, minKbps = 256) {
  if (!isDesktop) throw new Error('Scan disponible seulement en mode desktop')
  return invoke('scan_folder', { folder, minKbps })
}

export async function revealInFolder(path) {
  if (!isDesktop) return
  return invoke('reveal_in_folder', { path })
}

export async function openSpectrum(path) {
  if (!isDesktop) throw new Error('Spectre disponible seulement en mode desktop')
  const imgPath = await invoke('open_spectrum', { path })
  return convertFileSrc(imgPath)
}

export async function redownloadBad(paths) {
  if (!isDesktop) throw new Error('Disponible seulement en desktop')
  return invoke('redownload_bad', { paths })
}

export async function acceptRedownload(original, fresh) {
  if (!isDesktop) throw new Error('Disponible seulement en desktop')
  return invoke('accept_redownload', { original, new_path: fresh })
}

export async function discardFile(path) {
  if (!isDesktop) return null
  return invoke('discard_file', { path })
}

export async function listenScanProgress(callback) {
  if (!isDesktop) return null
  const unlisten = await listen('scan_progress', (event) => {
    callback(event?.payload)
  })
  return unlisten
}
