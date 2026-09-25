import { mockIPC } from '@tauri-apps/api/mocks'
import { analyzeStrudel, exportToWav, renderToUrl, type RenderSettings } from '../../src/lib/strudel-runner'

const automaticSettings: RenderSettings = {
  startCycle: 0,
  endCycle: 8,
  sampleRate: 44100,
  maxPolyphony: 32,
}

interface WavSummary {
  bytes: number
  channels: number
  sampleRate: number
  durationSeconds: number
  peakSample: number
  riff: string
  wave: string
  format: string
  data: string
}

declare global {
  interface Window {
    strudelFeature: {
      analyze(): Promise<{ cps: number; cycleDuration: number; minLoopCycles: number }>
      renderPreview(): Promise<WavSummary>
      renderManualPreview(): Promise<WavSummary>
      exportWav(): Promise<{ summary: WavSummary; bytes: number[]; saveCalls: number }>
    }
  }
}

let savedBytes = new Uint8Array()
let saveCalls = 0

mockIPC(async (command, payload) => {
  if (command === 'plugin:dialog|save') return '/tmp/forest.wav'

  if (command === 'save_looped_wav') {
    const base = new Uint8Array((payload?.baseBytes ?? []) as number[])
    const loops = Number(payload?.loops ?? 1)
    const dataSize = new DataView(base.buffer, base.byteOffset, base.byteLength).getUint32(40, true)
    savedBytes = new Uint8Array(44 + dataSize * loops)
    savedBytes.set(base.subarray(0, 44))
    const outputView = new DataView(savedBytes.buffer)
    outputView.setUint32(4, 36 + dataSize * loops, true)
    outputView.setUint32(40, dataSize * loops, true)
    for (let loop = 0; loop < loops; loop++) {
      savedBytes.set(base.subarray(44, 44 + dataSize), 44 + loop * dataSize)
    }
    saveCalls += 1
    return null
  }

  return null
})

async function loadFixture(): Promise<string> {
  const response = await fetch('/source/forest.strudel')
  if (!response.ok) throw new Error(`Fixture indisponível: HTTP ${response.status}`)
  return response.text()
}

function readAscii(view: DataView, offset: number, size: number): string {
  return Array.from({ length: size }, (_, index) => String.fromCharCode(view.getUint8(offset + index))).join('')
}

function summarizeWav(buffer: ArrayBuffer): WavSummary {
  const view = new DataView(buffer)
  const channels = view.getUint16(22, true)
  const sampleRate = view.getUint32(24, true)
  const byteRate = view.getUint32(28, true)
  const dataSize = view.getUint32(40, true)
  let peakSample = 0

  for (let offset = 44; offset + 1 < buffer.byteLength; offset += 2) {
    peakSample = Math.max(peakSample, Math.abs(view.getInt16(offset, true)))
  }

  return {
    bytes: buffer.byteLength,
    channels,
    sampleRate,
    durationSeconds: dataSize / byteRate,
    peakSample,
    riff: readAscii(view, 0, 4),
    wave: readAscii(view, 8, 4),
    format: readAscii(view, 12, 4),
    data: readAscii(view, 36, 4),
  }
}

window.strudelFeature = {
  async analyze() {
    return analyzeStrudel(await loadFixture())
  },

  async renderPreview() {
    const code = await loadFixture()
    const url = await renderToUrl(code, 1, automaticSettings)
    try {
      const wav = await fetch(url).then((response) => response.arrayBuffer())
      return summarizeWav(wav)
    } finally {
      URL.revokeObjectURL(url)
    }
  },

  async renderManualPreview() {
    const code = await loadFixture()
    const url = await renderToUrl(code, 1, {
      startCycle: 2,
      endCycle: 3,
      sampleRate: 48000,
      maxPolyphony: 16,
    })
    try {
      const wav = await fetch(url).then((response) => response.arrayBuffer())
      return summarizeWav(wav)
    } finally {
      URL.revokeObjectURL(url)
    }
  },

  async exportWav() {
    savedBytes = new Uint8Array()
    saveCalls = 0
    const code = await loadFixture()
    await exportToWav(code, 1, 'forest', automaticSettings)
    const buffer = savedBytes.buffer.slice(
      savedBytes.byteOffset,
      savedBytes.byteOffset + savedBytes.byteLength,
    ) as ArrayBuffer
    return {
      summary: summarizeWav(buffer),
      bytes: Array.from(savedBytes),
      saveCalls,
    }
  },
}
