export interface StrudelMeta {
  cps: number
  cycleDuration: number
  minLoopCycles: number
}

export function calcDuration(meta: StrudelMeta, loops: number): number {
  return meta.cycleDuration * meta.minLoopCycles * loops
}

export function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds.toFixed(1)}s`
  const m = Math.floor(seconds / 60)
  const s = (seconds % 60).toFixed(0).padStart(2, '0')
  return `${m}m ${s}s`
}
