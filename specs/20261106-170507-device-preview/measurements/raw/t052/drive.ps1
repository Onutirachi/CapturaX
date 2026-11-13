# Drives one T052 run: starts capturax with the present log, ten focus changes, ten fullscreen
# entries and exits, 30 s idle in fullscreen, then closes it. Not elevated.
param(
    [Parameter(Mandatory = $true)][string]$Name,
    [string]$Env = '',          # e.g. WGPU_DX12_PRESENTATION_SYSTEM=visual
    [int]$Gap = 3,              # seconds between steps
    [switch]$Manual,            # only start and wait for the window to close
    [switch]$Probe,             # one focus change and one F11 round, then close
    [string]$Exe = ''           # run this copy, outputs next to it (C:\capx-t052\capturax.exe)
)
$ErrorActionPreference = 'Stop'
$dir = if ($Exe) { Split-Path $Exe } else { $PSScriptRoot }
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class W {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags, int x, int y, uint data, UIntPtr extra);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out R r);
  public struct R { public int L, T, Rt, B; }
  [DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint f);
  public static void Key(byte vk) { keybd_event(vk, 0, 0, UIntPtr.Zero); keybd_event(vk, 0, 2, UIntPtr.Zero); }
  // A real click activates the window, as a person clicking another window and back.
  // The point is the middle of the window's top band (its title bar).
  public struct P { public int X, Y; }
  [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(P p);
  [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint flags);
  public static string Last = "";
  public static bool Click(IntPtr h) {
    R r; GetWindowRect(h, out r);
    P p; p.X = (r.L + r.Rt) / 2; p.Y = r.T + 12;
    var top = GetAncestor(WindowFromPoint(p), 2);
    var s = new System.Text.StringBuilder(256); GetWindowText(top, s, 256);
    Last = "at " + p.X + "," + p.Y + " '" + s + "'";
    SetCursorPos((r.L + r.Rt) / 2, r.T + 12);
    System.Threading.Thread.Sleep(50);
    mouse_event(2, 0, 0, 0, UIntPtr.Zero); mouse_event(4, 0, 0, 0, UIntPtr.Zero);
    System.Threading.Thread.Sleep(100);
    return GetForegroundWindow() == h;
  }
  public static bool Focused(IntPtr h) { return GetForegroundWindow() == h; }
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
  public static string Fg() { var h = GetForegroundWindow(); var s = new System.Text.StringBuilder(256); GetWindowText(h, s, 256); R r; GetWindowRect(h, out r); return "fg='" + s + "' " + r.L + "," + r.T; }
}
'@
[void][W]::SetProcessDPIAware()
# Keep the PC and the displays awake while the run lasts (ES_CONTINUOUS | SYSTEM | DISPLAY).
[void][W]::SetThreadExecutionState([uint32]2147483651)
function Stamp($text) { $line = '{0:HH:mm:ss.fff} {1}' -f (Get-Date), $text; Write-Host $line; Add-Content "$dir\$Name-steps.txt" $line }

Remove-Item "$dir\$Name-steps.txt" -ErrorAction SilentlyContinue
if ($Env) { $k, $v = $Env -split '=', 2; Set-Item "env:$k" $v; Stamp "env $k=$v" }
$p = Start-Process "$dir\capturax.exe" -ArgumentList '--stats', '--present-log', "$dir\$Name-app.tsv" `
    -RedirectStandardOutput "$dir\$Name.txt" -RedirectStandardError "$dir\$Name.err.txt" -PassThru
Stamp "started pid=$($p.Id)"
# winit's hidden event target window comes first: wait for the titled main window.
for ($i = 0; $i -lt 100 -and $p.MainWindowTitle -ne 'CapturaX'; $i++) { Start-Sleep -Milliseconds 200; $p.Refresh() }
$app = $p.MainWindowHandle
if ($Manual) { Stamp 'manual: waiting for exit'; $p.WaitForExit(); Stamp 'exited'; return }

# Focus target: a window of this script, on the monitor below (Arzopa).
Add-Type -AssemblyName System.Windows.Forms
$form = New-Object System.Windows.Forms.Form
$form.Text = 't052 focus target'; $form.StartPosition = 'Manual'
$form.Location = New-Object System.Drawing.Point(100, 1180); $form.Size = New-Object System.Drawing.Size(500, 300)
$form.Show()
function Wait($s) { $end = (Get-Date).AddSeconds($s); while ((Get-Date) -lt $end) { [System.Windows.Forms.Application]::DoEvents(); Start-Sleep -Milliseconds 20 } }
Wait 10   # device open and warm-up
Stamp ("before " + [W]::Fg()); $ok = [W]::Click($app); Stamp "focus app $ok $([W]::Fg()) $([W]::Last)"
if (-not $ok) { $form.Close(); [void]$p.CloseMainWindow(); Stamp 'abort: no focus'; exit 2 }
Wait $Gap
if ($Probe) { Stamp ("out " + [W]::Click($form.Handle)); Wait 2; Stamp ("in " + [W]::Click($app)); Wait 2; [W]::Key(0x7A); Wait 3; [W]::Key(0x7A); Wait 2; $form.Close(); [void]$p.CloseMainWindow(); [void]$p.WaitForExit(10000); Stamp "probe exited=$($p.HasExited)"; return }

for ($n = 1; $n -le 10; $n++) {
    Stamp ("focus-out $n " + [W]::Click($form.Handle)); Wait $Gap
    Stamp ("focus-in $n " + [W]::Click($app)); Wait $Gap
}
for ($n = 1; $n -le 10; $n++) {
    [W]::Key(0x7A); Stamp "f11-in $n $([W]::Focused($app))"; Wait $Gap
    [W]::Key(0x7A); Stamp "f11-out $n $([W]::Focused($app))"; Wait $Gap
}
[W]::Key(0x7A); Stamp "idle-in $([W]::Focused($app))"
Wait 30
[W]::Key(0x7A); Stamp "idle-out $([W]::Focused($app))"
Wait $Gap

$form.Close()
[void]$p.CloseMainWindow()
if (-not $p.WaitForExit(10000)) { Stop-Process -Id $p.Id; Stamp 'killed' } else { Stamp 'closed' }
