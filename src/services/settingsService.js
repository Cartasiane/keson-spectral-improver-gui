import { invoke } from '@tauri-apps/api/core'
import { isDesktop } from './scanService'

// Small helper around Tauri settings calls so components stay lean.

export async function fetchSettings() {
  if (!isDesktop) return null
  return invoke('get_settings')
}

export async function persistSettings(settings) {
  if (!isDesktop) return null
  return invoke('save_settings', { settings })
}
