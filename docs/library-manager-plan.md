# Plano — Strudel Library Manager

Status: em implementação. A Fase 1 já possui SQLite, migration inicial,
importação/listagem/remoção de Tracks e persistência da configuração de render.
Tags e Albums permanecem nas fases seguintes.

Este documento descreve a evolução do exportador atual para uma biblioteca
local de Tracks e Albums. O renderizador Strudel, a detecção de round, o cache
de preview e a repetição PCM em Rust continuam sendo a base de áudio.

## Objetivos

- cadastrar arquivos `.strudel` sem copiar ou alterar o arquivo original;
- salvar configurações de render por Track;
- salvar configurações independentes para uma Track dentro de cada Album;
- organizar Tracks com tags coloridas e filtros;
- montar Albums com capa, descrição, ordem e configuração por faixa;
- exportar um Album como um único WAV acompanhado de metadata;
- manter o app responsável apenas pelas configurações, não pelos WAVs exportados.

## Decisões principais

### Persistência

Usar SQLite no diretório de dados da aplicação. O banco guarda metadata,
configurações e caminhos dos sources. Os `.strudel` permanecem no filesystem e
os `.wav` gerados não são cadastrados no banco.

### Source externo

Importar uma Track registra o caminho absoluto e normalizado do `.strudel`.
O aplicativo não passa a ser dono desse arquivo e não o exclui.

### Responsabilidade pelos exports

Ao exportar, o usuário escolhe o destino e o app grava o WAV. Depois disso, o
arquivo pertence ao usuário: não há histórico, player, monitoramento ou exclusão
de exports dentro da biblioteca. O preview continua temporário e não é
persistido.

No MVP não haverá watcher nem varredura contínua dos sources. Se a leitura de
um `.strudel` falhar ao abrir ou renderizar, a interface informa o erro. Detecção
proativa de arquivos movidos/removidos fica como feature opcional.

### Repetição de áudio

O Strudel renderiza somente o intervalo configurado. Loops adicionais são
gravados em Rust repetindo o PCM, sem FFmpeg e sem recompressão.

### Sample rate de Album

Um Album tem um único sample rate (`44100`, `48000` ou `96000`). Todas as suas
faixas são renderizadas nesse sample rate para permitir concatenação direta.
Start/End, loops e maximum polyphony continuam configuráveis por faixa.

## Configurações salvas

Existem três níveis distintos.

### 1. Configuração padrão da Track

Cada Track possui uma configuração editável e persistida:

```text
range_mode         automatic | manual
start_cycle        padrão 0
end_cycle          round detectado ou valor manual
loops              1..999
sample_rate        44100 | 48000 | 96000
max_polyphony      1..256
export_file_name   nome sugerido sem .wav
```

No modo automático, uma nova análise do source pode atualizar `end_cycle` com o
round detectado. No modo manual, Start e End nunca são sobrescritos pela análise.

A tela informa quando existem alterações ainda não salvas e oferece:

- `Salvar configuração`;
- `Descartar alterações`;
- `Restaurar round automático`.

### 2. Configuração da Track dentro de um Album

Ao adicionar uma Track a um Album, a configuração atual da Track é copiada para
o item do Album. A partir desse momento ela é independente.

Isso evita que editar a Track posteriormente altere silenciosamente um Album já
montado. No item do Album existirão as ações:

- `Salvar neste Album`;
- `Recarregar padrão da Track`;
- `Aplicar esta configuração como padrão da Track` — exige confirmação.

O item salva:

```text
position
range_mode
start_cycle
end_cycle
loops
max_polyphony
gap_after_seconds
```

O sample rate vem do Album, não do item.

### Snapshot em memória durante a geração

Ao iniciar preview ou export, o app cria uma cópia em memória da configuração.
Os campos ficam bloqueados até concluir ou cancelar. Esse snapshot não é salvo
como histórico após a geração.

## Regras de duração

Para uma Track:

```text
base_seconds = (end_cycle - start_cycle) / cps
duration_seconds = base_seconds * loops
```

Para um Album:

```text
album_duration = soma(track_duration + gap_after_seconds)
```

O último gap pode ser zero por padrão.

## Modelo de dados

### tracks

```text
id                    UUID/TEXT PK
name                  TEXT NOT NULL
source_path           TEXT NOT NULL UNIQUE
source_path_normalized TEXT NOT NULL UNIQUE
created_at            INTEGER NOT NULL
updated_at            INTEGER NOT NULL
```

CPS, duração do cycle e round detectado são obtidos pelo Strudel quando a Track
é aberta. Eles não são necessários para persistir a configuração escolhida.

### track_render_settings

```text
track_id              FK tracks, PK
range_mode            automatic | manual
start_cycle           REAL
end_cycle             REAL
loops                 INTEGER
sample_rate           INTEGER
max_polyphony         INTEGER
export_file_name      TEXT
updated_at            INTEGER
```

### tags

```text
id                    UUID/TEXT PK
name                  TEXT NOT NULL UNIQUE COLLATE NOCASE
color                 TEXT NOT NULL
created_at            INTEGER
updated_at            INTEGER
```

### track_tags

```text
track_id              FK tracks
tag_id                FK tags
PRIMARY KEY(track_id, tag_id)
```

### albums

```text
id                    UUID/TEXT PK
name                  TEXT NOT NULL
description           TEXT
cover_path            TEXT NULL
sample_rate           INTEGER
default_gap_seconds   REAL
created_at            INTEGER
updated_at            INTEGER
```

### album_tracks

```text
id                    UUID/TEXT PK
album_id              FK albums
track_id              FK tracks
position              INTEGER
range_mode            automatic | manual
start_cycle           REAL
end_cycle             REAL
loops                 INTEGER
max_polyphony         INTEGER
gap_after_seconds     REAL
created_at            INTEGER
updated_at            INTEGER
UNIQUE(album_id, position)
```

A mesma Track pode aparecer mais de uma vez no mesmo Album. Por isso
`album_tracks` tem ID próprio e não usa `(album_id, track_id)` como chave.

## Resolução de configuração

| Contexto | Fonte da configuração |
|---|---|
| Tela da Track | `track_render_settings` |
| Track recém-adicionada ao Album | cópia da configuração da Track |
| Track já presente no Album | `album_tracks`, independente da Track |
| Preview no Album | configuração salva ou editada daquele item |
| Export da Track | cópia em memória da configuração da Track |
| Export do Album | cópia em memória do Album e de todos os seus itens |

Durante uma geração, os campos ficam bloqueados. Alterações posteriores não
podem afetar uma operação que já começou.

## Interface

### Navegação principal

```text
[ Tracks ] [ Albums ]                         [ Gerenciar tags ]
```

### Listagem de Tracks

- busca por nome e caminho;
- filtro por uma ou várias tags;
- modo `qualquer tag` ou `todas as tags`;
- ordenação por nome, importação, modificação ou duração;
- badges de tags e duração calculada;
- abrir detalhes e gerar preview temporário.

### Detalhe da Track

```text
Cabeçalho: nome, caminho e tags
Análise: CPS, duração do cycle e round detectado
Configuração salva de render
Preview
Exportar WAV
```

### CRUD de Tags

- criar, editar e excluir nome/cor;
- impedir nomes duplicados sem diferenciar maiúsculas;
- mostrar quantas Tracks usam a tag;
- ao excluir, remover somente os vínculos, nunca as Tracks.

### Listagem de Albums

- capa, nome, descrição curta;
- quantidade de faixas e duração total;
- união das tags das Tracks incluídas;
- acesso à edição e ao export completo.

### Detalhe do Album

```text
Capa, nome e descrição
Sample rate e gap padrão
Tracks ordenáveis por drag-and-drop
Configuração e preview de cada item
Timeline calculada
Exportar Album completo
```

Na tela do Album não existe export individual do item. O preview individual é
permitido, mas a ação de export gera o Album completo.

## Export de Album

Etapas:

1. validar que todas as Tracks e sources existem;
2. congelar um snapshot da configuração;
3. renderizar ou reutilizar cache de cada intervalo no sample rate do Album;
4. repetir loops de cada trecho em Rust;
5. inserir o silêncio configurado após cada faixa;
6. concatenar o PCM sem recompressão;
7. gravar o WAV final;
8. gravar manifesto JSON e copiar a capa, se houver;
9. entregar os arquivos no destino escolhido pelo usuário.

Se a operação falhar, arquivos temporários são removidos. A gravação final deve
usar arquivo temporário seguido de rename atômico quando o filesystem permitir.

## Manifesto do Album

O export gera, por padrão:

```text
Meu Album.wav
Meu Album.json
Meu Album-cover.png
```

Exemplo resumido:

```json
{
  "album": {
    "name": "Meu Album",
    "description": "Descrição",
    "sampleRate": 48000,
    "durationSeconds": 248
  },
  "tracks": [
    {
      "position": 1,
      "name": "Forest",
      "startsAtSeconds": 0,
      "endsAtSeconds": 160,
      "sourcePath": "/music/forest.strudel",
      "tags": ["ambient", "forest"],
      "render": {
        "startCycle": 0,
        "endCycle": 8,
        "loops": 10,
        "maxPolyphony": 32
      }
    }
  ]
}
```

Uma fase posterior pode adicionar chunks `cue` ao WAV para marcar o início das
faixas. O JSON permanece a fonte completa da metadata.

## Feature opcional — monitoramento do filesystem

Não faz parte do MVP. Em uma evolução futura, o app poderá verificar sources ao
abrir/focar a janela ou usar watcher nativo. A proposta futura é esconder da
lista padrão uma Track cujo source desapareceu, sem apagar sua configuração,
tags ou vínculos com Albums, e permitir `Localizar arquivo`.

Exports WAV nunca serão monitorados, mesmo nessa fase opcional.

## Organização sugerida do código

```text
src/
├── features/
│   ├── tracks/
│   ├── albums/
│   ├── tags/
│   └── render/
├── lib/
│   ├── strudel-runner.ts
│   ├── audio-player.ts
│   └── tauri-client.ts
└── App.svelte

src-tauri/src/
├── main.rs
├── db/
│   ├── migrations.rs
│   └── repositories/
├── commands/
│   ├── tracks.rs
│   ├── tags.rs
│   ├── albums.rs
│   └── render.rs
├── audio/
│   ├── wav.rs
│   └── album.rs
```

## Comandos Tauri previstos

```text
tracks_list / tracks_create / tracks_update / tracks_delete
tracks_save_render_settings
tags_list / tags_create / tags_update / tags_delete
track_tags_attach / track_tags_detach
albums_list / albums_create / albums_update / albums_delete
album_tracks_add / album_tracks_update / album_tracks_reorder / album_tracks_remove
track_export_wav / album_export_wav
```

Operações de banco devem ocorrer em transações. Excluir Track, Tag ou Album
remove somente dados do SQLite; nunca remove `.strudel` ou WAV do filesystem.

## Estratégia de testes

### Rust — unitários

- migrations e constraints do SQLite;
- CRUD e relacionamentos;
- ordenação de `album_tracks`;
- repetição e concatenação PCM;
- inserção de silêncio;
- cálculo da timeline;
- geração do manifesto.

### Feature — browser como runtime de áudio

- detectar o round de um `.strudel` real;
- salvar e restaurar configuração de Track;
- criar override de Album sem alterar o padrão da Track;
- recarregar o padrão da Track explicitamente;
- renderizar Start/End, sample rate e polifonia configurados;
- invalidar cache quando source ou configuração mudar;
- exportar N loops a partir de um único WAV-base.

### Integração Tauri/Rust

- importar Track e persistir após reiniciar;
- exportar Album e validar WAV, JSON, ordem, offsets e duração final;
- limpar arquivos temporários após cancelamento ou falha;
- garantir que excluir um registro nunca exclua arquivos do usuário.

Não serão usados testes dependentes da estrutura visual da interface para o
pipeline de áudio. Testes de componentes ficam restritos a comportamentos como
filtros, formulário sujo, badges e ordenação.

## Fases de implementação

### Fase 1 — Fundação e SQLite

- [x] separar o módulo de biblioteca no Rust;
- [x] criar a migration inicial;
- [x] implementar `tracks` e `track_render_settings`;
- [x] migrar a tela atual para carregar/salvar uma Track persistida;
- [x] preservar os testes de áudio existentes.

Critério: importar, fechar, reabrir e recuperar Track e configuração.

### Fase 2 — Biblioteca de Tracks

- listagem, busca e detalhe;
- salvar configuração padrão;
- preview temporário e export pelo diálogo de destino.

Critério: o fluxo atual funciona a partir de qualquer Track cadastrada.

### Fase 3 — Tags

- CRUD de tags;
- attach/detach N:N;
- badges e filtros;
- proteção ao excluir tags em uso.

Critério: filtros continuam corretos após editar ou excluir uma tag.

### Fase 4 — Albums

- CRUD com nome, descrição e capa;
- adicionar a mesma Track uma ou mais vezes;
- copiar a configuração padrão ao adicionar;
- salvar overrides independentes;
- reorder transacional;
- cálculo da timeline.

Critério: editar um item do Album não altera a configuração padrão da Track.

### Fase 5 — Preview e export de Album

- preview individual dos itens;
- sample rate único do Album;
- concatenação, gaps e progresso;
- cancelamento seguro;
- WAV, manifesto e capa;
- entrega dos arquivos no destino escolhido, sem histórico local.

Critério: offsets e duração do manifesto correspondem ao WAV em nível de frame.

### Fase 6 — Acabamento

- recuperação de operações interrompidas;
- acessibilidade, atalhos e estados vazios.

### Fase opcional — Sources ausentes

- verificação dos caminhos cadastrados;
- esconder ausentes da listagem padrão;
- relink de source;
- watcher nativo, somente se necessário.

## Ordem recomendada para começar

O primeiro incremento implementável deve ser pequeno:

1. adicionar SQLite e migrations;
2. criar `tracks` e `track_render_settings`;
3. importar um `.strudel` para a biblioteca;
4. listar Tracks persistidas;
5. abrir uma Track na tela de render atual;
6. salvar e restaurar sua configuração;
7. cobrir o ciclo completo com teste antes de iniciar Tags ou Albums.

Essa ordem mantém o export funcionando enquanto a aplicação cresce e cria a
base necessária para todos os demais relacionamentos.
