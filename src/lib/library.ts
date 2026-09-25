import { invoke } from '@tauri-apps/api/core'

export type RangeMode = 'automatic' | 'manual'

export interface SavedTrackSettings {
  rangeMode: RangeMode
  startCycle: number
  endCycle: number
  loops: number
  sampleRate: 44100 | 48000 | 96000
  maxPolyphony: number
  exportFileName: string
}

export interface LibraryTrack {
  id: number
  name: string
  sourcePath: string
  settings: SavedTrackSettings
  createdAt: number
  updatedAt: number
}

export function listTracks(): Promise<LibraryTrack[]> {
  return invoke('library_list_tracks')
}

export function importTrack(path: string): Promise<LibraryTrack> {
  return invoke('library_import_track', { path })
}

export function saveTrackSettings(
  trackId: number,
  settings: SavedTrackSettings,
): Promise<LibraryTrack> {
  return invoke('library_save_track_settings', { trackId, settings })
}

export function deleteTrack(trackId: number): Promise<void> {
  return invoke('library_delete_track', { trackId })
}
