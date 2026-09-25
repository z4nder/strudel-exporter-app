import { mount } from 'svelte'
import App from './App.svelte'
import { initLogger } from './lib/logger'

initLogger()

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app
