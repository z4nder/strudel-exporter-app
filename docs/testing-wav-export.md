# Teste ponta a ponta do export WAV

O teste usa `source/forest.strudel` e percorre o mesmo fluxo da interface:

1. clica na área de importação;
2. o mock oficial do IPC Tauri seleciona `source/forest.strudel`;
3. a aplicação chama `read_strudel_file` e atualiza a UI;
4. o teste clica em **Exportar WAV**;
5. o Strudel avalia o código e renderiza seus eventos em um `OfflineAudioContext`;
6. a aplicação abre o diálogo de destino e chama `save_wav_bytes`;
7. o teste valida o WAV salvo.

Somente as fronteiras nativas (diálogo, leitura e gravação) são simuladas. Os
cliques, o estado Svelte, o código de produção e a renderização Strudel são reais.

## Executar

```bash
pnpm test:e2e
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

- a UI não importar `forest.strudel`;
- o CPS exibido não for `0.5`;
- o backend nativo não receber uma chamada de gravação;
- o arquivo não tiver cabeçalhos `RIFF`, `WAVE`, `fmt ` e `data`;
- o WAV não for PCM estéreo em 44.1 kHz;
- a duração não estiver próxima de dois segundos para um cycle em `0.5 cps`;
- o áudio estiver silencioso;
- o motor Web Audio reportar sample ausente ou nós de contextos diferentes.
