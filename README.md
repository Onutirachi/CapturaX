# CapturaX

App desktop para Windows que exibe placas de captura com qualidade máxima e latência mínima, a ponto de dar para jogar um console pelo preview.

## Recursos

- Abre o dispositivo no modo nativo (resolução + FPS), sem reescala por software
- Modo automático: maior resolução que mantém ~60 fps
- Latência e FPS medidos em tempo real na barra de status
- Volume e mudo, tela cheia (F11) e luz ambiente opcional
- Reconecta sozinho quando o dispositivo é desconectado e volta

## Stack

[Tauri 2](https://tauri.app) (Rust) + [Preact](https://preactjs.com) + TypeScript + Tailwind CSS v4, com build pelo Vite.

## Desenvolvimento

Requer [Node.js](https://nodejs.org), [Rust](https://rustup.rs) e os [pré-requisitos do Tauri](https://tauri.app/start/prerequisites/) para Windows.

```bash
npm install
npm run tauri dev      # ou dois cliques em dev.cmd
npm run tauri build    # gera o instalador
```
