# Strudel WAV Exporter — Plano de Implementação (Tauri)

> Documento histórico do planejamento inicial. O fluxo implementado e seu teste
> ponta a ponta estão descritos em `docs/testing-wav-export.md`.

## Stack

- **Backend:** Rust (Tauri) — leitura de arquivo, encoding WAV, lógica do app
- **Frontend:** Svelte + TypeScript — UI, execução do Strudel, renderização via Web Audio
- **Audio rendering:** `OfflineAudioContext` (browser API nativa)

---

## Estrutura do projeto

```
strudel-wav-exporter/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs          # entry point Tauri
│   │   └── wav.rs           # encoding WAV a partir de PCM
│   └── Cargo.toml
├── src/
│   ├── App.svelte            # UI principal
│   ├── strudel-runner.ts     # executa Strudel no OfflineAudioContext
│   ├── loop-parser.ts        # detecta cycle time do arquivo .strudel
│   └── main.ts
├── package.json
└── tauri.conf.json
```

---

## Pré-requisitos

```bash
# Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Tauri CLI
cargo install tauri-cli

# Node
npm install -g pnpm

# Criar projeto
pnpm create tauri-app strudel-wav-exporter
# escolher: Svelte + TypeScript
```

---

## Passo 1 — Ler o arquivo .strudel (Rust)

Em `src-tauri/src/main.rs`, expor um comando Tauri pra leitura de arquivo:

```rust
use tauri::command;

#[command]
fn read_strudel_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![read_strudel_file, save_wav])
        .run(tauri::generate_context!())
        .expect("error running tauri app");
}
```

---

## Passo 2 — Detectar cycle time (TypeScript)

O cycle time em Strudel é controlado por `cps()`. Padrão é `0.5 cps` = 2 segundos por cycle.

Em `src/loop-parser.ts`:

```typescript
export interface StrudelMeta {
  cps: number;         // cycles per second
  cycleDuration: number; // segundos por cycle
  minLoopCycles: number; // ciclos no loop mínimo (sempre 1 por padrão)
}

export function parseStrudelMeta(code: string): StrudelMeta {
  // Detecta .cps(X) ou setcps(X) no arquivo
  const cpsMatch = code.match(/\.?cps\(([0-9.]+)\)/);
  const cps = cpsMatch ? parseFloat(cpsMatch[1]) : 0.5;
  const cycleDuration = 1 / cps;

  // Tenta detectar stack de padrões com comprimentos diferentes
  // ex: "s('bd sd bd sd')" tem 4 steps = 1 cycle
  // Para MVP, assume 1 cycle = loop mínimo
  const minLoopCycles = 1;

  return { cps, cycleDuration, minLoopCycles };
}

export function calcDuration(meta: StrudelMeta, loops: number): number {
  return meta.cycleDuration * meta.minLoopCycles * loops;
}
```

> **Nota:** Detectar o loop mínimo real (LCM de todos os padrões) requer executar o Strudel e observar quantos cycles até repetir. Isso pode ser feito na Fase 2 do projeto.

---

## Passo 3 — UI (Svelte)

Em `src/App.svelte`:

```svelte
<script lang="ts">
  import { invoke } from '@tauri-apps/api/tauri'
  import { open } from '@tauri-apps/api/dialog'
  import { parseStrudelMeta, calcDuration } from './loop-parser'

  let filePath = ''
  let fileContent = ''
  let trackName = 'minha-musica'
  let loops = 1
  let meta = { cps: 0.5, cycleDuration: 2, minLoopCycles: 1 }

  $: durationSec = calcDuration(meta, loops)
  $: durationMin = (durationSec / 60).toFixed(2)

  async function importFile() {
    const selected = await open({ filters: [{ name: 'Strudel', extensions: ['strudel', 'js'] }] })
    if (!selected || Array.isArray(selected)) return

    filePath = selected
    trackName = selected.split('/').pop()?.replace(/\.\w+$/, '') ?? 'track'
    fileContent = await invoke<string>('read_strudel_file', { path: filePath })
    meta = parseStrudelMeta(fileContent)
  }

  async function exportWav() {
    // chama strudel-runner.ts (Passo 4)
    const pcmData = await renderStrudel(fileContent, durationSec, meta.cps)
    await invoke('save_wav', { name: trackName, samples: Array.from(pcmData), sampleRate: 44100 })
  }
</script>

<main>
  <h1>Strudel WAV Exporter</h1>

  <button on:click={importFile}>Importar .strudel</button>

  {#if fileContent}
    <div class="form">
      <label>
        Nome da faixa
        <input bind:value={trackName} />
      </label>

      <label>
        Loops: <input type="number" bind:value={loops} min="1" />
      </label>

      <p>
        Duração: <strong>{durationSec.toFixed(1)}s</strong> ({durationMin} min)
        <br/>
        Cycle: {meta.cycleDuration.toFixed(2)}s @ {meta.cps} cps
      </p>

      <button on:click={exportWav}>Exportar WAV</button>
    </div>
  {/if}
</main>
```

---

## Passo 4 — Renderizar Strudel no OfflineAudioContext (TypeScript)

Em `src/strudel-runner.ts`:

```typescript
import { repl } from '@strudel/repl'  // ou importar o core diretamente

export async function renderStrudel(
  code: string,
  durationSec: number,
  cps: number
): Promise<Float32Array> {
  const sampleRate = 44100
  const offlineCtx = new OfflineAudioContext(2, sampleRate * durationSec, sampleRate)

  // Inicializa o Strudel apontando pro OfflineAudioContext
  // A API exata depende da versão do @strudel/core usada
  const { scheduler } = await repl({
    audioContext: offlineCtx,
    code,
    cps,
  })

  scheduler.start()

  const buffer = await offlineCtx.startRendering()

  // Converte AudioBuffer pra Float32Array intercalado (stereo)
  const left = buffer.getChannelData(0)
  const right = buffer.getChannelData(1)
  const interleaved = new Float32Array(left.length * 2)
  for (let i = 0; i < left.length; i++) {
    interleaved[i * 2] = left[i]
    interleaved[i * 2 + 1] = right[i]
  }

  return interleaved
}
```

> **Atenção:** A API do `@strudel/repl` / `@strudel/core` para uso headless precisa ser confirmada com a versão instalada. Pode exigir ajustes. Ver: https://strudel.cc/learn/code-exports

---

## Passo 5 — Encoding WAV (Rust)

Em `src-tauri/src/wav.rs`:

```rust
use std::io::Write;

pub fn encode_wav(samples: &[f32], sample_rate: u32, channels: u16) -> Vec<u8> {
    let num_samples = samples.len();
    let byte_rate = sample_rate * channels as u32 * 2; // 16-bit
    let block_align = channels * 2;
    let data_size = (num_samples * 2) as u32;
    let chunk_size = 36 + data_size;

    let mut buf: Vec<u8> = Vec::with_capacity(44 + data_size as usize);

    // RIFF header
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&chunk_size.to_le_bytes());
    buf.extend_from_slice(b"WAVE");

    // fmt chunk
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());  // chunk size
    buf.extend_from_slice(&1u16.to_le_bytes());   // PCM
    buf.extend_from_slice(&channels.to_le_bytes());
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&block_align.to_le_bytes());
    buf.extend_from_slice(&16u16.to_le_bytes());  // bits per sample

    // data chunk
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_size.to_le_bytes());
    for &s in samples {
        let val = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        buf.extend_from_slice(&val.to_le_bytes());
    }

    buf
}
```

Em `main.rs`, adicionar o comando de salvar:

```rust
mod wav;
use tauri::command;
use tauri::api::dialog::FileDialogBuilder;

#[command]
async fn save_wav(name: String, samples: Vec<f32>, sample_rate: u32) -> Result<String, String> {
    let encoded = wav::encode_wav(&samples, sample_rate, 2);

    let path = format!("{}.wav", name);
    std::fs::write(&path, encoded).map_err(|e| e.to_string())?;
    Ok(path)
}
```

---

## Dependências

### Rust (`Cargo.toml`)
```toml
[dependencies]
tauri = { version = "1", features = ["dialog-open", "dialog-save"] }
```

### Node (`package.json`)
```json
{
  "dependencies": {
    "@strudel/core": "latest",
    "@strudel/repl": "latest",
    "@tauri-apps/api": "latest"
  }
}
```

---

## Roadmap

| Fase | Escopo |
|------|--------|
| **MVP** | Import .strudel, calcular duração por cps, exportar WAV com N loops |
| **Fase 2** | Detectar loop mínimo real via execução do Strudel (LCM dos padrões) |
| **Fase 3** | Preview de áudio antes de exportar |
| **Fase 4** | Waveform visual, fade in/out, normalização |

---

## Pontos de atenção

1. **API headless do Strudel** — a parte mais incerta. O Strudel foi feito pra rodar no browser interativamente. Usar `OfflineAudioContext` pra renderizar offline pode exigir patches ou uso direto do `@strudel/core` sem o scheduler interativo.

2. **Samples e synths** — alguns sons do Strudel dependem de samples externos (carregados via HTTP). No modo offline, precisam ser pré-carregados ou substituídos.

3. **Dialog de save** — Tauri tem API nativa pra abrir "Save As" dialog, usar `@tauri-apps/api/dialog` `save()`.
