# Test de l'application Windows sur une machine propre (CI) : installation silencieuse,
# lancement, page servie par le proxy local, session gardée (chiffrée) après redémarrage,
# lien looty://, lancement caché, hors ligne puis retour, désinstallation propre.
# Ce qu'un runner ne peut pas vérifier (clic sur une notification, menu de la barre des
# tâches, raccourcis clavier) est dans desktop/tests/windows/CHECKLIST.md.
#   pwsh smoke.ps1 -Installer <Looty_x.y.z_x64-setup.exe> -Out <dossier des résultats>
param(
  [Parameter(Mandatory)] [string] $Installer,
  [string] $Out = "$PWD\smoke-out"
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $Out | Out-Null
$ServerPort = 18080
$ProxyPort = 18555
$Log = Join-Path $Out 'fake-server.log'
$failures = 0

function Check([string] $name, [bool] $ok, [string] $detail = '') {
  if ($ok) { Write-Host "PASS $name" } else { Write-Host "FAIL $name $detail"; $script:failures++ }
}
function Wait-Until([scriptblock] $cond, [int] $seconds = 60) {
  $end = (Get-Date).AddSeconds($seconds)
  while ((Get-Date) -lt $end) { if (& $cond) { return $true }; Start-Sleep -Milliseconds 500 }
  return $false
}
function Server-Log { if (Test-Path $Log) { Get-Content $Log -Raw } else { '' } }
function Start-Server {
  $script:server = Start-Process node -ArgumentList "`"$PSScriptRoot\fake-server.mjs`"", $ServerPort, "`"$Log`"" -PassThru -WindowStyle Hidden
  Wait-Until { (Server-Log) -match 'listening' } 15 | Out-Null
}
function Looty-Processes { Get-Process -Name looty -ErrorAction SilentlyContinue }
function Stop-Looty {
  Looty-Processes | Stop-Process -Force -ErrorAction SilentlyContinue
  Wait-Until { -not (Looty-Processes) } 15 | Out-Null
}
function Screenshot([string] $name) {
  try { Take-Screenshot $name } catch { Write-Host "screenshot $name unavailable: $_" }
}
function Take-Screenshot([string] $name) {
  Add-Type -AssemblyName System.Windows.Forms, System.Drawing
  $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
  $bmp.Save((Join-Path $Out "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
}

# --- Installation (pour l'utilisateur, sans droits administrateur) ---
Start-Server
$p = Start-Process $Installer -ArgumentList '/S' -PassThru -Wait
Check 'silent install exits cleanly' ($p.ExitCode -eq 0) "exit $($p.ExitCode)"
$exe = Get-ChildItem "$env:LOCALAPPDATA" -Filter looty.exe -Recurse -Depth 3 -ErrorAction SilentlyContinue | Select-Object -First 1
Check 'looty.exe installed in the user profile' ($null -ne $exe) $env:LOCALAPPDATA
if (-not $exe) { exit 1 }
Write-Host "installed: $($exe.FullName)"
$cmd = (Get-ItemProperty 'HKCU:\Software\Classes\looty\shell\open\command' -ErrorAction SilentlyContinue).'(default)'
Check 'looty:// links registered' ($cmd -like "*$($exe.Name)*%1*") "$cmd"
Check 'notification identity registered' (Test-Path 'HKCU:\Software\Classes\AppUserModelId\app.looty.desktop')
$uninstall = Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall' | ForEach-Object { Get-ItemProperty $_.PSPath } | Where-Object { $_.DisplayName -like 'Looty*' } | Select-Object -First 1
Check 'listed in installed apps' ($null -ne $uninstall)

# --- Configuration : serveur de test, port local fixe ---
$cfgDir = Join-Path $env:APPDATA 'looty'
New-Item -ItemType Directory -Force -Path $cfgDir | Out-Null
@"
server = "http://127.0.0.1:$ServerPort"
port = $ProxyPort
ask_download_location = false
global_shortcuts = false
"@ | Set-Content -Encoding utf8 (Join-Path $cfgDir 'config.toml')

# --- Premier lancement ---
Start-Process $exe.FullName | Out-Null
Check 'app starts and stays running' (Wait-Until { Looty-Processes } 20)
Check 'window loads the page through the local proxy' (Wait-Until { (Server-Log) -match 'GET /api/hit\?from=%2F' } 60) (Server-Log)
Check 'page signs in (session cookie kept by the app)' (Wait-Until { (Server-Log) -match 'GET /api/hit\?from=signed-in' } 30)
Start-Sleep 2
$win = Looty-Processes | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
Check 'main window visible' ($null -ne $win) ((Looty-Processes | ForEach-Object { "$($_.Id):$($_.MainWindowTitle)" }) -join ', ')
Screenshot '1-first-launch'
# Le proxy n'obéit qu'à la fenêtre de l'application (cookie à secret).
try { $r = Invoke-WebRequest "http://127.0.0.1:$ProxyPort/" -UseBasicParsing -SkipHttpErrorCheck; $code = [int]$r.StatusCode } catch { $code = [int]$_.Exception.Response.StatusCode }
Check 'local proxy refuses other programs' ($code -eq 403) "status $code"
Check 'background service reads tasks and notifications' (Wait-Until { (Server-Log) -match 'GET /api/workspaces/w1/my_tasks session=yes' } 90)

# --- Lien looty:// vers l'instance ouverte ---
Start-Process 'looty://w/w1/doc/p42' | Out-Null
Check 'looty:// link opens the page in the running app' (Wait-Until { (Server-Log) -match 'GET /api/hit\?from=%2Fw%2Fw1%2Fdoc%2Fp42' } 30)
Check 'still a single instance' (@(Looty-Processes | Where-Object { $_.MainWindowHandle -ne 0 }).Count -le 1)
Screenshot '2-looty-link'

# --- Session gardée après redémarrage, et chiffrée sur le disque ---
Stop-Looty
# Clé de chiffrement dans le gestionnaire d'identifiants Windows : session.enc ; session.json
# (en clair) n'est que le repli sans trousseau.
$session = Join-Path $env:APPDATA 'looty\session.enc'
Check 'session saved, encrypted' (Test-Path $session)
Check 'no clear-text session file' (-not (Test-Path (Join-Path $env:APPDATA 'looty\session.json')))
if (Test-Path $session) { Check 'session token not readable on disk' (-not ((Get-Content $session -Raw) -match 'fake-session-7f3a')) }
$before = (Server-Log).Length
Start-Process $exe.FullName -ArgumentList '--hidden' | Out-Null
Check 'hidden start (login launch) runs without a window' ((Wait-Until { Looty-Processes } 20) -and -not (Looty-Processes | Where-Object { $_.MainWindowHandle -ne 0 }))
Check 'session restored after restart' (Wait-Until { (Server-Log).Substring($before) -match 'GET /api/auth/me session=yes' } 60)
Stop-Looty

# --- Hors ligne, puis retour du serveur ---
Start-Process $exe.FullName | Out-Null
Wait-Until { (Server-Log) -match 'signed-in' } 30 | Out-Null
Stop-Process -Id $server.Id -Force
Start-Sleep 5
Check 'app survives the server going away' ($null -ne (Looty-Processes))
Screenshot '3-offline'
$before = (Server-Log).Length
Start-Server
Check 'app reconnects when the server is back' (Wait-Until { (Server-Log).Substring($before) -match 'session=yes' } 120)
Stop-Looty

# --- Désinstallation ---
$un = $uninstall.UninstallString -replace '"', ''
$p = Start-Process $un -ArgumentList '/S' -PassThru -Wait
Start-Sleep 3
Check 'silent uninstall exits cleanly' ($p.ExitCode -eq 0) "exit $($p.ExitCode)"
Check 'program removed' (Wait-Until { -not (Test-Path $exe.FullName) } 30)
# Le désinstallateur NSIS se recopie puis rend la main : on attend qu'il ait fini.
Check 'looty:// links removed' (Wait-Until { -not (Test-Path 'HKCU:\Software\Classes\looty') } 30)
Check 'launch at login removed' ($null -eq (Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -Name Looty -ErrorAction SilentlyContinue))

Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue
Write-Host "$failures failure(s)"
exit $failures
