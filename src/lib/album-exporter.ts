import { invoke } from '@tauri-apps/api/core'
import { save } from '@tauri-apps/plugin-dialog'
import { analyzeStrudel, renderToUrl, type RenderProgress } from './strudel-runner'
import type { LibraryAlbum } from './library'

function throwIfAborted(signal?: AbortSignal) {
  if (signal?.aborted) throw new DOMException('Geração cancelada', 'AbortError')
}

function safeFileName(name: string) {
  return name.trim().replace(/[\\/:*?"<>|]/g, '-').replace(/\s+/g, ' ') || 'album'
}

function wavDurationSeconds(bytes: Uint8Array) {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  const chunkName = (offset: number) => String.fromCharCode(...bytes.subarray(offset, offset + 4))
  let sampleRate = 0
  let blockAlign = 0
  let dataSize = 0
  for (let offset = 12; offset + 8 <= bytes.length;) {
    const name = chunkName(offset)
    const size = view.getUint32(offset + 4, true)
    if (name === 'fmt ' && size >= 16) {
      sampleRate = view.getUint32(offset + 12, true)
      blockAlign = view.getUint16(offset + 20, true)
    } else if (name === 'data') {
      dataSize = size
      break
    }
    offset += 8 + size + (size % 2)
  }
  if (!sampleRate || !blockAlign || !dataSize) throw new Error('WAV renderizado possui cabeçalho inválido')
  return dataSize / blockAlign / sampleRate
}

export async function exportAlbumToWav(
  album: LibraryAlbum,
  onProgress?: RenderProgress,
  signal?: AbortSignal,
) {
  if (album.tracks.length === 0) throw new Error('Adicione pelo menos uma Track ao Album')
  const path = await save({
    defaultPath: `${safeFileName(album.name)}.wav`,
    filters: [{ name: 'WAV audio', extensions: ['wav'] }],
  })
  if (!path) return
  throwIfAborted(signal)

  const segments: Array<{ baseBytes: number[]; loops: number; gapSeconds: number }> = []
  const manifestTracks: Array<Record<string, unknown>> = []
  let timelineSeconds = 0

  for (let index = 0; index < album.tracks.length; index++) {
    const item = album.tracks[index]
    onProgress?.(index / album.tracks.length * 0.9, `Renderizando ${index + 1}/${album.tracks.length}: ${item.track.name}`)
    const code = await invoke<string>('read_strudel_file', { path: item.track.sourcePath })
    const meta = await analyzeStrudel(code)
    throwIfAborted(signal)
    const url = await renderToUrl(
      code,
      1,
      {
        startCycle: item.settings.startCycle,
        endCycle: item.settings.endCycle,
        sampleRate: album.sampleRate,
        maxPolyphony: item.settings.maxPolyphony,
      },
      (progress, label) => onProgress?.(
        ((index + progress) / album.tracks.length) * 0.9,
        `${item.track.name}: ${label}`,
      ),
      signal,
    )
    try {
      const wavBytes = new Uint8Array(await fetch(url).then((response) => response.arrayBuffer()))
      const baseBytes = Array.from(wavBytes)
      const durationSeconds = wavDurationSeconds(wavBytes) * item.settings.loops
      const gapAfterSeconds = Math.round(item.settings.gapAfterSeconds * album.sampleRate) / album.sampleRate
      const startsAtSeconds = timelineSeconds
      const endsAtSeconds = startsAtSeconds + durationSeconds
      manifestTracks.push({
        position: index + 1,
        name: item.track.name,
        sourcePath: item.track.sourcePath,
        startsAtSeconds,
        endsAtSeconds,
        durationSeconds,
        gapAfterSeconds,
        tags: item.track.tags.map((tag) => tag.name),
        render: {
          startCycle: item.settings.startCycle,
          endCycle: item.settings.endCycle,
          loops: item.settings.loops,
          maxPolyphony: item.settings.maxPolyphony,
          cps: meta.cps,
        },
      })
      timelineSeconds = endsAtSeconds + gapAfterSeconds
      segments.push({
        baseBytes,
        loops: item.settings.loops,
        gapSeconds: item.settings.gapAfterSeconds,
      })
    } finally {
      URL.revokeObjectURL(url)
    }
  }

  throwIfAborted(signal)
  onProgress?.(0.94, 'Montando WAV e manifesto no Rust…')
  const manifestJson = JSON.stringify({
    album: {
      name: album.name,
      description: album.description,
      coverPath: album.coverPath,
      sampleRate: album.sampleRate,
      durationSeconds: timelineSeconds,
      tags: album.tags.map((tag) => ({ name: tag.name, color: tag.color })),
    },
    tracks: manifestTracks,
  }, null, 2)
  await invoke('save_album_wav', { segments, path, manifestJson })
  throwIfAborted(signal)
  onProgress?.(1, 'Album exportado')
}
