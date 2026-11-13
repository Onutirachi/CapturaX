# T052: the app windowed and idle with --stats for N seconds, outputs next to the exe; then closed.
param([Parameter(Mandatory=$true)][string]$Name, [string]$Exe = "$PSScriptRoot\capturax.exe", [int]$Seconds = 60)
$dir = Split-Path $Exe
$p = Start-Process $Exe -ArgumentList '--stats', '--present-log', "$dir\$Name-app.tsv" `
    -RedirectStandardOutput "$dir\$Name.txt" -RedirectStandardError "$dir\$Name.err.txt" -PassThru
Start-Sleep $Seconds
[void]$p.CloseMainWindow()
if (-not $p.WaitForExit(10000)) { Stop-Process -Id $p.Id; 'killed' } else { 'closed' }
