import { test, expect } from '@playwright/test'
import { appendFile, mkdir, readFile, stat } from 'node:fs/promises'
import path from 'node:path'

const projectRoot = process.cwd()
const logDir = path.join(projectRoot, 'logs')
const logPath = path.join(logDir, 'strudel-export.log')
const outputPath = path.join(projectRoot, 'test-results', 'forest.wav')

async function log(message: string) {
  await mkdir(logDir, { recursive: true })
  await appendFile(logPath, `${new Date().toISOString()} ${message}\n`)
}

test('importa forest.strudel pela UI e exporta um WAV válido', async ({ page }) => {
  await log('START import/export e2e source/forest.strudel')

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
  page.on('pageerror', (error) => void log(`PAGE_ERROR ${error.stack ?? error.message}`))
  page.on('requestfailed', (request) => void log(`REQUEST_FAILED ${request.url()} ${request.failure()?.errorText}`))

  await page.goto('/?e2e=1')
  await log('ACTION click import drop-zone')
  await page.getByRole('button', { name: /arraste um arquivo/i }).click()

  await expect(page.getByText('forest.strudel')).toBeVisible()
  await expect(page.getByText('0.5', { exact: true })).toBeVisible()
  await log('ASSERT imported forest.strudel cps=0.5')

  const downloadPromise = page.waitForEvent('download', { timeout: 160_000 })
  await log('ACTION click Exportar WAV')
  await page.getByRole('button', { name: 'Exportar WAV' }).click()
  const download = await downloadPromise
  await download.saveAs(outputPath)

  await expect.poll(() => page.evaluate(() => window.__STRUDEL_E2E__.saveCalls)).toBe(1)
  const browserBytes = await page.evaluate(() => window.__STRUDEL_E2E__.savedBytes)

  const wav = await readFile(outputPath)
  expect(wav.subarray(0, 4).toString('ascii')).toBe('RIFF')
  expect(wav.subarray(8, 12).toString('ascii')).toBe('WAVE')
  expect(wav.subarray(12, 16).toString('ascii')).toBe('fmt ')
  expect(wav.subarray(36, 40).toString('ascii')).toBe('data')
  expect(wav.readUInt16LE(22)).toBe(2)
  expect(wav.readUInt32LE(24)).toBe(44_100)
  expect(wav.length).toBeGreaterThan(44_100)
  expect(browserBytes).toBe(wav.length)

  const byteRate = wav.readUInt32LE(28)
  const dataSize = wav.readUInt32LE(40)
  const durationSeconds = dataSize / byteRate
  expect(durationSeconds).toBeGreaterThan(1.95)
  expect(durationSeconds).toBeLessThan(2.05)

  let peakSample = 0
  for (let offset = 44; offset + 1 < wav.length; offset += 2) {
    peakSample = Math.max(peakSample, Math.abs(wav.readInt16LE(offset)))
  }
  expect(peakSample).toBeGreaterThan(100)
  expect(audioErrors).toEqual([])

  const { size } = await stat(outputPath)
  await log(
    `PASS WAV path=${outputPath} bytes=${size} channels=2 sampleRate=44100 duration=${durationSeconds.toFixed(3)}s peak=${peakSample}`,
  )
})
