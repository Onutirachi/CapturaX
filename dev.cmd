@echo off
rem Roda o Capturax (Preact) em modo dev. Fechar o app ou esta janela encerra tudo.
cd /d "%~dp0"
title Capturax React (dev)

if not exist node_modules call npm install

call npm run tauri dev

rem Garante que nada ficou para tras depois que o app fechar
taskkill /f /t /im capturax-react.exe >nul 2>&1
exit
