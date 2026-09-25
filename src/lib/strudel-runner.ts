import { webaudioRepl } from '@strudel/webaudio'
import { evalScope } from '@strudel/core'
import * as strudelCore from '@strudel/core'
import * as strudelMini from '@strudel/mini'
import * as strudelWebaudio from '@strudel/webaudio'
import type { StrudelMeta } from './loop-parser'

let initialized = false
let _repl: ReturnType<typeof webaudioRepl> | null = null
let cachedComposition: { code: string; pattern: any; meta: StrudelMeta } | null = null

export type RenderProgress = (progress: number, label: string) => void

function throwIfAborted(signal?: AbortSignal) {
  if (signal?.aborted) throw new DOMException('Geração cancelada', 'AbortError')
}

async function ensureInit() {
  if (initialized) return
  console.log('[strudel] ensureInit: setting up official Strudel modules...')
  // Keep core, mini and webaudio on the same module graph. Importing prebake
  // from @strudel/repl brings a second bundled core instance, so samples get
  // registered in a different audio registry and the rendered WAV is silent.
  await evalScope(
    strudelCore,
    strudelMini,
    strudelWebaudio,
  )
  strudelMini.miniAllStrings()

  const samplesBase = 'https://raw.githubusercontent.com/felixroos/dough-samples/main'
  const strudelKit = 'https://raw.githubusercontent.com/tidalcycles/uzu-drumkit/main'
  await Promise.all([
    strudelWebaudio.registerSynthSounds(),
    strudelWebaudio.registerZZFXSounds(),
    strudelWebaudio.samples(`${samplesBase}/tidal-drum-machines.json`),
    strudelWebaudio.samples(`${samplesBase}/piano.json`),
    strudelWebaudio.samples(`${samplesBase}/Dirt-Samples.json`),
    strudelWebaudio.samples(`${samplesBase}/vcsl.json`),
    strudelWebaudio.samples(`${samplesBase}/mridangam.json`),
    strudelWebaudio.samples(`${strudelKit}/strudel.json`),
  ])
  initialized = true
  console.log('[strudel] ensureInit done')
}

/**
 * Transpiler passthrough that:
 *  1. Adds `return` before the last expression statement so the block body
 *     returns the Pattern (required when webaudioRepl wraps code in `{...}`)
 *  2. Returning any object triggers wrapExpression=true in Bn, giving us
 *     the async block form `(async () => { CODE })()` — which is the only
 *     form that supports multi-line code + `await` on any line in
 *     JavaScriptCore / WebKit2GTK.
 */
function strudelTranspiler(code: string): { output: string } {
  console.log('[strudel] transpiler input:', JSON.stringify(code.slice(0, 120)))

  // If code already has an explicit return, pass through
  if (/\breturn\s/.test(code)) {
    console.log('[strudel] transpiler: already has return, passthrough')
    return { output: code }
  }

  const lines = code.split('\n')

  // Walk backwards skipping blank / comment-only lines
  let lastMeaningfulLine = lines.length - 1
  while (lastMeaningfulLine >= 0) {
    const t = lines[lastMeaningfulLine].trim()
    if (t && !t.startsWith('//') && !t.startsWith('*') && !t.startsWith('/*') && !t.startsWith('*/')) break
    lastMeaningfulLine--
  }

  if (lastMeaningfulLine < 0) {
    console.log('[strudel] transpiler: no meaningful lines, passthrough')
    return { output: code }
  }

  // Walk up from lastMeaningfulLine while the line looks like a method-chain continuation
  let stmtStart = lastMeaningfulLine
  while (stmtStart > 0) {
    const curr = lines[stmtStart].trim()
    const prev = lines[stmtStart - 1].trimEnd()
    // Current line is a continuation if it starts with . ) ] +
    if (/^[.)\]+(|]/.test(curr)) { stmtStart--; continue }
    // Previous line ends with ( or , — mid-expression line break
    if (/[,(]$/.test(prev)) { stmtStart--; continue }
    break
  }

  // Skip adding return to declaration / control-flow lines
  const startTrim = lines[stmtStart].trim()
  const skipRe = /^(const|let|var|function|class|import|export|if|else|for|while|do|switch|try|catch|finally|throw|break|continue|debugger)\b/
  if (!skipRe.test(startTrim)) {
    const indent = lines[stmtStart].match(/^(\s*)/)?.[1] ?? ''
    lines[stmtStart] = indent + 'return ' + startTrim
    console.log('[strudel] transpiler: added return at line', stmtStart, ':', lines[stmtStart].slice(0, 80))
  } else {
    console.log('[strudel] transpiler: last stmt is declaration/flow, skipping return:', startTrim.slice(0, 60))
  }

  const output = lines.join('\n')
  console.log('[strudel] transpiler output:', JSON.stringify(output.slice(0, 200)))
  return { output }
}

async function evalPattern(code: string): Promise<any> {
  console.log('[strudel] evalPattern start')

  if (_repl) {
    try {
      console.log('[strudel] stopping previous repl')
      _repl.scheduler.stop()
    } catch (e) {
      console.warn('[strudel] stop error:', e)
    }
  }

  console.log('[strudel] creating webaudioRepl with custom transpiler...')
  _repl = webaudioRepl({ transpiler: strudelTranspiler } as any)
  console.log('[strudel] repl created')

  console.log('[strudel] calling repl.evaluate...')
  // Do not start the live scheduler: this workflow only renders offline.
  const result = await _repl.evaluate(code, false)
  console.log('[strudel] evaluate returned:', typeof result, result)

  const pattern = (_repl.scheduler as any).pattern
  console.log('[strudel] scheduler.pattern:', typeof pattern, pattern)

  // Log first event to verify sound names are correct (not mini-notation strings)
  try {
    const events = pattern?.queryArc(0, 1)
    const firstOnset = events?.find((e: any) => e.hasOnset?.())
    if (firstOnset) {
      console.log('[strudel] first event value:', JSON.stringify(firstOnset.value).slice(0, 120))
    }
  } catch (e) {
    console.warn('[strudel] queryArc debug failed:', e)
  }

  try { _repl.scheduler.stop() } catch (e) {
    console.warn('[strudel] stop after eval error:', e)
  }

  if (!pattern) {
    // Try result directly (evaluate might return the Pattern)
    if (result && typeof result.queryArc === 'function') {
      console.log('[strudel] using result directly as pattern')
      return result
    }
    throw new Error('Não foi possível obter o padrão. Verifique o log. (scheduler.pattern = undefined, evaluate returned: ' + typeof result + ')')
  }

  return pattern
}

function stableValue(value: unknown, seen = new WeakSet<object>()): unknown {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return value
  if (typeof value === 'number') return Number.isFinite(value) ? value : String(value)
  if (typeof value === 'bigint') return value.toString()
  if (typeof value === 'undefined' || typeof value === 'function' || typeof value === 'symbol') return undefined
  if (Array.isArray(value)) return value.map((item) => stableValue(item, seen))
  if (typeof value === 'object') {
    if (seen.has(value)) return '[circular]'
    seen.add(value)
    const normalized: Record<string, unknown> = {}
    for (const key of Object.keys(value).sort()) {
      const item = stableValue((value as Record<string, unknown>)[key], seen)
      if (item !== undefined) normalized[key] = item
    }
    seen.delete(value)
    return normalized
  }
  return String(value)
}

function cycleSignature(pattern: any, cycle: number, cps: number): string {
  const round = (value: number) => Math.round(value * 1_000_000_000) / 1_000_000_000
  const events = pattern
    .queryArc(cycle, cycle + 1, { _cps: cps })
    .filter((event: any) => event.hasOnset())
    .map((event: any) => ({
      begin: round(event.whole.begin.valueOf() - cycle),
      duration: round(event.duration.valueOf()),
      value: stableValue(event.value),
    }))

  return JSON.stringify(events)
}

function detectLoopCycles(pattern: any, cps: number, maxPeriod = 32, repetitions = 3): number {
  const signatures = Array.from(
    { length: maxPeriod * repetitions },
    (_, cycle) => cycleSignature(pattern, cycle, cps),
  )

  for (let period = 1; period <= maxPeriod; period++) {
    let repeats = true
    for (let repetition = 1; repetition < repetitions && repeats; repetition++) {
      for (let offset = 0; offset < period; offset++) {
        if (signatures[offset] !== signatures[repetition * period + offset]) {
          repeats = false
          break
        }
      }
    }
    if (repeats) return period
  }

  console.warn(`[strudel] no stable loop found within ${maxPeriod} cycles; falling back to 1 cycle`)
  return 1
}

async function evaluateComposition(code: string): Promise<{ pattern: any; meta: StrudelMeta }> {
  if (cachedComposition?.code === code) return cachedComposition

  await ensureInit()
  const pattern = await evalPattern(code)
  const schedulerCps = Number((_repl?.scheduler as any)?.cps)
  const cps = Number.isFinite(schedulerCps) && schedulerCps > 0 ? schedulerCps : 0.5
  const minLoopCycles = detectLoopCycles(pattern, cps)
  const meta = { cps, cycleDuration: 1 / cps, minLoopCycles }
  cachedComposition = { code, pattern, meta }
  console.log('[strudel] composition analyzed:', JSON.stringify(meta))
  return cachedComposition
}

export async function analyzeStrudel(code: string): Promise<StrudelMeta> {
  const { meta } = await evaluateComposition(code)
  return meta
}

export async function renderToUrl(
  code: string,
  loops: number,
  onProgress?: RenderProgress,
  signal?: AbortSignal,
): Promise<string> {
  console.log('[strudel] renderToUrl start — loops:', loops)
  const blob = await renderToWavBlob(code, loops, onProgress, signal)
  throwIfAborted(signal)
  const url = URL.createObjectURL(blob)
  onProgress?.(1, 'Preview pronto')
  console.log('[strudel] preview URL created — bytes:', blob.size)
  return url
}

/** Uses Strudel for evaluation, event scheduling, samples, effects and rendering. */
async function renderToWavBlob(
  code: string,
  loops: number,
  onProgress?: RenderProgress,
  signal?: AbortSignal,
): Promise<Blob> {
  throwIfAborted(signal)
  onProgress?.(0.03, 'Analisando composição…')
  const { pattern, meta } = await evaluateComposition(code)
  throwIfAborted(signal)
  const endCycle = meta.minLoopCycles * loops
  onProgress?.(0.1, `Preparando ${endCycle} cycles…`)
  console.log('[strudel] got pattern, rendering official Strudel events (begin=0, end=', endCycle, ')')

  const sampleRate = 44100
  const previousContext = strudelWebaudio.getAudioContext()
  await previousContext.close()
  const offlineContext = new OfflineAudioContext(2, (endCycle / meta.cps) * sampleRate, sampleRate)
  strudelWebaudio.setAudioContext(offlineContext)
  strudelWebaudio.setSuperdoughAudioController(null)

  try {
    await strudelWebaudio.initAudio({ maxPolyphony: 32, multiChannelOrbits: false })
    const events = pattern
      .queryArc(0, endCycle, { _cps: meta.cps })
      .sort((a: any, b: any) => a.whole.begin.valueOf() - b.whole.begin.valueOf())

    const onsets = events.filter((event: any) => event.hasOnset())
    console.log('[strudel] scheduling onsets:', onsets.length)
    for (let index = 0; index < onsets.length; index++) {
      throwIfAborted(signal)
      const event = onsets[index]
      event.ensureObjectValue()
      await strudelWebaudio.superdough(
        event.value,
        event.whole.begin.valueOf() / meta.cps,
        event.duration / meta.cps,
        meta.cps,
        event.whole.begin.valueOf() / meta.cps,
      )
      onProgress?.(
        0.12 + (0.68 * (index + 1)) / Math.max(onsets.length, 1),
        `Preparando sons ${index + 1}/${onsets.length}…`,
      )
    }

    onProgress?.(0.84, 'Renderizando áudio…')
    const renderedBuffer = await offlineContext.startRendering()
    throwIfAborted(signal)
    onProgress?.(0.94, 'Codificando WAV…')
    const wavBytes = audioBufferToWav(renderedBuffer)
    throwIfAborted(signal)
    const wavBlob = new Blob([wavBytes], { type: 'audio/wav' })
    console.log('[strudel] offline render done — bytes:', wavBlob.size)
    return wavBlob
  } finally {
    strudelWebaudio.resetGlobalEffects()
    strudelWebaudio.setSuperdoughAudioController(null)
    strudelWebaudio.setAudioContext(null)
  }
}

function audioBufferToWav(buffer: AudioBuffer): ArrayBuffer {
  const channels = buffer.numberOfChannels
  const samplesPerChannel = buffer.length
  const bytesPerSample = 2
  const dataSize = samplesPerChannel * channels * bytesPerSample
  const wav = new ArrayBuffer(44 + dataSize)
  const view = new DataView(wav)

  function writeAscii(offset: number, value: string) {
    for (let i = 0; i < value.length; i++) view.setUint8(offset + i, value.charCodeAt(i))
  }

  writeAscii(0, 'RIFF')
  view.setUint32(4, 36 + dataSize, true)
  writeAscii(8, 'WAVE')
  writeAscii(12, 'fmt ')
  view.setUint32(16, 16, true)
  view.setUint16(20, 1, true)
  view.setUint16(22, channels, true)
  view.setUint32(24, buffer.sampleRate, true)
  view.setUint32(28, buffer.sampleRate * channels * bytesPerSample, true)
  view.setUint16(32, channels * bytesPerSample, true)
  view.setUint16(34, 16, true)
  writeAscii(36, 'data')
  view.setUint32(40, dataSize, true)

  const channelData = Array.from({ length: channels }, (_, channel) => buffer.getChannelData(channel))
  let offset = 44
  for (let frame = 0; frame < samplesPerChannel; frame++) {
    for (let channel = 0; channel < channels; channel++) {
      const sample = Math.max(-1, Math.min(1, channelData[channel][frame]))
      view.setInt16(offset, sample < 0 ? sample * 0x8000 : sample * 0x7fff, true)
      offset += bytesPerSample
    }
  }

  return wav
}

export async function exportToWav(
  code: string,
  loops: number,
  name: string,
  onProgress?: RenderProgress,
  signal?: AbortSignal,
): Promise<void> {
  console.log('[strudel] exportToWav start — loops:', loops, 'name:', name)
  const blob = await renderToWavBlob(code, loops, onProgress, signal)
  throwIfAborted(signal)
  const { save } = await import('@tauri-apps/plugin-dialog')
  const fileName = name.toLowerCase().endsWith('.wav') ? name : `${name}.wav`
  onProgress?.(0.96, 'Escolhendo destino…')
  const path = await save({
    defaultPath: fileName,
    filters: [{ name: 'WAV audio', extensions: ['wav'] }],
  })

  throwIfAborted(signal)

  if (!path) {
    console.log('[strudel] export cancelled by user')
    return
  }

  const bytes = Array.from(new Uint8Array(await blob.arrayBuffer()))
  throwIfAborted(signal)
  onProgress?.(0.98, 'Salvando arquivo…')
  console.log('[strudel] saving WAV through Tauri — path:', path, 'bytes:', bytes.length)
  const { invoke } = await import('@tauri-apps/api/core')
  await invoke('save_wav_bytes', { path, bytes })
  throwIfAborted(signal)
  onProgress?.(1, 'Exportação concluída')
  console.log('[strudel] export done — path:', path)
}
