import { prebake } from '@strudel/repl'
import { webaudioRepl, renderPatternAudio } from '@strudel/webaudio'
import { evalScope } from '@strudel/core'
import * as strudelCore from '@strudel/core'
import * as strudelWebaudio from '@strudel/webaudio'

let initialized = false
let _repl: ReturnType<typeof webaudioRepl> | null = null

async function ensureInit() {
  if (initialized) return
  console.log('[strudel] ensureInit: setting up evalScope + prebake...')
  // Put strudel functions on globalThis using the SAME module instances
  // that webaudioRepl uses. This avoids the dual-instance problem where
  // @strudel/repl bundles its own copy of @strudel/core internally.
  await evalScope(
    strudelCore,
    strudelWebaudio,
  )
  // prebake loads default sample packs (Dirt-Samples, piano, etc.) from GitHub
  await prebake()
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
  const result = await _repl.evaluate(code)
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

export async function renderToUrl(
  code: string,
  cps: number,
  loops: number,
): Promise<string> {
  console.log('[strudel] renderToUrl start — cps:', cps, 'loops:', loops)
  await ensureInit()
  const pattern = await evalPattern(code)
  console.log('[strudel] got pattern, calling renderPatternAudio (begin=0, end=', loops, ')')

  return new Promise<string>((resolve, reject) => {
    const orig = HTMLAnchorElement.prototype.click
    HTMLAnchorElement.prototype.click = function (this: HTMLAnchorElement) {
      HTMLAnchorElement.prototype.click = orig
      console.log('[strudel] blob URL intercepted:', this.href.slice(0, 60))
      resolve(this.href)
    }
    renderPatternAudio(pattern, cps, 0, loops, 44100, 32, false)
      .then(() => console.log('[strudel] renderPatternAudio resolved'))
      .catch((err) => {
        HTMLAnchorElement.prototype.click = orig
        console.error('[strudel] renderPatternAudio error:', err)
        reject(err)
      })
  })
}

export async function exportToWav(
  code: string,
  cps: number,
  loops: number,
  name: string,
): Promise<void> {
  console.log('[strudel] exportToWav start — loops:', loops, 'name:', name)
  await ensureInit()
  const pattern = await evalPattern(code)
  console.log('[strudel] got pattern, exporting...')
  await renderPatternAudio(pattern, cps, 0, loops, 44100, 32, false, name)
  console.log('[strudel] export done')
}
