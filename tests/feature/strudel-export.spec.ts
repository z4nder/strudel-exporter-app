import { expect, test, type Page } from '@playwright/test'
import { appendFile, mkdir, writeFile } from 'node:fs/promises'
import path from 'node:path'

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

const projectRoot = process.cwd()
const logPath = path.join(projectRoot, 'logs', 'strudel-export.log')
const outputPath = path.join(projectRoot, 'test-results', 'forest.wav')

async function log(message: string) {
  await mkdir(path.dirname(logPath), { recursive: true })
  await appendFile(logPath, `${new Date().toISOString()} ${message}\n`)
}

function captureProcessLog(page: Page): string[] {
  const audioErrors: string[] = []
  page.on('console', (message) => {
    const text = message.text()
    void log(`BROWSER ${message.type()} ${text}`)
    if (
      text.includes('[webaudio] error:') ||
      text.includes('InvalidAccessError') ||
      /sound .* not found/i.test(text)
    ) {
      audioErrors.push(text)
    }
  })
  page.on('pageerror', (error) => {
    audioErrors.push(error.message)
    void log(`PAGE_ERROR ${error.stack ?? error.message}`)
  })
  page.on('requestfailed', (request) =>
    void log(`REQUEST_FAILED ${request.url()} ${request.failure()?.errorText}`),
  )
  return audioErrors
}

function expectValidWav(wav: WavSummary, expectedDuration: number, expectedSampleRate = 44_100) {
  expect(wav.riff).toBe('RIFF')
  expect(wav.wave).toBe('WAVE')
  expect(wav.format).toBe('fmt ')
  expect(wav.data).toBe('data')
  expect(wav.channels).toBe(2)
  expect(wav.sampleRate).toBe(expectedSampleRate)
  expect(wav.durationSeconds).toBeGreaterThan(expectedDuration - 0.05)
  expect(wav.durationSeconds).toBeLessThan(expectedDuration + 0.05)
  expect(wav.bytes).toBeGreaterThan(44_100)
  expect(wav.peakSample).toBeGreaterThan(100)
}

test('analisa forest.strudel e gera preview/export da composição completa', async ({ page }) => {
  await log('START feature composition source/forest.strudel')
  const audioErrors = captureProcessLog(page)
  await page.goto('/tests/feature/harness.html')
  await page.waitForFunction(() => Boolean(window.strudelFeature))

  const meta = await page.evaluate(() => window.strudelFeature.analyze())

  expect(meta.cps).toBe(0.5)
  expect(meta.cycleDuration).toBe(2)
  expect(meta.minLoopCycles).toBe(8)
  expect(meta.cycleDuration * meta.minLoopCycles * 10).toBe(160)

  const wav = await page.evaluate(() => window.strudelFeature.renderPreview())

  expectValidWav(wav, 16)
  await log(`PASS preview bytes=${wav.bytes} duration=${wav.durationSeconds.toFixed(3)}s peak=${wav.peakSample}`)

  const result = await page.evaluate(() => window.strudelFeature.exportWav())
  await mkdir(path.dirname(outputPath), { recursive: true })
  await writeFile(outputPath, Buffer.from(result.bytes))

  expect(result.saveCalls).toBe(1)
  expectValidWav(result.summary, 16)

  const manualWav = await page.evaluate(() => window.strudelFeature.renderManualPreview())
  expectValidWav(manualWav, 2, 48_000)
  await log(
    `PASS manual range=2..3 sampleRate=${manualWav.sampleRate} duration=${manualWav.durationSeconds.toFixed(3)}s`,
  )

  expect(audioErrors).toEqual([])
  await log(
    `PASS export path=${outputPath} bytes=${result.summary.bytes} duration=${result.summary.durationSeconds.toFixed(3)}s peak=${result.summary.peakSample}`,
  )
})
