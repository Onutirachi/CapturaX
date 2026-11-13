# Run in an elevated PowerShell and leave it open: one PresentMon capture per capturax.exe run,
# named by the start time. PresentMon's --terminate_on_proc_exit does not fire here, so this
# script ends the trace session itself 3 s after the app exits. Ctrl+C to stop.
#   powershell -ExecutionPolicy Bypass -File D:\Arquivos\Projetos\CapturaX\target\t052\pm.ps1
$dir = $PSScriptRoot
$pm = Join-Path $dir 'pm\PresentMon-2.3.1-x64.exe'
while ($true) {
    while (-not (Get-Process capturax -ErrorAction SilentlyContinue)) { Start-Sleep -Milliseconds 100 }
    $csv = Join-Path $dir ('pm-{0:HHmmss}.csv' -f (Get-Date))
    Write-Host "capturing -> $csv"
    $p = Start-Process $pm -NoNewWindow -PassThru -ArgumentList '--session_name', 't052', `
        '--process_name', 'capturax.exe', '--qpc_time', '--v1_metrics', '--output_file', "`"$csv`"", `
        '--no_console_stats'
    while (Get-Process capturax -ErrorAction SilentlyContinue) { Start-Sleep -Milliseconds 200 }
    Start-Sleep 3
    if (-not $p.HasExited) {
        # A second PresentMon ends the session by name; the capturing one then flushes and exits.
        & $pm --session_name t052 --terminate_existing_session | Out-Null
        if (-not $p.WaitForExit(10000)) { Stop-Process -Id $p.Id; Write-Host 'PresentMon killed' }
    }
    Write-Host 'done, waiting for the next run'
}
