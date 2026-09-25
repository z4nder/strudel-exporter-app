<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog'
  import { calcDuration, formatDuration, type StrudelMeta } from './lib/loop-parser'
  import { analyzeStrudel, renderToUrl, exportToWav } from './lib/strudel-runner'

  type AppStatus = 'idle' | 'analyzing' | 'rendering_preview' | 'rendering_export' | 'error'

  let filePath = $state('')
  let fileContent = $state('')
  let trackName = $state('track')
  let loops = $state(1)
  let appStatus = $state<AppStatus>('idle')
  let errorMsg = $state('')
  let isDragging = $state(false)
  let meta = $state<StrudelMeta | null>(null)
  let renderProgress = $state(0)
  let progressLabel = $state('')
  let renderController = $state<AbortController | null>(null)
  let cancelRequested = $state(false)

  // preview
  let previewUrl = $state('')
  let audioEl = $state<HTMLAudioElement | null>(null)
  let audioPlaying = $state(false)
  let audioTime = $state(0)
  let audioDuration = $state(0)
  let hasFile = $derived(fileContent.length > 0)
  let duration = $derived(meta ? calcDuration(meta, loops) : 0)
  let isBusy = $derived(appStatus === 'analyzing' || appStatus === 'rendering_preview' || appStatus === 'rendering_export')
  let isRendering = $derived(appStatus === 'rendering_preview' || appStatus === 'rendering_export')
  let hasPreview = $derived(previewUrl.length > 0)

  async function pickFile() {
    const selected = await open({
      filters: [{ name: 'Strudel', extensions: ['strudel', 'js'] }],
      multiple: false,
    })
    if (typeof selected === 'string') await loadFile(selected)
  }

  async function loadFile(path: string) {
    try {
      filePath = path
      const name = path.split(/[/\\]/).pop() ?? path
      trackName = name.replace(/\.(strudel|js)$/i, '')
      const { invoke } = await import('@tauri-apps/api/core')
      meta = null
      fileContent = await invoke<string>('read_strudel_file', { path })
      appStatus = 'analyzing'
      meta = await analyzeStrudel(fileContent)
      appStatus = 'idle'
      errorMsg = ''
      clearPreview()
    } catch (err) {
      appStatus = 'error'
      errorMsg = `Erro ao importar arquivo: ${err}`
    }
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault()
    isDragging = true
  }
  function handleDragLeave(e: DragEvent) {
    e.preventDefault()
    isDragging = false
  }
  function handleDrop(e: DragEvent) {
    e.preventDefault()
    isDragging = false
    const file = e.dataTransfer?.files[0]
    if (!file) return
    const path = (file as any).path
    if (path) loadFile(path)
  }

  function clearPreview() {
    if (previewUrl) {
      URL.revokeObjectURL(previewUrl)
      previewUrl = ''
    }
    audioPlaying = false
    audioTime = 0
    audioDuration = 0
  }

  function reset() {
    renderController?.abort()
    clearPreview()
    filePath = ''
    fileContent = ''
    trackName = 'track'
    loops = 1
    meta = null
    appStatus = 'idle'
    errorMsg = ''
    renderProgress = 0
    progressLabel = ''
    renderController = null
    cancelRequested = false
  }

  function updateProgress(progress: number, label: string) {
    renderProgress = Math.max(0, Math.min(1, progress))
    progressLabel = label
  }

  function normalizeLoops() {
    loops = Math.min(999, Math.max(1, Math.round(Number(loops) || 1)))
  }

  function normalizeTrackName() {
    trackName = trackName.trim().replace(/\.wav$/i, '') || 'track'
  }

  function cancelGeneration() {
    if (!renderController || cancelRequested) return
    cancelRequested = true
    progressLabel = 'Cancelando geração…'
    renderController.abort()
  }

  async function generatePreview() {
    if (!meta || !fileContent || isBusy) return
    normalizeLoops()
    clearPreview()
    renderProgress = 0
    progressLabel = 'Iniciando preview…'
    cancelRequested = false
    const controller = new AbortController()
    renderController = controller
    appStatus = 'rendering_preview'
    try {
      const url = await renderToUrl(
        fileContent,
        loops,
        (progress, label) => {
          if (!controller.signal.aborted) updateProgress(progress, label)
        },
        controller.signal,
      )
      previewUrl = url
      appStatus = 'idle'
    } catch (err) {
      if (controller.signal.aborted) {
        appStatus = 'idle'
        progressLabel = 'Preview cancelado'
      } else {
        appStatus = 'error'
        errorMsg = `Erro no preview: ${err}`
      }
    } finally {
      if (renderController === controller) renderController = null
      cancelRequested = false
    }
  }

  async function doExport() {
    if (!meta || !fileContent || isBusy) return
    normalizeLoops()
    normalizeTrackName()
    // Stop preview playback before creating a new offline AudioContext.
    if (audioEl) audioEl.pause()
    renderProgress = 0
    progressLabel = 'Iniciando exportação…'
    cancelRequested = false
    const controller = new AbortController()
    renderController = controller
    appStatus = 'rendering_export'
    try {
      await exportToWav(
        fileContent,
        loops,
        trackName,
        (progress, label) => {
          if (!controller.signal.aborted) updateProgress(progress, label)
        },
        controller.signal,
      )
      appStatus = 'idle'
    } catch (err) {
      if (controller.signal.aborted) {
        appStatus = 'idle'
        progressLabel = 'Exportação cancelada'
      } else {
        appStatus = 'error'
        errorMsg = `Erro ao exportar: ${err}`
      }
    } finally {
      if (renderController === controller) renderController = null
      cancelRequested = false
    }
  }

  // Audio element event handlers
  function onAudioTimeUpdate() {
    if (audioEl) audioTime = audioEl.currentTime
  }
  function onAudioDuration() {
    if (audioEl) audioDuration = audioEl.duration
  }
  function onAudioPlay() { audioPlaying = true }
  function onAudioPause() { audioPlaying = false }
  function onAudioEnded() { audioPlaying = false; audioTime = 0 }

  function togglePlay() {
    if (!audioEl) return
    if (audioPlaying) {
      audioEl.pause()
    } else {
      audioEl.play()
    }
  }

  function stopAudio() {
    if (!audioEl) return
    audioEl.pause()
    audioEl.currentTime = 0
  }

  function seek(e: Event) {
    if (!audioEl) return
    audioEl.currentTime = Number((e.target as HTMLInputElement).value)
  }

  function formatTime(s: number): string {
    if (!isFinite(s)) return '0:00'
    const m = Math.floor(s / 60)
    const sec = Math.floor(s % 60).toString().padStart(2, '0')
    return `${m}:${sec}`
  }
</script>

<!-- Hidden audio element for preview playback -->
{#if previewUrl}
  <audio
    bind:this={audioEl}
    src={previewUrl}
    ontimeupdate={onAudioTimeUpdate}
    ondurationchange={onAudioDuration}
    onplay={onAudioPlay}
    onpause={onAudioPause}
    onended={onAudioEnded}
  ></audio>
{/if}

<main>
  <header>
    <img src="/assets/logo.png" alt="Strudel logo" class="logo-sm" />
    <span class="app-name">Strudel WAV Exporter</span>
  </header>

  {#if !hasFile}
    <div
      class="drop-zone"
      class:dragging={isDragging}
      role="button"
      tabindex="0"
      aria-label="Arraste um arquivo .strudel ou clique para escolher"
      onclick={pickFile}
      onkeydown={(e) => e.key === 'Enter' && pickFile()}
      ondragover={handleDragOver}
      ondragleave={handleDragLeave}
      ondrop={handleDrop}
    >
      <img src="/assets/logo.png" alt="" class="logo-watermark" aria-hidden="true" />
      <div class="drop-text">
        <p class="drop-main">Arraste seu arquivo <span class="ext">.strudel</span></p>
        <p class="drop-sub">ou clique para escolher</p>
      </div>
    </div>
  {:else}
    <div class="file-card">
      <div class="file-card-header">
        <svg class="file-icon" viewBox="0 0 20 20" fill="none" aria-hidden="true">
          <rect x="3" y="1" width="11" height="18" rx="1.5" stroke="currentColor" stroke-width="1.5"/>
          <path d="M14 1l3 3v15H7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
          <path d="M6 8h8M6 11h6M6 14h4" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" opacity=".5"/>
        </svg>
        <span class="file-name">{filePath.split('/').pop()}</span>
        <button class="close-btn" onclick={reset} aria-label="Remover arquivo" disabled={isBusy}>×</button>
      </div>
      {#if meta}
        <div class="file-meta">
          <div class="meta-chip">
            <span class="meta-val">{meta.cps}</span>
            <span class="meta-label">cps</span>
          </div>
          <div class="meta-sep">·</div>
          <div class="meta-chip">
            <span class="meta-val">{meta.cycleDuration.toFixed(2)}s</span>
            <span class="meta-label">/ cycle</span>
          </div>
          <div class="meta-sep">·</div>
          <div class="meta-chip">
            <span class="meta-val">{meta.minLoopCycles}</span>
            <span class="meta-label">cycles / loop</span>
          </div>
        </div>
      {/if}
    </div>

    <section class="controls">
      <label class="field-label" for="track-name">Nome do arquivo</label>
      <div class="filename-input">
        <input
          id="track-name"
          type="text"
          bind:value={trackName}
          maxlength="180"
          disabled={isBusy}
          onblur={normalizeTrackName}
          aria-describedby="filename-suffix"
        />
        <span id="filename-suffix">.wav</span>
      </div>

      <label class="loops-label" for="loops-range">
        Loops para preview e export
        <input
          class="loops-number"
          type="number"
          min="1"
          max="999"
          bind:value={loops}
          disabled={isBusy}
          onchange={normalizeLoops}
          onblur={normalizeLoops}
        />
      </label>
      <input
        id="loops-range"
        class="loops-slider"
        type="range"
        min="1"
        max="999"
        bind:value={loops}
        disabled={isBusy}
        onchange={normalizeLoops}
      />

      <div class="duration-display">
        Exportar: <strong>{formatDuration(duration)}</strong>
        <span class="duration-sep">·</span>
        Preview: <strong>{formatDuration(duration)}</strong>
        <span class="loops-hint">({loops} {loops === 1 ? 'loop' : 'loops'})</span>
      </div>

      <div class="action-row">
        <button
          class="btn-preview"
          onclick={generatePreview}
          disabled={isBusy}
        >
          {#if appStatus === 'rendering_preview'}
            <span class="spinner" aria-hidden="true"></span> Gerando...
          {:else}
            <svg viewBox="0 0 16 16" fill="currentColor" width="14" height="14" aria-hidden="true">
              <path d="M3 2.5l10 5.5-10 5.5V2.5z"/>
            </svg>
            Preview
          {/if}
        </button>

        <button
          class="btn-export"
          onclick={doExport}
          disabled={isBusy}
        >
          {#if appStatus === 'rendering_export'}
            <span class="spinner spinner-dark" aria-hidden="true"></span> Renderizando...
          {:else}
            Exportar WAV
          {/if}
        </button>
      </div>

      {#if isRendering}
        <div class="render-progress">
          <div class="progress-copy">
            <span>{progressLabel}</span>
            <span>{Math.round(renderProgress * 100)}%</span>
          </div>
          <div
            class="progress-track"
            role="progressbar"
            aria-label={appStatus === 'rendering_preview' ? 'Geração do preview' : 'Exportação do WAV'}
            aria-valuemin="0"
            aria-valuemax="100"
            aria-valuenow={Math.round(renderProgress * 100)}
          >
            <div class="progress-fill" style={`width: ${Math.round(renderProgress * 100)}%`}></div>
          </div>
          <button class="btn-cancel" type="button" onclick={cancelGeneration} disabled={cancelRequested}>
            {cancelRequested ? 'Cancelando…' : 'Cancelar geração'}
          </button>
        </div>
      {/if}

      {#if hasPreview}
        <div class="player">
          <div class="player-controls">
            <button class="player-btn" onclick={togglePlay} aria-label={audioPlaying ? 'Pausar' : 'Reproduzir'}>
              {#if audioPlaying}
                <!-- Pause icon -->
                <svg viewBox="0 0 16 16" fill="currentColor" width="16" height="16">
                  <rect x="3" y="2" width="4" height="12" rx="1"/>
                  <rect x="9" y="2" width="4" height="12" rx="1"/>
                </svg>
              {:else}
                <!-- Play icon -->
                <svg viewBox="0 0 16 16" fill="currentColor" width="16" height="16">
                  <path d="M3 2.5l10 5.5-10 5.5V2.5z"/>
                </svg>
              {/if}
            </button>

            <button class="player-btn" onclick={stopAudio} aria-label="Parar">
              <svg viewBox="0 0 16 16" fill="currentColor" width="14" height="14">
                <rect x="2" y="2" width="12" height="12" rx="1"/>
              </svg>
            </button>

            <span class="player-time">{formatTime(audioTime)}</span>

            <input
              class="player-seek"
              type="range"
              min="0"
              max={audioDuration || 0}
              step="0.01"
              value={audioTime}
              oninput={seek}
              aria-label="Posição"
            />

            <span class="player-time player-time-total">{formatTime(audioDuration)}</span>
          </div>
        </div>
      {/if}

      {#if appStatus === 'error'}
        <div class="error-banner">
          <span>{errorMsg}</span>
          <button onclick={() => { appStatus = 'idle'; errorMsg = '' }}>×</button>
        </div>
      {/if}
    </section>
  {/if}
</main>

<style>
  :global(*, *::before, *::after) { box-sizing: border-box; margin: 0; padding: 0; }
  :global(body) {
    background: #0c0c12;
    color: #e0dcd4;
    font-family: 'Space Grotesk', system-ui, sans-serif;
    height: 100vh;
    overflow: hidden;
    -webkit-font-smoothing: antialiased;
  }
  :global(#app) { height: 100vh; }

  main {
    height: 100vh;
    display: flex;
    flex-direction: column;
    padding: 28px 40px 32px;
    gap: 20px;
    overflow-y: auto;
  }

  header {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .logo-sm { width: 28px; height: 28px; object-fit: contain; }
  .app-name { font-size: 15px; font-weight: 500; color: #8a8aa0; letter-spacing: 0.01em; }

  /* ── Drop zone ── */
  .drop-zone {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    position: relative;
    border: 2px dashed #2a2a3a;
    border-radius: 12px;
    cursor: pointer;
    transition: border-color 180ms ease, background 180ms ease;
    outline: none;
    overflow: hidden;
    user-select: none;
  }
  .drop-zone:hover, .drop-zone:focus-visible {
    border-color: rgba(200, 134, 30, 0.5);
    background: rgba(200, 134, 30, 0.04);
  }
  .drop-zone.dragging {
    border-color: #c8861e;
    background: rgba(200, 134, 30, 0.08);
    box-shadow: inset 0 0 60px rgba(200, 134, 30, 0.06);
  }
  .logo-watermark {
    position: absolute;
    width: 200px; height: 200px;
    object-fit: contain;
    opacity: 0.07;
    pointer-events: none;
    transition: opacity 180ms ease;
  }
  .drop-zone:hover .logo-watermark, .drop-zone:focus-visible .logo-watermark { opacity: 0.12; }
  .drop-zone.dragging .logo-watermark { opacity: 0.22; }
  .drop-text { position: relative; z-index: 1; text-align: center; display: flex; flex-direction: column; gap: 8px; }
  .drop-main { font-size: 18px; font-weight: 400; color: #70708a; }
  .ext { color: #c8861e; font-family: 'JetBrains Mono', monospace; font-size: 0.95em; }
  .drop-sub { font-size: 13px; color: #3a3a50; }

  /* ── File card ── */
  .file-card {
    background: #131320;
    border: 1px solid #1e1e2c;
    border-radius: 10px;
    padding: 14px 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .file-card-header { display: flex; align-items: center; gap: 10px; }
  .file-icon { width: 18px; height: 18px; color: #c8861e; flex-shrink: 0; }
  .file-name {
    font-family: 'JetBrains Mono', monospace;
    font-size: 13px; color: #c0bdb5;
    flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .close-btn {
    background: none; border: none; color: #3a3a50;
    font-size: 20px; line-height: 1; cursor: pointer; padding: 0 4px;
    transition: color 140ms;
  }
  .close-btn:hover { color: #c85050; }
  .close-btn:disabled { opacity: 0.35; cursor: not-allowed; }
  .file-meta { display: flex; align-items: center; gap: 10px; }
  .meta-chip { display: flex; align-items: baseline; gap: 5px; }
  .meta-val { font-family: 'JetBrains Mono', monospace; font-size: 14px; font-weight: 500; color: #c8861e; }
  .meta-label { font-size: 12px; color: #50506a; }
  .meta-sep { color: #2a2a3a; font-size: 16px; }

  /* ── Controls ── */
  .controls { display: flex; flex-direction: column; gap: 14px; }

  .field-label {
    font-size: 13px; color: #50506a; font-weight: 500;
  }
  .filename-input {
    display: flex; align-items: stretch;
    background: #131320; border: 1px solid #1e1e2c; border-radius: 8px;
    overflow: hidden; transition: border-color 140ms;
  }
  .filename-input:focus-within { border-color: #c8861e; }
  .filename-input input {
    min-width: 0; flex: 1; padding: 10px 12px;
    background: transparent; border: 0; outline: none;
    color: #e0dcd4; font-family: 'JetBrains Mono', monospace; font-size: 13px;
  }
  .filename-input span {
    display: flex; align-items: center; padding: 0 13px;
    background: #1a1a28; border-left: 1px solid #29293a;
    color: #c8861e; font-family: 'JetBrains Mono', monospace; font-size: 13px;
    user-select: none;
  }
  .filename-input:has(input:disabled) { opacity: 0.55; }

  .loops-label {
    display: flex; align-items: center; justify-content: space-between;
    font-size: 13px; color: #50506a; font-weight: 500;
  }
  .loops-number {
    background: #131320; border: 1px solid #1e1e2c; border-radius: 6px;
    color: #e0dcd4; font-family: 'JetBrains Mono', monospace;
    font-size: 15px; padding: 7px 12px; width: 96px; text-align: center;
    outline: none; transition: border-color 140ms;
  }
  .loops-number:focus { border-color: #c8861e; }
  .loops-number:disabled, .loops-slider:disabled { opacity: 0.45; cursor: not-allowed; }
  .loops-slider {
    width: 100%; appearance: none; height: 3px;
    background: #1e1e2c; border-radius: 2px; outline: none; cursor: pointer;
  }
  .loops-slider::-webkit-slider-thumb {
    appearance: none; width: 16px; height: 16px; border-radius: 50%;
    background: #c8861e; cursor: pointer; border: 2px solid #0c0c12;
    transition: box-shadow 140ms;
  }
  .loops-slider::-webkit-slider-thumb:hover { box-shadow: 0 0 0 6px rgba(200, 134, 30, 0.18); }

  .duration-display { font-size: 13px; color: #50506a; display: flex; align-items: center; gap: 6px; }
  .duration-display strong { color: #e0dcd4; font-weight: 500; }
  .duration-sep { color: #2a2a3a; }
  .loops-hint { font-size: 12px; color: #38384a; }

  /* ── Action buttons ── */
  .action-row { display: flex; gap: 10px; }

  .btn-preview {
    display: flex; align-items: center; justify-content: center; gap: 8px;
    padding: 12px 20px; flex: 0 0 auto;
    background: #1a1a28; border: 1px solid #c8861e;
    color: #c8861e; border-radius: 8px;
    font-family: 'Space Grotesk', sans-serif;
    font-size: 14px; font-weight: 500; cursor: pointer;
    transition: background 160ms, opacity 160ms;
  }
  .btn-preview:hover:not(:disabled) { background: rgba(200, 134, 30, 0.12); }
  .btn-preview:disabled { opacity: 0.45; cursor: not-allowed; }

  .btn-export {
    display: flex; align-items: center; justify-content: center; gap: 10px;
    flex: 1; padding: 12px;
    background: #c8861e; color: #0c0c12;
    border: none; border-radius: 8px;
    font-family: 'Space Grotesk', sans-serif;
    font-size: 14px; font-weight: 600; cursor: pointer;
    transition: background 160ms, opacity 160ms;
  }
  .btn-export:hover:not(:disabled) { background: #d9971f; }
  .btn-export:disabled { opacity: 0.55; cursor: not-allowed; }

  /* ── Render progress ── */
  .render-progress {
    display: grid; grid-template-columns: 1fr auto; gap: 9px 14px; align-items: center;
    padding: 12px 14px; background: #131320;
    border: 1px solid #242435; border-radius: 8px;
  }
  .progress-copy {
    grid-column: 1 / -1; display: flex; justify-content: space-between; gap: 16px;
    color: #77778e; font-size: 12px; font-family: 'JetBrains Mono', monospace;
  }
  .progress-track {
    height: 6px; overflow: hidden; background: #272738; border-radius: 999px;
  }
  .progress-fill {
    height: 100%; background: #c8861e; border-radius: inherit;
    transition: width 120ms ease;
  }
  .btn-cancel {
    padding: 6px 10px; white-space: nowrap;
    background: transparent; color: #c86b62; border: 1px solid #613a3a; border-radius: 6px;
    font: 500 12px 'Space Grotesk', sans-serif; cursor: pointer;
  }
  .btn-cancel:hover:not(:disabled) { background: rgba(200, 80, 80, 0.1); }
  .btn-cancel:disabled { opacity: 0.45; cursor: wait; }

  /* ── Spinner ── */
  .spinner {
    display: inline-block; width: 13px; height: 13px;
    border: 2px solid rgba(200, 134, 30, 0.3);
    border-top-color: #c8861e;
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }
  .spinner-dark {
    border-color: rgba(12, 12, 18, 0.3);
    border-top-color: #0c0c12;
  }
  @keyframes spin { to { transform: rotate(360deg); } }

  /* ── Player ── */
  .player {
    background: #131320;
    border: 1px solid #1e1e2c;
    border-radius: 10px;
    padding: 12px 16px;
  }
  .player-controls {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .player-btn {
    display: flex; align-items: center; justify-content: center;
    width: 32px; height: 32px; flex-shrink: 0;
    background: #1e1e2c; border: none; border-radius: 6px;
    color: #c8861e; cursor: pointer;
    transition: background 140ms;
  }
  .player-btn:hover { background: #28283a; }

  .player-time {
    font-family: 'JetBrains Mono', monospace;
    font-size: 12px; color: #50506a;
    white-space: nowrap; flex-shrink: 0;
    min-width: 32px;
  }
  .player-time-total { color: #38384a; }

  .player-seek {
    flex: 1;
    appearance: none; height: 3px;
    background: #2a2a3a; border-radius: 2px;
    outline: none; cursor: pointer;
  }
  .player-seek::-webkit-slider-thumb {
    appearance: none; width: 12px; height: 12px;
    border-radius: 50%; background: #c8861e;
    cursor: pointer; border: 2px solid #0c0c12;
  }

  /* ── Error ── */
  .error-banner {
    display: flex; align-items: center; justify-content: space-between;
    padding: 10px 14px;
    background: rgba(200, 80, 80, 0.1);
    border-left: 3px solid #c85050;
    border-radius: 6px;
    font-size: 12px; color: #e07070;
    font-family: 'JetBrains Mono', monospace;
  }
  .error-banner button {
    background: none; border: none; color: inherit;
    font-size: 16px; cursor: pointer; opacity: 0.6; padding: 0 4px;
  }
  .error-banner button:hover { opacity: 1; }

  @media (prefers-reduced-motion: reduce) {
    .spinner { animation: none; }
    .drop-zone, .logo-watermark { transition: none; }
  }
</style>
