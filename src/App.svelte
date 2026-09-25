<script lang="ts">
  import { onMount } from 'svelte'
  import { open } from '@tauri-apps/plugin-dialog'
  import { invoke } from '@tauri-apps/api/core'
  import { formatDuration, type StrudelMeta } from './lib/loop-parser'
  import { analyzeStrudel, renderToUrl, exportToWav, type RenderSettings } from './lib/strudel-runner'
  import {
    deleteTrack as deleteLibraryTrack,
    importTrack as importLibraryTrack,
    listTracks,
    saveTrackSettings,
    type LibraryTrack,
    type SavedTrackSettings,
  } from './lib/library'

  type AppStatus = 'idle' | 'analyzing' | 'rendering_preview' | 'rendering_export' | 'error'
  type AppScreen = 'library' | 'track'
  type HomeTab = 'tracks' | 'albums'

  let screen = $state<AppScreen>('library')
  let homeTab = $state<HomeTab>('tracks')
  let tracks = $state<LibraryTrack[]>([])
  let activeTrackId = $state<number | null>(null)
  let libraryLoading = $state(true)
  let libraryError = $state('')
  let trackSearch = $state('')
  let savedSettingsKey = $state('')
  let saveMessage = $state('')
  let trackPendingDelete = $state<LibraryTrack | null>(null)
  let deletingTrack = $state(false)

  let filePath = $state('')
  let fileContent = $state('')
  let trackName = $state('track')
  let loops = $state(1)
  let startCycle = $state(0)
  let endCycle = $state(1)
  let sampleRate = $state<RenderSettings['sampleRate']>(44100)
  let maxPolyphony = $state(32)
  let automaticRange = $state(true)
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
  let baseAudioDuration = $state(0)
  let previewIteration = $state(0)
  let previewLoops = $state(1)
  let duration = $derived(meta ? ((endCycle - startCycle) / meta.cps) * loops : 0)
  let isBusy = $derived(appStatus === 'analyzing' || appStatus === 'rendering_preview' || appStatus === 'rendering_export')
  let isRendering = $derived(appStatus === 'rendering_preview' || appStatus === 'rendering_export')
  let hasPreview = $derived(previewUrl.length > 0)
  let visibleTracks = $derived(
    tracks.filter((track) => {
      const query = trackSearch.trim().toLocaleLowerCase()
      return !query || track.name.toLocaleLowerCase().includes(query) || track.sourcePath.toLocaleLowerCase().includes(query)
    }),
  )
  let settingsDirty = $derived(activeTrackId !== null && savedSettingsKey !== serializedSettings())

  onMount(() => {
    void refreshTracks()
  })

  async function refreshTracks() {
    libraryLoading = true
    try {
      tracks = await listTracks()
      libraryError = ''
    } catch (err) {
      libraryError = `Erro ao carregar biblioteca: ${err}`
    } finally {
      libraryLoading = false
    }
  }

  async function pickFile() {
    const selected = await open({
      filters: [{ name: 'Strudel', extensions: ['strudel', 'js'] }],
      multiple: false,
    })
    if (typeof selected === 'string') await addTrack(selected)
  }

  async function addTrack(path: string) {
    try {
      const track = await importLibraryTrack(path)
      await refreshTracks()
      await openTrack(track)
    } catch (err) {
      libraryError = `Erro ao importar arquivo: ${err}`
    }
  }

  async function openTrack(track: LibraryTrack) {
    try {
      clearPreview()
      screen = 'track'
      activeTrackId = track.id
      filePath = track.sourcePath
      trackName = track.settings.exportFileName
      loops = track.settings.loops
      startCycle = track.settings.startCycle
      endCycle = track.settings.endCycle
      sampleRate = track.settings.sampleRate
      maxPolyphony = track.settings.maxPolyphony
      automaticRange = track.settings.rangeMode === 'automatic'
      savedSettingsKey = JSON.stringify(track.settings)
      saveMessage = ''
      meta = null
      appStatus = 'analyzing'
      fileContent = await invoke<string>('read_strudel_file', { path: track.sourcePath })
      meta = await analyzeStrudel(fileContent)
      if (automaticRange) {
        startCycle = 0
        endCycle = meta.minLoopCycles
        if (savedSettingsKey !== serializedSettings()) await saveTrackConfiguration(false)
      }
      appStatus = 'idle'
      errorMsg = ''
    } catch (err) {
      appStatus = 'error'
      errorMsg = `Erro ao abrir Track: ${err}`
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
    if (path) addTrack(path)
  }

  function clearPreview() {
    if (previewUrl) {
      URL.revokeObjectURL(previewUrl)
      previewUrl = ''
    }
    audioPlaying = false
    audioTime = 0
    audioDuration = 0
    baseAudioDuration = 0
    previewIteration = 0
    previewLoops = 1
  }

  function closeTrack() {
    renderController?.abort()
    clearPreview()
    filePath = ''
    fileContent = ''
    trackName = 'track'
    loops = 1
    startCycle = 0
    endCycle = 1
    sampleRate = 44100
    maxPolyphony = 32
    automaticRange = true
    meta = null
    appStatus = 'idle'
    errorMsg = ''
    renderProgress = 0
    progressLabel = ''
    renderController = null
    cancelRequested = false
    activeTrackId = null
    savedSettingsKey = ''
    saveMessage = ''
    screen = 'library'
    void refreshTracks()
  }

  function updateProgress(progress: number, label: string) {
    renderProgress = Math.max(0, Math.min(1, progress))
    progressLabel = label
  }

  function normalizeLoops() {
    loops = Math.min(999, Math.max(1, Math.round(Number(loops) || 1)))
    if (hasPreview) {
      previewLoops = loops
      audioDuration = baseAudioDuration * previewLoops
      if (previewIteration >= previewLoops) stopAudio()
    }
  }

  function normalizeTrackName() {
    trackName = trackName.trim().replace(/\.wav$/i, '') || 'track'
  }

  function normalizeRenderSettings(manual = true) {
    startCycle = Math.max(0, Number(startCycle) || 0)
    endCycle = Math.max(startCycle + 0.25, Number(endCycle) || startCycle + 1)
    maxPolyphony = Math.min(256, Math.max(1, Math.round(Number(maxPolyphony) || 32)))
    if (manual) automaticRange = false
  }

  function changeRenderSettings(manualRange = false) {
    normalizeRenderSettings(manualRange)
    onAudioSettingChange()
  }

  function useDetectedRange(clear = true) {
    startCycle = 0
    endCycle = meta?.minLoopCycles ?? 1
    automaticRange = true
    if (clear && hasPreview) clearPreview()
  }

  function onAudioSettingChange() {
    if (hasPreview) clearPreview()
  }

  function currentRenderSettings(): RenderSettings {
    return { startCycle, endCycle, sampleRate, maxPolyphony }
  }

  function currentSavedSettings(): SavedTrackSettings {
    return {
      rangeMode: automaticRange ? 'automatic' : 'manual',
      startCycle,
      endCycle,
      loops,
      sampleRate,
      maxPolyphony,
      exportFileName: trackName,
    }
  }

  function serializedSettings() {
    return JSON.stringify(currentSavedSettings())
  }

  async function saveTrackConfiguration(showMessage = true) {
    if (activeTrackId === null) return
    normalizeLoops()
    normalizeRenderSettings(false)
    normalizeTrackName()
    try {
      const updated = await saveTrackSettings(activeTrackId, currentSavedSettings())
      savedSettingsKey = JSON.stringify(updated.settings)
      tracks = tracks.map((track) => track.id === updated.id ? updated : track)
      if (showMessage) saveMessage = 'Configuração salva'
      errorMsg = ''
    } catch (err) {
      appStatus = 'error'
      errorMsg = `Erro ao salvar configuração: ${err}`
    }
  }

  function requestTrackRemoval(track: LibraryTrack, event: MouseEvent) {
    event.stopPropagation()
    trackPendingDelete = track
  }

  function cancelTrackRemoval() {
    if (!deletingTrack) trackPendingDelete = null
  }

  async function confirmTrackRemoval() {
    if (!trackPendingDelete || deletingTrack) return
    deletingTrack = true
    try {
      await deleteLibraryTrack(trackPendingDelete.id)
      tracks = tracks.filter((item) => item.id !== trackPendingDelete?.id)
      trackPendingDelete = null
    } catch (err) {
      libraryError = `Erro ao remover Track: ${err}`
    } finally {
      deletingTrack = false
    }
  }

  function formatLibraryDate(timestamp: number) {
    return new Intl.DateTimeFormat('pt-BR', { dateStyle: 'short', timeStyle: 'short' })
      .format(new Date(timestamp * 1000))
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
    normalizeRenderSettings(false)
    clearPreview()
    previewLoops = loops
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
        currentRenderSettings(),
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
    normalizeRenderSettings(false)
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
        currentRenderSettings(),
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
    if (audioEl) audioTime = previewIteration * baseAudioDuration + audioEl.currentTime
  }
  function onAudioDuration() {
    if (audioEl && Number.isFinite(audioEl.duration)) {
      baseAudioDuration = audioEl.duration
      audioDuration = baseAudioDuration * previewLoops
    }
  }
  function onAudioPlay() { audioPlaying = true }
  function onAudioPause() { audioPlaying = false }
  function onAudioEnded() {
    if (!audioEl) return
    if (previewIteration + 1 < previewLoops) {
      previewIteration += 1
      audioEl.currentTime = 0
      void audioEl.play()
      return
    }
    audioPlaying = false
    previewIteration = 0
    audioTime = 0
    audioEl.currentTime = 0
  }

  function togglePlay() {
    if (!audioEl) return
    if (audioPlaying) {
      audioEl.pause()
    } else {
      void audioEl.play()
    }
  }

  function stopAudio() {
    if (!audioEl) return
    audioEl.pause()
    previewIteration = 0
    audioEl.currentTime = 0
    audioTime = 0
  }

  function seek(e: Event) {
    if (!audioEl || baseAudioDuration <= 0) return
    const target = Math.min(Number((e.target as HTMLInputElement).value), audioDuration)
    previewIteration = Math.min(Math.floor(target / baseAudioDuration), previewLoops - 1)
    audioEl.currentTime = Math.min(target - previewIteration * baseAudioDuration, baseAudioDuration)
    audioTime = target
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

<svelte:window onkeydown={(event) => event.key === 'Escape' && cancelTrackRemoval()} />

<main>
  <header>
    <div class="brand">
      <img src="/assets/logo.png" alt="Strudel logo" class="logo-sm" />
      <span class="app-name">Strudel Library</span>
    </div>
    {#if screen === 'library'}
      <div class="header-actions">
        <button class="btn-tags" type="button" disabled title="Será implementado na próxima fase">Tags</button>
        <button class="btn-import" type="button" onclick={pickFile}>+ Importar Track</button>
      </div>
    {:else}
      <button class="btn-back" type="button" onclick={closeTrack} disabled={isBusy}>← Biblioteca</button>
    {/if}
  </header>

  {#if screen === 'library'}
    <section class="library-view">
      <div class="home-tabs" role="tablist" aria-label="Biblioteca">
        <button class:active={homeTab === 'tracks'} role="tab" aria-selected={homeTab === 'tracks'} onclick={() => homeTab = 'tracks'}>
          Tracks <span>{tracks.length}</span>
        </button>
        <button class:active={homeTab === 'albums'} role="tab" aria-selected={homeTab === 'albums'} onclick={() => homeTab = 'albums'}>
          Albums <span>0</span>
        </button>
      </div>

      {#if homeTab === 'tracks'}
        <div class="library-toolbar">
          <input bind:value={trackSearch} type="search" placeholder="Buscar por nome ou caminho…" aria-label="Buscar Tracks" />
          <span>{visibleTracks.length} {visibleTracks.length === 1 ? 'Track' : 'Tracks'}</span>
        </div>

        {#if libraryLoading}
          <div class="library-state"><span class="spinner"></span> Carregando biblioteca…</div>
        {:else if visibleTracks.length === 0}
          <div
            class="drop-zone library-empty"
            class:dragging={isDragging}
            role="button"
            tabindex="0"
            aria-label="Arraste um arquivo .strudel ou clique para importar"
            onclick={pickFile}
            onkeydown={(e) => e.key === 'Enter' && pickFile()}
            ondragover={handleDragOver}
            ondragleave={handleDragLeave}
            ondrop={handleDrop}
          >
            <img src="/assets/logo.png" alt="" class="logo-watermark" aria-hidden="true" />
            <div class="drop-text">
              <p class="drop-main">Sua biblioteca está vazia</p>
              <p class="drop-sub">Arraste um arquivo <span class="ext">.strudel</span> ou clique para importar</p>
            </div>
          </div>
        {:else}
          <div class="track-grid">
            {#each visibleTracks as track (track.id)}
              <div
                class="track-card"
                role="button"
                tabindex="0"
                onclick={() => openTrack(track)}
                onkeydown={(event) => event.key === 'Enter' && openTrack(track)}
              >
                <div class="track-card-icon">♪</div>
                <div class="track-card-body">
                  <strong>{track.name}</strong>
                  <span class="track-path">{track.sourcePath}</span>
                  <div class="track-card-meta">
                    <span>{track.settings.endCycle - track.settings.startCycle} cycles</span>
                    <span>{track.settings.loops}× loop</span>
                    <span>{(track.settings.sampleRate / 1000).toFixed(1)} kHz</span>
                    <span>{formatLibraryDate(track.updatedAt)}</span>
                  </div>
                </div>
                <button class="track-delete" type="button" onclick={(event) => requestTrackRemoval(track, event)} aria-label={`Remover ${track.name} da biblioteca`}>×</button>
              </div>
            {/each}
          </div>
        {/if}
      {:else}
        <div class="coming-soon">
          <span>◎</span>
          <strong>Albums entram na próxima fase</strong>
          <p>A fundação de Tracks e configurações persistidas já prepara o relacionamento ordenado do Album.</p>
        </div>
      {/if}

      {#if libraryError}
        <div class="error-banner">
          <span>{libraryError}</span>
          <button onclick={() => libraryError = ''}>×</button>
        </div>
      {/if}
    </section>
  {:else}
    <div class="file-card">
      <div class="file-card-header">
        <svg class="file-icon" viewBox="0 0 20 20" fill="none" aria-hidden="true">
          <rect x="3" y="1" width="11" height="18" rx="1.5" stroke="currentColor" stroke-width="1.5"/>
          <path d="M14 1l3 3v15H7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
          <path d="M6 8h8M6 11h6M6 14h4" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" opacity=".5"/>
        </svg>
        <span class="file-name">{filePath.split('/').pop()}</span>
        <button class="close-btn" onclick={closeTrack} aria-label="Voltar à biblioteca" disabled={isBusy}>×</button>
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

      <details class="advanced-settings">
        <summary>
          <span>Configurações avançadas</span>
          <span class:manual={!automaticRange} class="range-mode">
            {automaticRange ? 'intervalo automático' : 'intervalo manual'}
          </span>
        </summary>
        <div class="advanced-content">
          <div class="advanced-grid">
            <label>
              <span>Start cycle</span>
              <input
                type="number"
                min="0"
                step="0.25"
                bind:value={startCycle}
                disabled={isBusy}
                onchange={() => changeRenderSettings(true)}
                onblur={() => changeRenderSettings(true)}
              />
            </label>
            <label>
              <span>End cycle</span>
              <input
                type="number"
                min={startCycle + 0.25}
                step="0.25"
                bind:value={endCycle}
                disabled={isBusy}
                onchange={() => changeRenderSettings(true)}
                onblur={() => changeRenderSettings(true)}
              />
            </label>
            <label>
              <span>Sample rate</span>
              <select bind:value={sampleRate} disabled={isBusy} onchange={() => changeRenderSettings()}>
                <option value={44100}>44.100 Hz</option>
                <option value={48000}>48.000 Hz</option>
                <option value={96000}>96.000 Hz</option>
              </select>
            </label>
            <label>
              <span>Maximum polyphony</span>
              <input
                type="number"
                min="1"
                max="256"
                step="1"
                bind:value={maxPolyphony}
                disabled={isBusy}
                onchange={() => changeRenderSettings()}
                onblur={() => changeRenderSettings()}
              />
            </label>
          </div>
          <div class="detected-range">
            <span>
              Round detectado: <strong>0 → {meta?.minLoopCycles ?? 1}</strong>
            </span>
            <button
              type="button"
              onclick={() => useDetectedRange()}
              disabled={isBusy}
            >
              Restaurar automático
            </button>
          </div>
          <p class="advanced-help">
            Alterar Start ou End substitui o round detectado. Os loops repetem exatamente esse intervalo.
          </p>
        </div>
      </details>

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
      <div class="render-summary">
        Cycles {startCycle} → {endCycle} · {(sampleRate / 1000).toFixed(1)} kHz · {maxPolyphony} vozes
      </div>

      <div class="save-config-row" class:dirty={settingsDirty}>
        <div>
          <strong>{settingsDirty ? 'Alterações não salvas' : 'Configuração salva'}</strong>
          <span>{saveMessage || 'Esta configuração será restaurada ao abrir a Track.'}</span>
        </div>
        <button type="button" onclick={() => saveTrackConfiguration()} disabled={isBusy || !settingsDirty}>
          Salvar configuração
        </button>
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
          <div class="player-loop-info">1 round renderizado × {previewLoops}</div>
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

{#if trackPendingDelete}
  <div class="modal-backdrop">
    <button class="modal-dismiss" type="button" onclick={cancelTrackRemoval} aria-label="Fechar confirmação"></button>
    <div
      class="confirm-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="remove-track-title"
    >
      <div class="modal-icon" aria-hidden="true">×</div>
      <div class="modal-copy">
        <h2 id="remove-track-title">Remover Track?</h2>
        <p>
          <strong>{trackPendingDelete.name}</strong> será removida da biblioteca junto com sua configuração salva.
        </p>
        <p class="modal-note">O arquivo <code>.strudel</code> original não será excluído.</p>
      </div>
      <div class="modal-actions">
        <button class="modal-cancel" type="button" onclick={cancelTrackRemoval} disabled={deletingTrack}>Cancelar</button>
        <button class="modal-confirm" type="button" onclick={confirmTrackRemoval} disabled={deletingTrack}>
          {deletingTrack ? 'Removendo…' : 'Remover da biblioteca'}
        </button>
      </div>
    </div>
  </div>
{/if}

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
    justify-content: space-between;
    gap: 12px;
  }
  .brand, .header-actions { display: flex; align-items: center; gap: 12px; }
  .logo-sm { width: 28px; height: 28px; object-fit: contain; }
  .app-name { font-size: 15px; font-weight: 500; color: #8a8aa0; letter-spacing: 0.01em; }
  .btn-import, .btn-back, .btn-tags {
    padding: 8px 12px; border-radius: 7px; font: 500 12px 'Space Grotesk', sans-serif;
    cursor: pointer;
  }
  .btn-import { background: #c8861e; color: #0c0c12; border: 0; }
  .btn-import:hover { background: #d9971f; }
  .btn-back, .btn-tags { background: #151522; color: #88889b; border: 1px solid #29293a; }
  .btn-back:hover:not(:disabled) { color: #c8861e; border-color: #6a4a22; }
  .btn-back:disabled, .btn-tags:disabled { opacity: 0.4; cursor: not-allowed; }

  /* ── Library ── */
  .library-view { flex: 1; min-height: 0; display: flex; flex-direction: column; gap: 16px; }
  .home-tabs { display: flex; gap: 4px; border-bottom: 1px solid #20202e; }
  .home-tabs button {
    display: flex; align-items: center; gap: 7px; padding: 10px 14px;
    background: transparent; color: #55556a; border: 0; border-bottom: 2px solid transparent;
    font: 500 13px 'Space Grotesk', sans-serif; cursor: pointer;
  }
  .home-tabs button.active { color: #d4d0c7; border-bottom-color: #c8861e; }
  .home-tabs button span {
    min-width: 18px; padding: 2px 5px; border-radius: 999px;
    background: #1a1a28; color: #77778c; font: 10px 'JetBrains Mono', monospace;
  }
  .library-toolbar { display: flex; align-items: center; gap: 14px; }
  .library-toolbar input {
    flex: 1; min-width: 0; padding: 10px 13px;
    background: #131320; color: #d8d4cb; border: 1px solid #242435; border-radius: 8px;
    outline: none; font: 12px 'Space Grotesk', sans-serif;
  }
  .library-toolbar input:focus { border-color: #c8861e; }
  .library-toolbar > span { color: #45455a; font: 11px 'JetBrains Mono', monospace; white-space: nowrap; }
  .library-state { flex: 1; display: flex; align-items: center; justify-content: center; gap: 9px; color: #5e5e73; font-size: 13px; }
  .library-empty { min-height: 300px; }
  .track-grid {
    display: grid; grid-template-columns: repeat(auto-fill, minmax(310px, 1fr));
    gap: 12px; align-content: start;
  }
  .track-card {
    display: flex; align-items: center; gap: 12px; min-width: 0; padding: 14px;
    background: #131320; border: 1px solid #20202f; border-radius: 9px;
    cursor: pointer; outline: none; transition: border-color 140ms, transform 140ms, background 140ms;
  }
  .track-card:hover, .track-card:focus-visible {
    background: #161624; border-color: #664820; transform: translateY(-1px);
  }
  .track-card-icon {
    display: grid; place-items: center; width: 38px; height: 38px; flex: 0 0 auto;
    background: rgba(200, 134, 30, 0.1); color: #c8861e; border-radius: 8px; font-size: 18px;
  }
  .track-card-body { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 5px; }
  .track-card-body strong { color: #d8d4cb; font-size: 14px; font-weight: 600; }
  .track-path {
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    color: #4b4b60; font: 10px 'JetBrains Mono', monospace;
  }
  .track-card-meta { display: flex; flex-wrap: wrap; gap: 5px 10px; color: #67677c; font-size: 10px; }
  .track-delete {
    align-self: flex-start; flex: 0 0 auto; width: 24px; height: 24px;
    background: transparent; color: #45455a; border: 0; border-radius: 5px;
    cursor: pointer; font-size: 17px;
  }
  .track-delete:hover { background: rgba(200, 80, 80, 0.1); color: #d06b6b; }
  .coming-soon {
    flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center;
    gap: 9px; min-height: 300px; color: #4b4b60; text-align: center;
  }
  .coming-soon > span { color: #c8861e; font-size: 34px; opacity: 0.55; }
  .coming-soon strong { color: #848496; font-size: 15px; }
  .coming-soon p { max-width: 440px; font-size: 12px; line-height: 1.55; }

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

  .advanced-settings {
    background: #11111c; border: 1px solid #1e1e2c; border-radius: 8px;
  }
  .advanced-settings summary {
    display: flex; align-items: center; justify-content: space-between; gap: 12px;
    padding: 11px 13px; cursor: pointer; list-style: none;
    color: #77778e; font-size: 13px; user-select: none;
  }
  .advanced-settings summary::-webkit-details-marker { display: none; }
  .advanced-settings summary::before {
    content: '›'; color: #c8861e; font-size: 18px; line-height: 1;
    transition: transform 140ms ease;
  }
  .advanced-settings[open] summary::before { transform: rotate(90deg); }
  .advanced-settings summary > :first-child { margin-right: auto; }
  .range-mode {
    padding: 3px 7px; border-radius: 999px;
    background: rgba(90, 150, 105, 0.12); color: #6d9d77;
    font: 10px 'JetBrains Mono', monospace;
  }
  .range-mode.manual { background: rgba(200, 134, 30, 0.12); color: #c8861e; }
  .advanced-content {
    display: flex; flex-direction: column; gap: 12px;
    padding: 4px 13px 13px; border-top: 1px solid #1b1b29;
  }
  .advanced-grid {
    display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px;
    padding-top: 12px;
  }
  .advanced-grid label { display: flex; flex-direction: column; gap: 6px; }
  .advanced-grid label span { color: #50506a; font-size: 11px; }
  .advanced-grid input, .advanced-grid select {
    width: 100%; height: 36px; padding: 8px 10px;
    background: #171724; color: #d5d1c9; border: 1px solid #272738; border-radius: 6px;
    outline: none; font: 12px 'JetBrains Mono', monospace;
  }
  .advanced-grid select {
    appearance: none; color-scheme: dark; padding-right: 32px;
    background-color: #171724;
    background-image:
      linear-gradient(45deg, transparent 50%, #8b7653 50%),
      linear-gradient(135deg, #8b7653 50%, transparent 50%);
    background-position:
      calc(100% - 15px) 15px,
      calc(100% - 10px) 15px;
    background-size: 5px 5px, 5px 5px;
    background-repeat: no-repeat;
  }
  .advanced-grid select option { background: #171724; color: #d5d1c9; }
  .advanced-grid input:focus, .advanced-grid select:focus { border-color: #c8861e; }
  .advanced-grid input:disabled, .advanced-grid select:disabled { opacity: 0.5; cursor: not-allowed; }
  .detected-range { display: flex; align-items: center; justify-content: space-between; gap: 12px; }
  .detected-range span { color: #4d4d63; font-size: 11px; }
  .detected-range strong { color: #85859a; font-family: 'JetBrains Mono', monospace; }
  .detected-range button {
    padding: 5px 8px; background: transparent; color: #8b7653;
    border: 1px solid #443a2c; border-radius: 5px; cursor: pointer; font-size: 11px;
  }
  .detected-range button:hover:not(:disabled) { border-color: #c8861e; color: #c8861e; }
  .detected-range button:disabled { opacity: 0.35; cursor: not-allowed; }
  .advanced-help { color: #3d3d50; font-size: 11px; line-height: 1.45; }

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
  .render-summary { color: #38384a; font: 11px 'JetBrains Mono', monospace; }
  .save-config-row {
    display: flex; align-items: center; justify-content: space-between; gap: 14px;
    padding: 10px 12px; background: rgba(82, 132, 93, 0.06);
    border: 1px solid #26372b; border-radius: 7px;
  }
  .save-config-row.dirty { background: rgba(200, 134, 30, 0.06); border-color: #4b3a22; }
  .save-config-row > div { display: flex; flex-direction: column; gap: 2px; }
  .save-config-row strong { color: #7d9a82; font-size: 11px; }
  .save-config-row.dirty strong { color: #c8861e; }
  .save-config-row span { color: #45455a; font-size: 10px; }
  .save-config-row button {
    padding: 7px 10px; white-space: nowrap; background: #1b1b28; color: #c8861e;
    border: 1px solid #5a421f; border-radius: 6px; cursor: pointer; font-size: 11px;
  }
  .save-config-row button:disabled { opacity: 0.35; cursor: not-allowed; }

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
  .player-loop-info {
    margin-bottom: 9px; color: #50506a;
    font: 11px 'JetBrains Mono', monospace;
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

  /* ── Confirmation modal ── */
  .modal-backdrop {
    position: fixed; inset: 0; z-index: 1000;
    display: grid; place-items: center; padding: 24px;
    background: rgba(5, 5, 9, 0.78); backdrop-filter: blur(4px);
  }
  .modal-dismiss {
    position: absolute; inset: 0; width: 100%; height: 100%;
    background: transparent; border: 0; cursor: default;
  }
  .confirm-modal {
    position: relative; z-index: 1;
    width: min(440px, 100%); padding: 22px;
    background: #151520; border: 1px solid #303043; border-radius: 12px;
    box-shadow: 0 24px 80px rgba(0, 0, 0, 0.55);
  }
  .modal-icon {
    display: grid; place-items: center; width: 38px; height: 38px; margin-bottom: 15px;
    border-radius: 50%; background: rgba(200, 80, 80, 0.12); color: #d66f6f;
    font-size: 23px; line-height: 1;
  }
  .modal-copy { display: flex; flex-direction: column; gap: 8px; }
  .modal-copy h2 { color: #e0dcd4; font-size: 18px; font-weight: 600; }
  .modal-copy p { color: #77778c; font-size: 13px; line-height: 1.5; }
  .modal-copy p strong { color: #c7c3ba; }
  .modal-copy .modal-note { color: #56566b; font-size: 12px; }
  .modal-copy code { color: #c8861e; font-family: 'JetBrains Mono', monospace; }
  .modal-actions { display: flex; justify-content: flex-end; gap: 9px; margin-top: 22px; }
  .modal-actions button {
    padding: 9px 13px; border-radius: 7px;
    font: 500 12px 'Space Grotesk', sans-serif; cursor: pointer;
  }
  .modal-cancel { background: #1c1c2a; color: #9999aa; border: 1px solid #303043; }
  .modal-cancel:hover:not(:disabled) { border-color: #4b4b61; }
  .modal-confirm { background: #b94f4f; color: #fff; border: 1px solid #c75a5a; }
  .modal-confirm:hover:not(:disabled) { background: #ca5858; }
  .modal-actions button:disabled { opacity: 0.5; cursor: wait; }

  @media (prefers-reduced-motion: reduce) {
    .spinner { animation: none; }
    .drop-zone, .logo-watermark { transition: none; }
  }
</style>
