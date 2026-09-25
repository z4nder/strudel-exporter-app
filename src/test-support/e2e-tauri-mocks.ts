import { mockIPC } from '@tauri-apps/api/mocks'

declare global {
  interface Window {
    __STRUDEL_E2E__: {
      saveCalls: number
      savedBytes: number
    }
  }
}

window.__STRUDEL_E2E__ = {
  saveCalls: 0,
  savedBytes: 0,
}

mockIPC(async (command, payload) => {
  if (command === 'plugin:dialog|open') {
    return '/source/forest.strudel'
  }

  if (command === 'read_strudel_file') {
    const response = await fetch('/source/forest.strudel')
    if (!response.ok) throw new Error(`Fixture indisponível: HTTP ${response.status}`)
    return response.text()
  }

  if (command === 'plugin:dialog|save') {
    return '/tmp/forest.wav'
  }

  if (command === 'save_wav_bytes') {
    const bytes = (payload?.bytes ?? []) as number[]
    window.__STRUDEL_E2E__.saveCalls += 1
    window.__STRUDEL_E2E__.savedBytes = bytes.length

    const blob = new Blob([new Uint8Array(bytes)], { type: 'audio/wav' })
    const url = URL.createObjectURL(blob)
    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = 'forest.wav'
    anchor.click()
    URL.revokeObjectURL(url)
    return null
  }

  return null
})

export {}
