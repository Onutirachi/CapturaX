# Notas de continuidade — CapturaX (Preact)

Resumo de uma conversa anterior com o Claude, para retomar o trabalho num chat novo.
A arquitetura e as convenções estão no [AGENTS.md](AGENTS.md); aqui fica só o que não está lá.

## Contexto
- Existem duas versões do app. Esta (`capturax-react`, Preact + Tailwind + Tauri 2) é a **principal**.
  A versão vanilla (`../capturax-tauri`) foi **abandonada**, não precisa mexer nela.
- Rodar: dar dois cliques em `dev.cmd` (faz `npm install` se precisar, roda `npm run tauri dev`
  e encerra tudo quando o app ou a janela do cmd fecha).
- Fluxo combinado: o usuário roda o app e vai contando o que funciona e o que não funciona;
  o Claude compara com a lista abaixo e corrige.

## Decisão de arquitetura
- Reescrever tudo em Rust (egui + nokhwa + cpal) foi avaliado e **descartado por enquanto**:
  o WebView (`getUserMedia` + `<video>`) já faz conversão de formato na GPU, sincroniza
  áudio/vídeo e tem baixa latência.
- Ideia para o futuro (híbrido): um comando Tauri em Rust (nokhwa / Media Foundation) que
  liste os **modos exatos** do dispositivo (resolução + FPS + formato) para preencher os
  dropdowns, no lugar da lista fixa de `constants.ts`. O navegador só informa faixas mín/máx.

## Recursos adicionados
- **Latência:** `VideoStream` mede com `requestVideoFrameCallback`
  (`expectedDisplayTime - captureTime`, mediana a cada 500 ms) → `latency` no `VideoStreamContext`
  → bolha "Latency N ms" na StatusBar (some se o driver não fornecer `captureTime`). Mede só do
  timestamp do driver até a tela; atraso interno da câmera/placa não entra.

## Bugs suspeitos (vistos lendo o código; nada foi confirmado rodando ainda)
1. ~~Resolução e FPS se anulam~~ / 2. ~~"Automatic" não faz nada~~ — **corrigido**. Confirmado
   rodando: ao trocar a resolução a placa continuava no modo original (o Chromium só reescalava
   por software). Agora `Stream.setVideoDevice/setResolutionTag/setFrameRate` **reabrem** o
   dispositivo (`getUserMedia`) com resolução + FPS juntos e `resizeMode: "none"` (modo nativo).
   Automático = maior resolução com ≥ ~59 fps (`frameRate.min = alvo × 0.97`, aceita 59.94);
   sem modo assim, cai para a maior resolução. Resolução escolhida que não é nativa: último
   recurso é `crop-and-scale` com alerta. Reaberturas são agrupadas (`requestVideoRestart`).
3. ~~Sem microfone a toolbar some~~ — **corrigido**. Permissão agora é do `PermissionGate`
   (`src/components/PermissionGate/`): pede vídeo e áudio separados (microfone opcional) e mostra
   splash com o motivo (bloqueado no Windows / sem câmera / em uso) e botão de tentar de novo.
   No Rust (`lib.rs`), um handler `PermissionRequested` do WebView2 libera câmera e microfone
   automaticamente, sem prompt.
4. **A StatusBar pode ficar desatualizada** — `setVideoTrack(this.videoTrack)` passa a mesma
   referência, então o Preact não re-renderiza depois do `applyConstraints`. O FPS mostrado é o
   configurado (`getSettings`), não o medido.
5. ~~Erros de constraint silenciosos~~ — **corrigido** no vídeo (alerta via `handleError`/
   `handleWarning`); o áudio (`updateAudioTrack`) ainda só falha no console.
6. ~~Dropdown fecha sozinho~~ — **corrigido** (confirmado rodando: ficou pior com a latência,
   que re-renderiza o `ToolbarContent` 2×/s). O efeito agora depende da lista de valores das
   opções (`optionValues`), não da identidade de `children`.
7. **Plugar/desplugar dispositivo não atualiza as listas** — falta listener de `devicechange`.
8. **Pronto mas não ligado:** `AmbientLight.tsx` não é usado; o botão de mudo do protótipo
   `capture.html` não foi portado. (Fullscreen **feito**: `FullscreenContext.toggleFullscreen()`
   controla a janela Tauri; botão na Dock, F11 alterna, Esc sai; a WindowBar some em
   fullscreen. Se a janela estiver maximizada, desmaximiza antes (senão o tao prende o
   fullscreen ao rcWork) com as animações do DWM desligadas pelo comando Rust
   `set_window_animations`, e re-maximiza ao sair. **Dock** (`Overlay/Dock.tsx`): lado direito, aparece junto com a Toolbar
   (estado `active` e hover ficam no `Overlay`); tem fullscreen + slider de volume/mudo, com
   `volume`/`muted` no `VideoStreamContext`, salvos no localStorage.)
9. ~~Detalhes menores~~ — **corrigido** (import `Globals.css`, título "CapturaX", `ButtonToggle`
   sem `storeState`, `greet` removido, e erros de `tsc` que impediam o `npm run build`).
