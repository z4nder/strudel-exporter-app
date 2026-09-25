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

export interface AlbumSummary {
  id: number
  name: string
  description: string
  coverPath: string | null
  sampleRate: 44100 | 48000 | 96000
  defaultGapSeconds: number
  trackCount: number
  tags: LibraryTag[]
  createdAt: number
  updatedAt: number
}

export interface AlbumTrackSettings {
  rangeMode: RangeMode
  startCycle: number
  endCycle: number
  loops: number
  maxPolyphony: number
  gapAfterSeconds: number
}

export interface AlbumTrackItem {
  id: number
  position: number
  track: LibraryTrack
  settings: AlbumTrackSettings
}

export interface LibraryAlbum extends AlbumSummary {
  tracks: AlbumTrackItem[]
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

export function listAlbums(): Promise<AlbumSummary[]> {
  return invoke('library_list_albums')
}

export function getAlbum(albumId: number): Promise<LibraryAlbum> {
  return invoke('library_get_album', { albumId })
}

export interface AlbumFields {
  name: string
  description: string
  coverPath: string | null
  sampleRate: 44100 | 48000 | 96000
  defaultGapSeconds: number
}

export function createAlbum(fields: AlbumFields): Promise<LibraryAlbum> {
  return invoke('library_create_album', fields)
}

export function updateAlbum(albumId: number, fields: AlbumFields): Promise<LibraryAlbum> {
  return invoke('library_update_album', { albumId, ...fields })
}

export function deleteAlbum(albumId: number): Promise<void> {
  return invoke('library_delete_album', { albumId })
}

export function addAlbumTrack(albumId: number, trackId: number): Promise<LibraryAlbum> {
  return invoke('library_add_album_track', { albumId, trackId })
}

export function updateAlbumTrack(itemId: number, settings: AlbumTrackSettings): Promise<LibraryAlbum> {
  return invoke('library_update_album_track', { itemId, settings })
}

export function removeAlbumTrack(itemId: number): Promise<LibraryAlbum> {
  return invoke('library_remove_album_track', { itemId })
}

export function reorderAlbumTracks(albumId: number, itemIds: number[]): Promise<LibraryAlbum> {
  return invoke('library_reorder_album_tracks', { albumId, itemIds })
}
