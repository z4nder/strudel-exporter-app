import { trace, debug, info, warn, error } from '@tauri-apps/plugin-log'

// Forward console.* to Tauri log plugin so all logs go to the file
function patchConsole() {
  const _log = console.log
  const _warn = console.warn
  const _error = console.error
  const _debug = console.debug

  console.log = (...args) => {
    _log(...args)
    info(args.map(String).join(' ')).catch(() => {})
  }
  console.warn = (...args) => {
    _warn(...args)
    warn(args.map(String).join(' ')).catch(() => {})
  }
  console.error = (...args) => {
    _error(...args)
    error(args.map(String).join(' ')).catch(() => {})
  }
  console.debug = (...args) => {
    _debug(...args)
    debug(args.map(String).join(' ')).catch(() => {})
  }
}

export function initLogger() {
  try {
    patchConsole()
    info('Logger initialized').catch(() => {})
  } catch (e) {
    // silently fail if not in Tauri context
  }
}

export { trace, debug, info, warn, error }
