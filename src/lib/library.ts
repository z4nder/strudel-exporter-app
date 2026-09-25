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

export interface LibraryTag {
  id: number
  name: string
  color: string
  trackCount: number
}

export interface LibraryTrack {
  id: number
  name: string
  sourcePath: string
  settings: SavedTrackSettings
  tags: LibraryTag[]
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

export function listTags(): Promise<LibraryTag[]> {
  return invoke('library_list_tags')
}

export function createTag(name: string, color: string): Promise<LibraryTag> {
  return invoke('library_create_tag', { name, color })
}

export function updateTag(tagId: number, name: string, color: string): Promise<LibraryTag> {
  return invoke('library_update_tag', { tagId, name, color })
}

export function deleteTag(tagId: number): Promise<void> {
  return invoke('library_delete_tag', { tagId })
}

export function setTrackTag(trackId: number, tagId: number, attached: boolean): Promise<LibraryTrack> {
  return invoke('library_set_track_tag', { trackId, tagId, attached })
}
