import { mount } from 'svelte'
import App from './App.svelte'
import { initLogger } from './lib/logger'

if (import.meta.env.DEV && new URLSearchParams(location.search).has('e2e')) {
  await import('./test-support/e2e-tauri-mocks')
}

initLogger()

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app
