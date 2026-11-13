param([string]$Out, [int]$Seconds = 120)
$end = (Get-Date).AddSeconds($Seconds)
while ((Get-Date) -lt $end) {
    $t0 = (Get-Date).ToUniversalTime().ToString('HH:mm:ss.fff')
    $txt = gst-device-monitor-1.0 Video/Source 2>$null | Out-String
    $t1 = (Get-Date).ToUniversalTime().ToString('HH:mm:ss.fff')
    $has = $txt -match 'mfvideosrc device-path="[^"]*vid_046d&pid_082d'
    "{0} {1} c920-in-fresh-mf-list={2}" -f $t0, $t1, $has | Add-Content $Out
}
