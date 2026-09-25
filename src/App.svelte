<script lang="ts">
  import { onMount } from 'svelte'
  import { open } from '@tauri-apps/plugin-dialog'
  import { convertFileSrc, invoke } from '@tauri-apps/api/core'
  import { formatDuration, type StrudelMeta } from './lib/loop-parser'
  import { analyzeStrudel, renderToUrl, exportToWav, type RenderSettings } from './lib/strudel-runner'
  import {
    createTag,
    createAlbum,
    addAlbumTrack,
    deleteAlbum,
    deleteTag,
    deleteTrack as deleteLibraryTrack,
    importTrack as importLibraryTrack,
    listTags,
    listAlbums,
    getAlbum,
    listTracks,
    saveTrackSettings,
    setTrackTag,
    removeAlbumTrack,
    reorderAlbumTracks,
    updateAlbum,
    updateAlbumTrack,
    updateTag,
    type LibraryTag,
    type AlbumSummary,
    type AlbumTrackItem,
    type AlbumTrackSettings,
    type LibraryAlbum,
    type LibraryTrack,
    type SavedTrackSettings,
  } from './lib/library'
  import { exportAlbumToWav } from './lib/album-exporter'

  type AppStatus = 'idle' | 'analyzing' | 'rendering_preview' | 'rendering_export' | 'error'
  type AppScreen = 'library' | 'track' | 'album'
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
  let tags = $state<LibraryTag[]>([])
  let selectedTagIds = $state<number[]>([])
  let tagManagerOpen = $state(false)
  let tagFormName = $state('')
  let tagFormColor = $state('#C8861E')
  let editingTagId = $state<number | null>(null)
  let tagSaving = $state(false)
  let tagError = $state('')
  let tagPendingDelete = $state<LibraryTag | null>(null)
  let tagLinkSavingId = $state<number | null>(null)
  let albums = $state<AlbumSummary[]>([])
  let activeAlbum = $state<LibraryAlbum | null>(null)
  let albumModalOpen = $state(false)
  let editingAlbumId = $state<number | null>(null)
  let albumName = $state('')
  let albumDescription = $state('')
  let albumCoverPath = $state<string | null>(null)
  let albumSampleRate = $state<44100 | 48000 | 96000>(44100)
  let albumDefaultGap = $state(0)
  let albumSaving = $state(false)
  let albumError = $state('')
  let albumPendingDelete = $state<AlbumSummary | null>(null)
  let albumTrackPickerOpen = $state(false)
  let albumItemSavingId = $state<number | null>(null)
  let albumPreviewItemId = $state<number | null>(null)
  let albumCpsByItem = $state<Record<number, number>>({})
  let albumItemPendingRemove = $state<AlbumTrackItem | null>(null)

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
      const matchesQuery = !query || track.name.toLocaleLowerCase().includes(query) || track.sourcePath.toLocaleLowerCase().includes(query)
      const matchesTags = selectedTagIds.every((tagId) => track.tags.some((tag) => tag.id === tagId))
      return matchesQuery && matchesTags
    }),
  )
  let settingsDirty = $derived(activeTrackId !== null && savedSettingsKey !== serializedSettings())
  let albumDuration = $derived(
    activeAlbum?.tracks.reduce((total, item) => {
      const cps = albumCpsByItem[item.id]
      return total + (cps ? ((item.settings.endCycle - item.settings.startCycle) / cps) * item.settings.loops : 0) + item.settings.gapAfterSeconds
    }, 0) ?? 0,
  )

  onMount(() => {
    void refreshTracks()
  })

  async function refreshTracks() {
    libraryLoading = true
    try {
      const [loadedTracks, loadedTags, loadedAlbums] = await Promise.all([listTracks(), listTags(), listAlbums()])
      tracks = loadedTracks
      tags = loadedTags
      albums = loadedAlbums
      selectedTagIds = selectedTagIds.filter((tagId) => loadedTags.some((tag) => tag.id === tagId))
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

  function handleEscape() {
    if (albumItemPendingRemove && albumItemSavingId === null) {
      albumItemPendingRemove = null
    } else if (albumPendingDelete && !albumSaving) {
      albumPendingDelete = null
    } else if (albumTrackPickerOpen) {
      albumTrackPickerOpen = false
    } else if (albumModalOpen) {
      closeAlbumModal()
    } else if (tagPendingDelete && !tagSaving) {
      tagPendingDelete = null
    } else if (tagManagerOpen) {
      closeTagManager()
    } else {
      cancelTrackRemoval()
    }
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

  function toggleTagFilter(tagId: number) {
    selectedTagIds = selectedTagIds.includes(tagId)
      ? selectedTagIds.filter((id) => id !== tagId)
      : [...selectedTagIds, tagId]
  }

  function openTagManager() {
    tagManagerOpen = true
    tagError = ''
    resetTagForm()
  }

  function closeTagManager() {
    if (tagSaving) return
    tagManagerOpen = false
    tagPendingDelete = null
    resetTagForm()
  }

  function resetTagForm() {
    editingTagId = null
    tagFormName = ''
    tagFormColor = '#C8861E'
  }

  function editTag(tag: LibraryTag) {
    editingTagId = tag.id
    tagFormName = tag.name
    tagFormColor = tag.color
    tagError = ''
  }

  async function submitTag() {
    if (tagSaving) return
    tagSaving = true
    tagError = ''
    try {
      if (editingTagId === null) {
        await createTag(tagFormName, tagFormColor)
      } else {
        await updateTag(editingTagId, tagFormName, tagFormColor)
      }
      resetTagForm()
      await refreshTracks()
    } catch (err) {
      tagError = String(err)
    } finally {
      tagSaving = false
    }
  }

  async function confirmTagRemoval() {
    if (!tagPendingDelete || tagSaving) return
    tagSaving = true
    tagError = ''
    try {
      await deleteTag(tagPendingDelete.id)
      tagPendingDelete = null
      resetTagForm()
      await refreshTracks()
    } catch (err) {
      tagError = String(err)
    } finally {
      tagSaving = false
    }
  }

  async function toggleActiveTrackTag(tag: LibraryTag) {
    if (activeTrackId === null || tagLinkSavingId !== null) return
    const activeTrack = tracks.find((track) => track.id === activeTrackId)
    if (!activeTrack) return
    const attached = activeTrack.tags.some((item) => item.id === tag.id)
    tagLinkSavingId = tag.id
    try {
      const updated = await setTrackTag(activeTrackId, tag.id, !attached)
      tracks = tracks.map((track) => track.id === updated.id ? updated : track)
      tags = await listTags()
    } catch (err) {
      errorMsg = `Erro ao atualizar Tag: ${err}`
      appStatus = 'error'
    } finally {
      tagLinkSavingId = null
    }
  }

  function resetAlbumForm() {
    editingAlbumId = null
    albumName = ''
    albumDescription = ''
    albumCoverPath = null
    albumSampleRate = 44100
    albumDefaultGap = 0
    albumError = ''
  }

  function openAlbumCreator() {
    resetAlbumForm()
    albumModalOpen = true
  }

  function openAlbumEditor(album: AlbumSummary | LibraryAlbum) {
    editingAlbumId = album.id
    albumName = album.name
    albumDescription = album.description
    albumCoverPath = album.coverPath
    albumSampleRate = album.sampleRate
    albumDefaultGap = album.defaultGapSeconds
    albumError = ''
    albumModalOpen = true
  }

  function closeAlbumModal() {
    if (!albumSaving) albumModalOpen = false
  }

  async function chooseAlbumCover() {
    const selected = await open({
      filters: [{ name: 'Imagem', extensions: ['png', 'jpg', 'jpeg', 'webp'] }],
      multiple: false,
    })
    if (typeof selected === 'string') albumCoverPath = selected
  }

  async function submitAlbum() {
    if (albumSaving || !albumName.trim()) return
    albumSaving = true
    albumError = ''
    try {
      const fields = {
        name: albumName,
        description: albumDescription,
        coverPath: albumCoverPath,
        sampleRate: albumSampleRate,
        defaultGapSeconds: Math.max(0, Number(albumDefaultGap) || 0),
      }
      const album = editingAlbumId === null
        ? await createAlbum(fields)
        : await updateAlbum(editingAlbumId, fields)
      albumModalOpen = false
      await refreshTracks()
      if (screen === 'album' && activeAlbum?.id === album.id) activeAlbum = album
      if (editingAlbumId === null) await openAlbum(album)
    } catch (err) {
      albumError = String(err)
    } finally {
      albumSaving = false
    }
  }

  async function openAlbum(album: AlbumSummary | LibraryAlbum) {
    try {
      clearPreview()
      albumPreviewItemId = null
      albumCpsByItem = {}
      activeAlbum = 'tracks' in album ? album : await getAlbum(album.id)
      screen = 'album'
      appStatus = 'idle'
      errorMsg = ''
      void analyzeAlbumTracks()
    } catch (err) {
      libraryError = `Erro ao abrir Album: ${err}`
    }
  }

  function closeAlbum() {
    renderController?.abort()
    clearPreview()
    activeAlbum = null
    albumPreviewItemId = null
    albumCpsByItem = {}
    screen = 'library'
    void refreshTracks()
  }

  async function analyzeAlbumTracks() {
    const album = activeAlbum
    if (!album) return
    for (const item of album.tracks) {
      try {
        const code = await invoke<string>('read_strudel_file', { path: item.track.sourcePath })
        const analysis = await analyzeStrudel(code)
        if (activeAlbum?.id !== album.id) return
        albumCpsByItem = { ...albumCpsByItem, [item.id]: analysis.cps }
      } catch {
        // The export/preview action will expose the concrete source error.
      }
    }
  }

  function albumItemDuration(item: AlbumTrackItem) {
    const cps = albumCpsByItem[item.id]
    return cps ? ((item.settings.endCycle - item.settings.startCycle) / cps) * item.settings.loops : 0
  }

  async function addTrackToActiveAlbum(trackId: number) {
    if (!activeAlbum || albumSaving) return
    albumSaving = true
    try {
      activeAlbum = await addAlbumTrack(activeAlbum.id, trackId)
      albumTrackPickerOpen = false
      await refreshTracks()
      void analyzeAlbumTracks()
    } catch (err) {
      errorMsg = `Erro ao adicionar Track: ${err}`
      appStatus = 'error'
    } finally {
      albumSaving = false
    }
  }

  async function saveAlbumItem(item: AlbumTrackItem) {
    if (!activeAlbum || albumItemSavingId !== null) return
    item.settings.startCycle = Math.max(0, Number(item.settings.startCycle) || 0)
    item.settings.endCycle = Math.max(item.settings.startCycle + 0.25, Number(item.settings.endCycle) || item.settings.startCycle + 1)
    item.settings.loops = Math.min(999, Math.max(1, Math.round(Number(item.settings.loops) || 1)))
    item.settings.maxPolyphony = Math.min(256, Math.max(1, Math.round(Number(item.settings.maxPolyphony) || 32)))
    item.settings.gapAfterSeconds = Math.max(0, Number(item.settings.gapAfterSeconds) || 0)
    albumItemSavingId = item.id
    try {
      activeAlbum = await updateAlbumTrack(item.id, item.settings)
    } catch (err) {
      errorMsg = `Erro ao salvar faixa do Album: ${err}`
      appStatus = 'error'
    } finally {
      albumItemSavingId = null
    }
  }

  async function restoreAlbumItemDefaults(item: AlbumTrackItem) {
    item.settings = {
      rangeMode: item.track.settings.rangeMode,
      startCycle: item.track.settings.startCycle,
      endCycle: item.track.settings.endCycle,
      loops: item.track.settings.loops,
      maxPolyphony: item.track.settings.maxPolyphony,
      gapAfterSeconds: activeAlbum?.defaultGapSeconds ?? 0,
    }
    await saveAlbumItem(item)
  }

  async function moveAlbumItem(index: number, direction: -1 | 1) {
    if (!activeAlbum || albumItemSavingId !== null) return
    const target = index + direction
    if (target < 0 || target >= activeAlbum.tracks.length) return
    const ordered = [...activeAlbum.tracks]
    ;[ordered[index], ordered[target]] = [ordered[target], ordered[index]]
    albumItemSavingId = ordered[target].id
    try {
      activeAlbum = await reorderAlbumTracks(activeAlbum.id, ordered.map((item) => item.id))
    } catch (err) {
      errorMsg = `Erro ao reordenar Album: ${err}`
      appStatus = 'error'
    } finally {
      albumItemSavingId = null
    }
  }

  async function confirmAlbumItemRemoval() {
    if (!albumItemPendingRemove || albumItemSavingId !== null) return
    albumItemSavingId = albumItemPendingRemove.id
    try {
      activeAlbum = await removeAlbumTrack(albumItemPendingRemove.id)
      albumItemPendingRemove = null
      await refreshTracks()
    } catch (err) {
      errorMsg = `Erro ao remover faixa do Album: ${err}`
      appStatus = 'error'
    } finally {
      albumItemSavingId = null
    }
  }

  async function generateAlbumItemPreview(item: AlbumTrackItem) {
    if (!activeAlbum || isBusy) return
    clearPreview()
    albumPreviewItemId = item.id
    previewLoops = item.settings.loops
    renderProgress = 0
    progressLabel = `Preparando ${item.track.name}…`
    cancelRequested = false
    const controller = new AbortController()
    renderController = controller
    appStatus = 'rendering_preview'
    try {
      const code = await invoke<string>('read_strudel_file', { path: item.track.sourcePath })
      previewUrl = await renderToUrl(
        code,
        item.settings.loops,
        {
          startCycle: item.settings.startCycle,
          endCycle: item.settings.endCycle,
          sampleRate: activeAlbum.sampleRate,
          maxPolyphony: item.settings.maxPolyphony,
        },
        updateProgress,
        controller.signal,
      )
      appStatus = 'idle'
    } catch (err) {
      if (controller.signal.aborted) appStatus = 'idle'
      else {
        appStatus = 'error'
        errorMsg = `Erro no preview do Album: ${err}`
      }
    } finally {
      if (renderController === controller) renderController = null
      cancelRequested = false
    }
  }

  async function exportActiveAlbum() {
    if (!activeAlbum || isBusy) return
    renderProgress = 0
    progressLabel = 'Iniciando Album…'
    cancelRequested = false
    const controller = new AbortController()
    renderController = controller
    appStatus = 'rendering_export'
    try {
      await exportAlbumToWav(activeAlbum, updateProgress, controller.signal)
      appStatus = 'idle'
    } catch (err) {
      if (controller.signal.aborted) appStatus = 'idle'
      else {
        appStatus = 'error'
        errorMsg = `Erro ao exportar Album: ${err}`
      }
    } finally {
      if (renderController === controller) renderController = null
      cancelRequested = false
    }
  }

  async function confirmAlbumRemoval() {
    if (!albumPendingDelete || albumSaving) return
    albumSaving = true
    try {
      await deleteAlbum(albumPendingDelete.id)
      if (activeAlbum?.id === albumPendingDelete.id) closeAlbum()
      albumPendingDelete = null
      await refreshTracks()
    } catch (err) {
      libraryError = `Erro ao excluir Album: ${err}`
    } finally {
      albumSaving = false
    }
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

<svelte:window onkeydown={(event) => event.key === 'Escape' && handleEscape()} />

<main>
  <header>
    <div class="brand">
      <img src="/assets/logo.png" alt="Strudel logo" class="logo-sm" />
      <span class="app-name">Strudel Library</span>
    </div>
    {#if screen === 'library'}
      <div class="header-actions">
        <button class="btn-tags" type="button" onclick={openTagManager}>Tags</button>
        {#if homeTab === 'tracks'}
          <button class="btn-import" type="button" onclick={pickFile}>+ Importar Track</button>
        {:else}
          <button class="btn-import" type="button" onclick={openAlbumCreator}>+ Novo Album</button>
        {/if}
      </div>
    {:else}
      <button class="btn-back" type="button" onclick={screen === 'track' ? closeTrack : closeAlbum} disabled={isBusy}>← Biblioteca</button>
    {/if}
  </header>

  {#if screen === 'library'}
    <section class="library-view">
      <div class="home-tabs" role="tablist" aria-label="Biblioteca">
        <button class:active={homeTab === 'tracks'} role="tab" aria-selected={homeTab === 'tracks'} onclick={() => homeTab = 'tracks'}>
          Tracks <span>{tracks.length}</span>
        </button>
        <button class:active={homeTab === 'albums'} role="tab" aria-selected={homeTab === 'albums'} onclick={() => homeTab = 'albums'}>
          Albums <span>{albums.length}</span>
        </button>
      </div>

      {#if homeTab === 'tracks'}
        <div class="library-toolbar">
          <input bind:value={trackSearch} type="search" placeholder="Buscar por nome ou caminho…" aria-label="Buscar Tracks" />
          <span>{visibleTracks.length} {visibleTracks.length === 1 ? 'Track' : 'Tracks'}</span>
        </div>

        {#if tags.length > 0}
          <div class="tag-filters" aria-label="Filtrar por tags">
            <span>Filtrar:</span>
            {#each tags as tag (tag.id)}
              <button
                type="button"
                class:active={selectedTagIds.includes(tag.id)}
                style={`--tag-color: ${tag.color}`}
                onclick={() => toggleTagFilter(tag.id)}
              >
                <i></i>{tag.name} <small>{tag.trackCount}</small>
              </button>
            {/each}
            {#if selectedTagIds.length > 0}
              <button class="clear-tags" type="button" onclick={() => selectedTagIds = []}>Limpar</button>
            {/if}
          </div>
        {/if}

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
                  {#if track.tags.length > 0}
                    <div class="track-badges">
                      {#each track.tags as tag (tag.id)}
                        <span style={`--tag-color: ${tag.color}`}><i></i>{tag.name}</span>
                      {/each}
                    </div>
                  {/if}
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
        {#if albums.length === 0}
          <div class="coming-soon">
            <span>◎</span>
            <strong>Nenhum Album criado</strong>
            <p>Crie um Album para organizar Tracks, definir a ordem e exportar um WAV completo.</p>
            <button class="btn-import" type="button" onclick={openAlbumCreator}>+ Criar primeiro Album</button>
          </div>
        {:else}
          <div class="album-grid">
            {#each albums as album (album.id)}
              <div class="album-card" role="button" tabindex="0" onclick={() => openAlbum(album)} onkeydown={(event) => event.key === 'Enter' && openAlbum(album)}>
                <div class="album-cover">
                  {#if album.coverPath}
                    <img src={convertFileSrc(album.coverPath)} alt={`Capa de ${album.name}`} />
                  {:else}
                    <span>◎</span>
                  {/if}
                </div>
                <div class="album-card-copy">
                  <strong>{album.name}</strong>
                  <p>{album.description || 'Sem descrição'}</p>
                  {#if album.tags.length > 0}
                    <div class="track-badges">
                      {#each album.tags as tag (tag.id)}
                        <span style={`--tag-color: ${tag.color}`}><i></i>{tag.name}</span>
                      {/each}
                    </div>
                  {/if}
                  <div><span>{album.trackCount} {album.trackCount === 1 ? 'Track' : 'Tracks'}</span><span>{(album.sampleRate / 1000).toFixed(1)} kHz</span></div>
                </div>
                <button class="track-delete" type="button" onclick={(event) => { event.stopPropagation(); albumPendingDelete = album }} aria-label={`Excluir ${album.name}`}>×</button>
              </div>
            {/each}
          </div>
        {/if}
      {/if}

      {#if libraryError}
        <div class="error-banner">
          <span>{libraryError}</span>
          <button onclick={() => libraryError = ''}>×</button>
        </div>
      {/if}
    </section>
  {:else if screen === 'track'}
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
      <div class="track-tags-editor">
        <span class="track-tags-label">Tags</span>
        {#if tags.length === 0}
          <button type="button" class="create-first-tag" onclick={openTagManager}>Criar primeira Tag</button>
        {:else}
          <div class="track-tag-options">
            {#each tags as tag (tag.id)}
              {@const attached = tracks.find((track) => track.id === activeTrackId)?.tags.some((item) => item.id === tag.id) ?? false}
              <button
                type="button"
                class:attached
                style={`--tag-color: ${tag.color}`}
                disabled={isBusy || tagLinkSavingId !== null}
                onclick={() => toggleActiveTrackTag(tag)}
              >
                <i></i>{tag.name}{attached ? ' ×' : ' +'}
              </button>
            {/each}
          </div>
        {/if}
      </div>
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
  {:else if activeAlbum}
    <section class="album-detail">
      <div class="album-detail-header">
        <div class="album-detail-cover">
          {#if activeAlbum.coverPath}
            <img src={convertFileSrc(activeAlbum.coverPath)} alt={`Capa de ${activeAlbum.name}`} />
          {:else}
            <span aria-hidden="true">♫</span>
          {/if}
        </div>
        <div class="album-detail-copy">
          <span class="album-eyebrow">ALBUM</span>
          <h1>{activeAlbum.name}</h1>
          <p>{activeAlbum.description || 'Sem descrição.'}</p>
          <div class="album-detail-meta">
            <span>{activeAlbum.tracks.length} {activeAlbum.tracks.length === 1 ? 'faixa' : 'faixas'}</span>
            <span>{activeAlbum.sampleRate.toLocaleString('pt-BR')} Hz</span>
            <span>{formatDuration(albumDuration)}</span>
          </div>
        </div>
        <div class="album-header-actions">
          <button type="button" onclick={() => openAlbumEditor(activeAlbum!)} disabled={isBusy}>Editar Album</button>
          <button class="album-add-track" type="button" onclick={() => albumTrackPickerOpen = true} disabled={isBusy}>+ Adicionar Track</button>
        </div>
      </div>

      {#if activeAlbum.tracks.length === 0}
        <div class="album-empty">
          <span aria-hidden="true">＋</span>
          <strong>Este Album ainda está vazio</strong>
          <p>Adicione Tracks da biblioteca. A mesma Track pode aparecer mais de uma vez.</p>
          <button type="button" onclick={() => albumTrackPickerOpen = true}>Adicionar primeira Track</button>
        </div>
      {:else}
        <div class="album-track-list">
          {#each activeAlbum.tracks as item, index (item.id)}
            <article class="album-track-item">
              <div class="album-track-heading">
                <span class="album-track-position">{String(index + 1).padStart(2, '0')}</span>
                <div class="album-track-copy">
                  <strong>{item.track.name}</strong>
                  <span>{item.track.sourcePath}</span>
                  {#if item.track.tags.length > 0}
                    <div class="track-badges">
                      {#each item.track.tags as tag (tag.id)}
                        <span style={`--tag-color: ${tag.color}`}><i></i>{tag.name}</span>
                      {/each}
                    </div>
                  {/if}
                </div>
                <div class="album-track-duration">
                  <strong>{formatDuration(albumItemDuration(item))}</strong>
                  {#if item.settings.gapAfterSeconds > 0}<span>+ {item.settings.gapAfterSeconds}s pausa</span>{/if}
                </div>
                <div class="album-order-actions">
                  <button type="button" aria-label="Mover faixa para cima" onclick={() => moveAlbumItem(index, -1)} disabled={index === 0 || isBusy || albumItemSavingId !== null}>↑</button>
                  <button type="button" aria-label="Mover faixa para baixo" onclick={() => moveAlbumItem(index, 1)} disabled={index === activeAlbum!.tracks.length - 1 || isBusy || albumItemSavingId !== null}>↓</button>
                </div>
                <button class="album-remove-track" type="button" aria-label="Remover faixa do Album" onclick={() => albumItemPendingRemove = item} disabled={isBusy || albumItemSavingId !== null}>×</button>
              </div>

              <div class="album-item-settings">
                <label>
                  <span>Start cycle</span>
                  <input type="number" min="0" step="0.25" bind:value={item.settings.startCycle} onchange={() => { item.settings.rangeMode = 'manual'; void saveAlbumItem(item) }} disabled={isBusy || albumItemSavingId !== null} />
                </label>
                <label>
                  <span>End cycle</span>
                  <input type="number" min="0.25" step="0.25" bind:value={item.settings.endCycle} onchange={() => { item.settings.rangeMode = 'manual'; void saveAlbumItem(item) }} disabled={isBusy || albumItemSavingId !== null} />
                </label>
                <label>
                  <span>Loops</span>
                  <input type="number" min="1" max="999" step="1" bind:value={item.settings.loops} onchange={() => saveAlbumItem(item)} disabled={isBusy || albumItemSavingId !== null} />
                </label>
                <label>
                  <span>Polifonia</span>
                  <input type="number" min="1" max="256" step="1" bind:value={item.settings.maxPolyphony} onchange={() => saveAlbumItem(item)} disabled={isBusy || albumItemSavingId !== null} />
                </label>
                <label>
                  <span>Pausa depois (s)</span>
                  <input type="number" min="0" max="3600" step="0.1" bind:value={item.settings.gapAfterSeconds} onchange={() => saveAlbumItem(item)} disabled={isBusy || albumItemSavingId !== null} />
                </label>
                <div class="album-item-actions">
                  <button type="button" onclick={() => restoreAlbumItemDefaults(item)} disabled={isBusy || albumItemSavingId !== null}>Restaurar Track</button>
                  <button class="album-item-preview" type="button" onclick={() => generateAlbumItemPreview(item)} disabled={isBusy || albumItemSavingId !== null}>
                    {albumPreviewItemId === item.id && appStatus === 'rendering_preview' ? 'Gerando…' : '▶ Preview'}
                  </button>
                </div>
              </div>

              {#if albumPreviewItemId === item.id && hasPreview}
                <div class="player album-player">
                  <div class="player-loop-info">Preview desta faixa × {previewLoops}</div>
                  <div class="player-controls">
                    <button class="player-btn" onclick={togglePlay} aria-label={audioPlaying ? 'Pausar' : 'Reproduzir'}>
                      {audioPlaying ? 'Ⅱ' : '▶'}
                    </button>
                    <button class="player-btn" onclick={stopAudio} aria-label="Parar">■</button>
                    <span class="player-time">{formatTime(audioTime)}</span>
                    <input class="player-seek" type="range" min="0" max={audioDuration || 0} step="0.01" value={audioTime} oninput={seek} aria-label="Posição" />
                    <span class="player-time player-time-total">{formatTime(audioDuration)}</span>
                  </div>
                </div>
              {/if}
            </article>
          {/each}
        </div>
      {/if}

      {#if isRendering}
        <div class="render-progress album-render-progress">
          <div class="progress-copy"><span>{progressLabel}</span><span>{Math.round(renderProgress * 100)}%</span></div>
          <div class="progress-track" role="progressbar" aria-label="Geração do Album" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(renderProgress * 100)}>
            <div class="progress-fill" style={`width: ${Math.round(renderProgress * 100)}%`}></div>
          </div>
          <button class="btn-cancel" type="button" onclick={cancelGeneration} disabled={cancelRequested}>{cancelRequested ? 'Cancelando…' : 'Cancelar geração'}</button>
        </div>
      {/if}

      {#if appStatus === 'error'}
        <div class="error-banner"><span>{errorMsg}</span><button onclick={() => { appStatus = 'idle'; errorMsg = '' }}>×</button></div>
      {/if}

      <div class="album-export-bar">
        <div>
          <strong>Exportar Album completo</strong>
          <span>WAV único + manifesto JSON com timeline, Tags e configurações.</span>
        </div>
        <button class="btn-export" type="button" onclick={exportActiveAlbum} disabled={isBusy || activeAlbum.tracks.length === 0}>
          {appStatus === 'rendering_export' ? 'Renderizando Album…' : 'Exportar Album WAV'}
        </button>
      </div>
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

{#if tagManagerOpen}
  <div class="modal-backdrop">
    <button class="modal-dismiss" type="button" onclick={closeTagManager} aria-label="Fechar gerenciador de Tags"></button>
    <div class="confirm-modal tag-modal" role="dialog" aria-modal="true" aria-labelledby="tag-manager-title">
      {#if tagPendingDelete}
        <div class="modal-icon" aria-hidden="true">×</div>
        <div class="modal-copy">
          <h2 id="tag-manager-title">Excluir Tag?</h2>
          <p>
            A Tag <strong>{tagPendingDelete.name}</strong> será removida de {tagPendingDelete.trackCount}
            {tagPendingDelete.trackCount === 1 ? ' Track' : ' Tracks'}.
          </p>
          <p class="modal-note">Nenhuma Track ou arquivo será excluído.</p>
        </div>
        <div class="modal-actions">
          <button class="modal-cancel" type="button" onclick={() => tagPendingDelete = null} disabled={tagSaving}>Cancelar</button>
          <button class="modal-confirm" type="button" onclick={confirmTagRemoval} disabled={tagSaving}>
            {tagSaving ? 'Excluindo…' : 'Excluir Tag'}
          </button>
        </div>
      {:else}
        <div class="tag-modal-header">
          <div>
            <h2 id="tag-manager-title">Gerenciar Tags</h2>
            <p>Nomes, cores e uso nas Tracks.</p>
          </div>
          <button type="button" onclick={closeTagManager} aria-label="Fechar">×</button>
        </div>

        <form class="tag-form" onsubmit={(event) => { event.preventDefault(); void submitTag() }}>
          <label>
            <span>Nome</span>
            <input bind:value={tagFormName} maxlength="40" placeholder="Ex.: Ambient" disabled={tagSaving} />
          </label>
          <label class="tag-color-field">
            <span>Cor</span>
            <div>
              <input type="color" bind:value={tagFormColor} disabled={tagSaving} />
              <code>{tagFormColor.toUpperCase()}</code>
            </div>
          </label>
          <button class="tag-submit" type="submit" disabled={tagSaving || !tagFormName.trim()}>
            {tagSaving ? 'Salvando…' : editingTagId === null ? 'Criar Tag' : 'Salvar Tag'}
          </button>
          {#if editingTagId !== null}
            <button class="tag-form-cancel" type="button" onclick={resetTagForm} disabled={tagSaving}>Cancelar edição</button>
          {/if}
        </form>

        {#if tagError}<div class="tag-error">{tagError}</div>{/if}

        <div class="tag-list">
          {#if tags.length === 0}
            <p class="tag-list-empty">Nenhuma Tag criada.</p>
          {:else}
            {#each tags as tag (tag.id)}
              <div class="tag-list-item">
                <i style={`--tag-color: ${tag.color}`}></i>
                <div><strong>{tag.name}</strong><span>{tag.trackCount} {tag.trackCount === 1 ? 'Track' : 'Tracks'}</span></div>
                <code>{tag.color}</code>
                <button type="button" onclick={() => editTag(tag)}>Editar</button>
                <button class="tag-remove" type="button" onclick={() => tagPendingDelete = tag}>Excluir</button>
              </div>
            {/each}
          {/if}
        </div>
      {/if}
    </div>
  </div>
{/if}

{#if albumModalOpen}
  <div class="modal-backdrop">
    <button class="modal-dismiss" type="button" onclick={closeAlbumModal} aria-label="Fechar formulário do Album"></button>
    <form class="confirm-modal album-modal" onsubmit={(event) => { event.preventDefault(); void submitAlbum() }}>
      <div class="tag-modal-header">
        <div>
          <h2>{editingAlbumId === null ? 'Novo Album' : 'Editar Album'}</h2>
          <p>Os ajustes do Album não alteram a configuração original das Tracks.</p>
        </div>
        <button type="button" onclick={closeAlbumModal} aria-label="Fechar">×</button>
      </div>

      <div class="album-form">
        <label class="album-form-wide">
          <span>Nome</span>
          <input bind:value={albumName} maxlength="120" placeholder="Nome do Album" disabled={albumSaving} required />
        </label>
        <label class="album-form-wide">
          <span>Descrição</span>
          <textarea bind:value={albumDescription} maxlength="2000" rows="3" placeholder="Sobre este Album" disabled={albumSaving}></textarea>
        </label>
        <div class="album-cover-field album-form-wide">
          <span>Capa</span>
          <div>
            <div class="album-cover-preview">
              {#if albumCoverPath}<img src={convertFileSrc(albumCoverPath)} alt="Prévia da capa" />{:else}<span>♫</span>{/if}
            </div>
            <button type="button" onclick={chooseAlbumCover} disabled={albumSaving}>Escolher imagem</button>
            {#if albumCoverPath}<button class="album-cover-remove" type="button" onclick={() => albumCoverPath = null} disabled={albumSaving}>Remover</button>{/if}
          </div>
          {#if albumCoverPath}<code title={albumCoverPath}>{albumCoverPath}</code>{/if}
        </div>
        <label>
          <span>Sample rate</span>
          <select bind:value={albumSampleRate} disabled={albumSaving}>
            <option value={44100}>44.100 Hz</option>
            <option value={48000}>48.000 Hz</option>
            <option value={96000}>96.000 Hz</option>
          </select>
        </label>
        <label>
          <span>Pausa padrão (s)</span>
          <input type="number" min="0" max="3600" step="0.1" bind:value={albumDefaultGap} disabled={albumSaving} />
        </label>
      </div>

      {#if albumError}<div class="tag-error">{albumError}</div>{/if}
      <div class="modal-actions">
        <button class="modal-cancel" type="button" onclick={closeAlbumModal} disabled={albumSaving}>Cancelar</button>
        <button class="album-save" type="submit" disabled={albumSaving || !albumName.trim()}>{albumSaving ? 'Salvando…' : 'Salvar Album'}</button>
      </div>
    </form>
  </div>
{/if}

{#if albumTrackPickerOpen && activeAlbum}
  <div class="modal-backdrop">
    <button class="modal-dismiss" type="button" onclick={() => albumTrackPickerOpen = false} aria-label="Fechar seleção de Track"></button>
    <div class="confirm-modal album-picker-modal" role="dialog" aria-modal="true" aria-labelledby="album-picker-title">
      <div class="tag-modal-header">
        <div><h2 id="album-picker-title">Adicionar Track</h2><p>Escolha uma Track. Ela pode ser adicionada novamente em outra posição.</p></div>
        <button type="button" onclick={() => albumTrackPickerOpen = false} aria-label="Fechar">×</button>
      </div>
      <div class="album-picker-list">
        {#if tracks.length === 0}
          <p>Nenhuma Track importada na biblioteca.</p>
        {:else}
          {#each tracks as track (track.id)}
            <div class="album-picker-item">
              <div><strong>{track.name}</strong><span>{track.sourcePath}</span></div>
              <button type="button" onclick={() => addTrackToActiveAlbum(track.id)} disabled={albumSaving}>Adicionar</button>
            </div>
          {/each}
        {/if}
      </div>
    </div>
  </div>
{/if}

{#if albumPendingDelete}
  <div class="modal-backdrop">
    <button class="modal-dismiss" type="button" onclick={() => albumPendingDelete = null} aria-label="Fechar confirmação"></button>
    <div class="confirm-modal" role="dialog" aria-modal="true" aria-labelledby="remove-album-title">
      <div class="modal-icon" aria-hidden="true">×</div>
      <div class="modal-copy">
        <h2 id="remove-album-title">Excluir Album?</h2>
        <p><strong>{albumPendingDelete.name}</strong> e sua ordem/configuração serão removidos.</p>
        <p class="modal-note">As Tracks e os arquivos <code>.strudel</code> não serão excluídos.</p>
      </div>
      <div class="modal-actions">
        <button class="modal-cancel" type="button" onclick={() => albumPendingDelete = null} disabled={albumSaving}>Cancelar</button>
        <button class="modal-confirm" type="button" onclick={confirmAlbumRemoval} disabled={albumSaving}>{albumSaving ? 'Excluindo…' : 'Excluir Album'}</button>
      </div>
    </div>
  </div>
{/if}

{#if albumItemPendingRemove}
  <div class="modal-backdrop">
    <button class="modal-dismiss" type="button" onclick={() => albumItemPendingRemove = null} aria-label="Fechar confirmação"></button>
    <div class="confirm-modal" role="dialog" aria-modal="true" aria-labelledby="remove-album-track-title">
      <div class="modal-icon" aria-hidden="true">×</div>
      <div class="modal-copy">
        <h2 id="remove-album-track-title">Remover faixa do Album?</h2>
        <p><strong>{albumItemPendingRemove.track.name}</strong> será retirada somente desta posição do Album.</p>
        <p class="modal-note">A Track permanece na biblioteca.</p>
      </div>
      <div class="modal-actions">
        <button class="modal-cancel" type="button" onclick={() => albumItemPendingRemove = null} disabled={albumItemSavingId !== null}>Cancelar</button>
        <button class="modal-confirm" type="button" onclick={confirmAlbumItemRemoval} disabled={albumItemSavingId !== null}>{albumItemSavingId !== null ? 'Removendo…' : 'Remover faixa'}</button>
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
  .tag-filters { display: flex; align-items: center; flex-wrap: wrap; gap: 7px; }
  .tag-filters > span { margin-right: 2px; color: #46465a; font-size: 11px; }
  .tag-filters button, .track-badges span, .track-tag-options button {
    --tag-color: #c8861e;
    display: inline-flex; align-items: center; gap: 5px; padding: 4px 7px;
    background: color-mix(in srgb, var(--tag-color) 9%, #131320);
    color: color-mix(in srgb, var(--tag-color) 78%, #ddd);
    border: 1px solid color-mix(in srgb, var(--tag-color) 28%, #29293a);
    border-radius: 999px; font: 10px 'Space Grotesk', sans-serif;
  }
  .tag-filters button { cursor: pointer; opacity: 0.7; }
  .tag-filters button.active { opacity: 1; background: color-mix(in srgb, var(--tag-color) 20%, #131320); }
  .tag-filters button i, .track-badges i, .track-tag-options i {
    width: 6px; height: 6px; border-radius: 50%; background: var(--tag-color);
  }
  .tag-filters button small { color: inherit; opacity: 0.55; font-size: 9px; }
  .tag-filters .clear-tags { --tag-color: #77778c; background: transparent; }
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
  .track-badges { display: flex; flex-wrap: wrap; gap: 5px; }
  .track-badges span { padding: 3px 6px; }
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

  .album-grid {
    display: grid; grid-template-columns: repeat(auto-fill, minmax(350px, 1fr));
    gap: 14px; align-content: start;
  }
  .album-card {
    display: grid; grid-template-columns: 76px minmax(0, 1fr) auto; gap: 13px;
    padding: 12px; background: #131320; border: 1px solid #20202f; border-radius: 10px;
    cursor: pointer; outline: none; transition: border-color 140ms, transform 140ms;
  }
  .album-card:hover, .album-card:focus-visible { border-color: #664820; transform: translateY(-1px); }
  .album-cover, .album-detail-cover, .album-cover-preview {
    overflow: hidden; display: grid; place-items: center; background: #1b1b28; color: #876427;
  }
  .album-cover { width: 76px; aspect-ratio: 1; border-radius: 7px; font-size: 25px; }
  .album-cover img, .album-detail-cover img, .album-cover-preview img { width: 100%; height: 100%; object-fit: cover; }
  .album-card-copy { min-width: 0; display: flex; flex-direction: column; justify-content: center; gap: 5px; }
  .album-card-copy strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #d8d4cb; font-size: 14px; }
  .album-card-copy p { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #57576b; font-size: 11px; }
  .album-card-copy > div { display: flex; gap: 10px; color: #77778c; font-size: 10px; }

  /* ── Album detail ── */
  .album-detail { display: flex; flex-direction: column; gap: 16px; min-height: 0; }
  .album-detail-header {
    display: grid; grid-template-columns: 112px minmax(0, 1fr) auto; gap: 18px; align-items: center;
    padding: 16px; background: linear-gradient(135deg, #171724, #11111b); border: 1px solid #242434; border-radius: 11px;
  }
  .album-detail-cover { width: 112px; aspect-ratio: 1; border-radius: 8px; font-size: 34px; }
  .album-detail-copy { min-width: 0; }
  .album-eyebrow { color: #c8861e; font: 9px 'JetBrains Mono', monospace; letter-spacing: .16em; }
  .album-detail-copy h1 { margin-top: 4px; color: #e5e1d9; font-size: 25px; line-height: 1.15; }
  .album-detail-copy > p { margin-top: 7px; max-width: 680px; color: #68687d; font-size: 12px; line-height: 1.45; }
  .album-detail-meta { display: flex; gap: 14px; margin-top: 10px; color: #89899a; font: 10px 'JetBrains Mono', monospace; }
  .album-header-actions { display: flex; flex-direction: column; gap: 8px; }
  .album-header-actions button, .album-empty button {
    padding: 8px 11px; background: #1b1b29; color: #9696a7; border: 1px solid #333345; border-radius: 6px;
    cursor: pointer; font: 500 11px 'Space Grotesk', sans-serif;
  }
  .album-header-actions .album-add-track, .album-empty button { background: #c8861e; color: #0c0c12; border-color: #c8861e; }
  .album-header-actions button:disabled { opacity: .45; cursor: not-allowed; }
  .album-empty {
    display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 44px;
    background: #11111b; border: 1px dashed #303041; border-radius: 10px; text-align: center;
  }
  .album-empty > span { color: #c8861e; font-size: 28px; }
  .album-empty strong { color: #aaa6a0; font-size: 14px; }
  .album-empty p { margin-bottom: 8px; color: #55556a; font-size: 11px; }
  .album-track-list { display: flex; flex-direction: column; gap: 9px; }
  .album-track-item { background: #12121d; border: 1px solid #222231; border-radius: 9px; overflow: hidden; }
  .album-track-heading {
    display: grid; grid-template-columns: 34px minmax(0, 1fr) auto auto 25px; gap: 10px; align-items: center;
    padding: 12px 13px;
  }
  .album-track-position { color: #c8861e; font: 12px 'JetBrains Mono', monospace; }
  .album-track-copy { min-width: 0; display: flex; flex-direction: column; gap: 3px; }
  .album-track-copy > strong { color: #d2cec6; font-size: 13px; }
  .album-track-copy > span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #444459; font: 9px 'JetBrains Mono', monospace; }
  .album-track-copy .track-badges { margin-top: 2px; }
  .album-track-duration { display: flex; flex-direction: column; align-items: flex-end; gap: 2px; }
  .album-track-duration strong { color: #9999aa; font: 11px 'JetBrains Mono', monospace; }
  .album-track-duration span { color: #56566b; font-size: 9px; }
  .album-order-actions { display: flex; gap: 3px; }
  .album-order-actions button, .album-remove-track {
    width: 25px; height: 25px; background: #1b1b28; color: #77778c; border: 1px solid #2d2d3e; border-radius: 5px; cursor: pointer;
  }
  .album-order-actions button:disabled, .album-remove-track:disabled { opacity: .3; cursor: not-allowed; }
  .album-remove-track { background: transparent; color: #a95e5e; border: 0; font-size: 17px; }
  .album-item-settings {
    display: grid; grid-template-columns: repeat(5, minmax(90px, 1fr)) auto; gap: 8px; align-items: end;
    padding: 10px 13px 12px; background: #0f0f18; border-top: 1px solid #1d1d2b;
  }
  .album-item-settings label { display: flex; flex-direction: column; gap: 5px; }
  .album-item-settings label span { color: #55556a; font-size: 9px; }
  .album-item-settings input {
    width: 100%; height: 32px; padding: 6px 8px; background: #181825; color: #ccc8c0;
    border: 1px solid #29293a; border-radius: 5px; outline: none; font: 11px 'JetBrains Mono', monospace;
  }
  .album-item-settings input:focus { border-color: #c8861e; }
  .album-item-settings input:disabled { opacity: .45; }
  .album-item-actions { display: flex; gap: 5px; }
  .album-item-actions button {
    height: 32px; padding: 0 8px; white-space: nowrap; background: transparent; color: #77778c;
    border: 1px solid #303041; border-radius: 5px; cursor: pointer; font-size: 9px;
  }
  .album-item-actions .album-item-preview { color: #c8861e; border-color: #59431f; }
  .album-item-actions button:disabled { opacity: .4; cursor: not-allowed; }
  .album-player { margin: 0 13px 12px; }
  .album-render-progress { position: sticky; bottom: 78px; z-index: 4; box-shadow: 0 10px 30px #09090f; }
  .album-export-bar {
    position: sticky; bottom: -32px; z-index: 3; display: flex; align-items: center; gap: 20px;
    padding: 13px 15px; background: rgba(18, 18, 29, .96); border: 1px solid #29293a; border-radius: 9px;
    backdrop-filter: blur(8px);
  }
  .album-export-bar > div { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 3px; }
  .album-export-bar strong { color: #c9c5bd; font-size: 12px; }
  .album-export-bar span { color: #55556a; font-size: 10px; }
  .album-export-bar .btn-export { flex: 0 0 auto; min-width: 190px; padding: 10px 14px; }

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
  .track-tags-editor { display: flex; align-items: flex-start; gap: 9px; }
  .track-tags-label { padding-top: 4px; color: #4c4c61; font-size: 11px; }
  .track-tag-options { display: flex; flex-wrap: wrap; gap: 6px; }
  .track-tag-options button { cursor: pointer; opacity: 0.55; }
  .track-tag-options button.attached { opacity: 1; background: color-mix(in srgb, var(--tag-color) 18%, #131320); }
  .track-tag-options button:disabled { cursor: wait; }
  .create-first-tag {
    padding: 4px 7px; background: transparent; color: #9a6b25;
    border: 1px dashed #60461f; border-radius: 6px; cursor: pointer; font-size: 10px;
  }

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

  .tag-modal { width: min(620px, 100%); max-height: min(720px, calc(100vh - 48px)); overflow-y: auto; }
  .tag-modal-header { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; }
  .tag-modal-header h2 { color: #e0dcd4; font-size: 18px; }
  .tag-modal-header p { margin-top: 4px; color: #59596e; font-size: 12px; }
  .tag-modal-header > button {
    background: transparent; color: #606076; border: 0; cursor: pointer; font-size: 21px;
  }
  .tag-form {
    display: grid; grid-template-columns: minmax(0, 1fr) 130px auto; gap: 9px; align-items: end;
    margin-top: 18px; padding: 13px; background: #101019; border: 1px solid #232333; border-radius: 8px;
  }
  .tag-form label { display: flex; flex-direction: column; gap: 6px; }
  .tag-form label > span { color: #55556a; font-size: 10px; }
  .tag-form input:not([type='color']) {
    width: 100%; height: 35px; padding: 7px 9px;
    background: #181826; color: #d8d4cb; border: 1px solid #2b2b3d; border-radius: 6px;
    outline: none; font: 12px 'Space Grotesk', sans-serif;
  }
  .tag-form input:focus { border-color: #c8861e; }
  .tag-color-field > div { display: flex; align-items: center; height: 35px; gap: 7px; }
  .tag-color-field input[type='color'] {
    width: 38px; height: 35px; padding: 3px; background: #181826; border: 1px solid #2b2b3d; border-radius: 6px;
  }
  .tag-color-field code { color: #77778c; font: 10px 'JetBrains Mono', monospace; }
  .tag-submit, .tag-form-cancel {
    height: 35px; padding: 0 11px; border-radius: 6px; cursor: pointer;
    font: 500 11px 'Space Grotesk', sans-serif;
  }
  .tag-submit { background: #c8861e; color: #0c0c12; border: 0; }
  .tag-submit:disabled { opacity: 0.4; cursor: not-allowed; }
  .tag-form-cancel { grid-column: 3; background: transparent; color: #77778c; border: 1px solid #333345; }
  .tag-error {
    margin-top: 9px; padding: 8px 10px; color: #d67575; background: rgba(200, 80, 80, 0.08);
    border-radius: 6px; font-size: 11px;
  }
  .tag-list { display: flex; flex-direction: column; gap: 6px; margin-top: 14px; }
  .tag-list-empty { padding: 28px; text-align: center; color: #505064; font-size: 12px; }
  .tag-list-item {
    display: grid; grid-template-columns: 10px minmax(0, 1fr) auto auto auto;
    align-items: center; gap: 10px; padding: 9px 10px;
    background: #11111b; border: 1px solid #20202f; border-radius: 7px;
  }
  .tag-list-item > i { width: 9px; height: 9px; border-radius: 50%; background: var(--tag-color); }
  .tag-list-item > div { display: flex; flex-direction: column; gap: 2px; }
  .tag-list-item strong { color: #c9c5bc; font-size: 12px; }
  .tag-list-item span { color: #4f4f63; font-size: 10px; }
  .tag-list-item code { color: #606075; font: 10px 'JetBrains Mono', monospace; }
  .tag-list-item button {
    padding: 5px 7px; background: transparent; color: #77778b;
    border: 1px solid #303041; border-radius: 5px; cursor: pointer; font-size: 10px;
  }
  .tag-list-item button:hover { color: #c8861e; border-color: #674b23; }
  .tag-list-item .tag-remove:hover { color: #d06b6b; border-color: #663838; }

  .album-modal { width: min(610px, 100%); max-height: calc(100vh - 48px); overflow-y: auto; }
  .album-form { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin-top: 18px; }
  .album-form label, .album-cover-field { display: flex; flex-direction: column; gap: 6px; }
  .album-form label > span, .album-cover-field > span { color: #5d5d71; font-size: 10px; }
  .album-form-wide { grid-column: 1 / -1; }
  .album-form input, .album-form textarea, .album-form select {
    width: 100%; padding: 9px 10px; background: #101019; color: #d5d1c9;
    border: 1px solid #2b2b3d; border-radius: 6px; outline: none; font: 12px 'Space Grotesk', sans-serif;
  }
  .album-form select { color-scheme: dark; }
  .album-form input:focus, .album-form textarea:focus, .album-form select:focus { border-color: #c8861e; }
  .album-form textarea { resize: vertical; min-height: 74px; }
  .album-cover-field > div { display: flex; align-items: center; gap: 8px; }
  .album-cover-preview { width: 58px; height: 58px; flex: 0 0 auto; border-radius: 6px; }
  .album-cover-field button {
    padding: 7px 9px; background: #1b1b29; color: #9090a0; border: 1px solid #333345; border-radius: 5px; cursor: pointer; font-size: 10px;
  }
  .album-cover-field .album-cover-remove { color: #b26767; border-color: #563535; }
  .album-cover-field code { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #4d4d61; font: 9px 'JetBrains Mono', monospace; }
  .album-save {
    padding: 9px 13px; background: #c8861e; color: #0c0c12; border: 0; border-radius: 7px;
    cursor: pointer; font: 600 12px 'Space Grotesk', sans-serif;
  }
  .album-save:disabled { opacity: .45; cursor: not-allowed; }
  .album-picker-modal { width: min(620px, 100%); max-height: min(720px, calc(100vh - 48px)); overflow-y: auto; }
  .album-picker-list { display: flex; flex-direction: column; gap: 6px; margin-top: 16px; }
  .album-picker-list > p { padding: 32px; text-align: center; color: #55556a; font-size: 12px; }
  .album-picker-item {
    display: flex; align-items: center; gap: 12px; padding: 10px;
    background: #101019; border: 1px solid #222231; border-radius: 7px;
  }
  .album-picker-item > div { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 3px; }
  .album-picker-item strong { color: #c9c5bd; font-size: 12px; }
  .album-picker-item span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: #49495d; font: 9px 'JetBrains Mono', monospace; }
  .album-picker-item button {
    padding: 6px 9px; background: transparent; color: #c8861e; border: 1px solid #5b4320; border-radius: 5px; cursor: pointer; font-size: 10px;
  }
  .album-picker-item button:disabled { opacity: .4; cursor: wait; }

  @media (max-width: 900px) {
    .album-detail-header { grid-template-columns: 90px minmax(0, 1fr); }
    .album-detail-cover { width: 90px; }
    .album-header-actions { grid-column: 1 / -1; flex-direction: row; }
    .album-item-settings { grid-template-columns: repeat(3, minmax(90px, 1fr)); }
    .album-item-actions { grid-column: span 2; }
  }

  @media (prefers-reduced-motion: reduce) {
    .spinner { animation: none; }
    .drop-zone, .logo-watermark { transition: none; }
  }
</style>
