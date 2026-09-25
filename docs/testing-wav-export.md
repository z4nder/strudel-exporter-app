# Testes de feature do export WAV

Os testes usam `source/forest.strudel` e chamam diretamente as funções usadas
pela aplicação, sem montar `App.svelte` e sem depender da interface.

O cenário verifica em sequência:

- `analyzeStrudel()` identifica `0.5 cps` e o período completo de 8 cycles;
- `renderToUrl()` transforma um round completo em uma URL de preview WAV válida;
- `exportToWav()` reutiliza esse WAV-base e envia ao Rust a base e a quantidade de loops.
- um intervalo manual `2 → 3` gera exatamente 2 segundos em 48 kHz para a
  composição de `0.5 cps`.

O Strudel nunca renderiza novamente a composição inteira para cada repetição.
Ele sintetiza somente `minLoopCycles` uma vez e mantém esse WAV-base em cache.
O cache inclui código, Start/End cycle, sample rate e maximum polyphony; uma
configuração nunca reutiliza o áudio produzido por outra.
O player repete a base durante o preview; no export, o Rust grava o bloco PCM
sequencialmente e atualiza os tamanhos `RIFF` e `data`, sem FFmpeg e sem
recompressão.

O período é descoberto comparando os eventos gerados pelo próprio `Pattern` do
Strudel em cycles sucessivos. O teste não interpreta o texto de `arrange()`.

O Chromium headless é usado somente como runtime para `OfflineAudioContext`,
`AudioBuffer` e as demais APIs Web Audio. Não há interação com botões, textos,
HTML da aplicação ou estado de componentes Svelte.

Somente as fronteiras nativas do segundo caso — diálogo e escrita Tauri — são
simuladas. Avaliação, samples, synths, efeitos e renderização Strudel são reais.

## Executar

```bash
pnpm test:feature
cargo test --manifest-path src-tauri/Cargo.toml
```

O teste requer internet porque `forest.strudel` e os bancos padrão carregam
samples remotos.

## Saídas

- Log detalhado: `logs/strudel-export.log`
- WAV gerado: `test-results/forest.wav`
- Trace de falha: `test-results/**/trace.zip`

O log é acumulativo para permitir acompanhar as iterações do ciclo TDD.

## Critérios de validade

O teste falha se qualquer uma destas condições não for atendida:

- a fixture `forest.strudel` não puder ser carregada;
- o período mínimo detectado não for 8 cycles;
- 10 loops não corresponderem a 160 segundos em `0.5 cps`;
- qualquer uma das funções de preview/export falhar;
- o backend nativo não receber uma chamada de gravação;
- o arquivo não tiver cabeçalhos `RIFF`, `WAVE`, `fmt ` e `data`;
- o WAV não for PCM estéreo em 44.1 kHz;
- a duração de um loop completo não estiver próxima de 16 segundos;
- o intervalo manual `2 → 3` não resultar em 2 segundos a 48 kHz;
- o áudio estiver silencioso;
- o motor Web Audio reportar sample ausente ou nós de contextos diferentes.

Os testes Rust também falham se a repetição não preservar exatamente o PCM,
se o cabeçalho final tiver tamanhos incorretos ou se loops fora de `1..=999`
forem aceitos.
