# CapturaX

App desktop para Windows que exibe placas de captura com qualidade máxima e latência mínima, a ponto de dar para jogar um console pelo preview.

## Recursos

- Abre o dispositivo no modo nativo (resolução + FPS), sem reescala por software
- Modo automático: maior resolução que mantém ~60 fps
- Áudio do preview direto pelo Windows (WASAPI), com latência mínima
- Latência e FPS medidos em tempo real na barra de status
- Gravação em MP4 (H.264/HEVC na GPU + AAC), com qualidade ajustável e à prova de queda (F10)
- Áudio dos apps na gravação, deixando de fora os que você escolher (ex.: Discord)
- Print em PNG direto para a área de transferência (F9)
- Captura de tela/janela como fonte de vídeo
- Transmissão por link para assistir no navegador, servida pelo próprio PC (UPnP e firewall automáticos)
- Volume e mudo, tela cheia (F11) e luz ambiente opcional
- Reconecta sozinho quando o dispositivo é desconectado e volta

## Stack

[Tauri 2](https://tauri.app) (Rust) + [Preact](https://preactjs.com) + TypeScript + Tailwind CSS v4, com build pelo Vite. Somente Windows 10 (2004+) / 11.

## Desenvolvimento

Requer [Node.js](https://nodejs.org), [Rust](https://rustup.rs) e os [pré-requisitos do Tauri](https://tauri.app/start/prerequisites/) para Windows.

```bash
npm install
npm run tauri dev      # ou dois cliques em dev.cmd
npx tauri build        # gera o instalador
```
