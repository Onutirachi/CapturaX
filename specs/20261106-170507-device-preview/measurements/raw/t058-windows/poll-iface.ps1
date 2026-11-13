param([string]$Out, [int]$Seconds = 120)
$guid = '{e5323777-f976-4f5b-9b55-b94699c46e44}'
$prev = $null
$end = (Get-Date).AddSeconds($Seconds)
while ((Get-Date) -lt $end) {
    $txt = pnputil /enum-interfaces /class $guid /enabled | Out-String
    $has = $txt -match 'vid_046d&pid_082d'
    if ($has -ne $prev) {
        "{0} c920-interface-enabled={1}" -f (Get-Date).ToUniversalTime().ToString('HH:mm:ss.fff'), $has | Add-Content $Out
        $prev = $has
    }
    Start-Sleep -Milliseconds 50
}
