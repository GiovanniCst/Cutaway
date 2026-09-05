# Runs one package through a clean Windows and closes the sandbox behind it.
#
#   powershell -File tools\sandbox\run.ps1 -Mode installed
#   powershell -File tools\sandbox\run.ps1 -Mode portable
#
# Windows runs one sandbox at a time, so this refuses to start a second and
# stops the one it started when the report says it is done. Leaving it open is
# what made the second run fail the first time this was used.
#
# The .wsb file is written here rather than kept in the repository: a sandbox
# configuration is a list of absolute paths on one machine, and a checked-in one
# would be both wrong for anybody else and a note about where this particular
# copy happens to live.

param(
    [ValidateSet('installed', 'portable')]
    [string]$Mode = 'installed',
    [int]$MinutesAtMost = 12
)

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$root = (Resolve-Path (Join-Path $here '..\..')).Path
$report = Join-Path $root "dist\prove\$Mode"
$log = Join-Path $report "$Mode.txt"
$packages = Join-Path $root 'dist\2.0'
$wsb = Join-Path $report "$Mode.wsb"

if (Get-Process vmmemWindowsSandbox -EA SilentlyContinue) {
    Write-Error "Una sandbox e' gia' aperta: Windows ne consente una sola. Chiudila e riprova."
    exit 1
}
if (-not (Test-Path (Join-Path $packages 'Cutaway-Setup.exe'))) {
    Write-Error "Mancano i pacchetti in ${packages}: esegui prima node tools\build-2.0.mjs"
    exit 1
}

New-Item -ItemType Directory -Path $report -Force | Out-Null
Get-ChildItem $report -EA SilentlyContinue | Remove-Item -Recurse -Force

# The packages read-only, this folder read-only, and one folder the sandbox can
# write back through: the sandbox is thrown away when its window closes, so a
# finding that stays inside it never happened.
@"
<Configuration>
  <VGpu>Default</VGpu>
  <Networking>Default</Networking>
  <MemoryInMB>4096</MemoryInMB>
  <MappedFolders>
    <MappedFolder>
      <HostFolder>$packages</HostFolder>
      <SandboxFolder>C:\packages</SandboxFolder>
      <ReadOnly>true</ReadOnly>
    </MappedFolder>
    <MappedFolder>
      <HostFolder>$here</HostFolder>
      <SandboxFolder>C:\prova</SandboxFolder>
      <ReadOnly>true</ReadOnly>
    </MappedFolder>
    <MappedFolder>
      <HostFolder>$report</HostFolder>
      <SandboxFolder>C:\report</SandboxFolder>
      <ReadOnly>false</ReadOnly>
    </MappedFolder>
  </MappedFolders>
  <LogonCommand>
    <Command>powershell.exe -ExecutionPolicy Bypass -NoProfile -File C:\prova\check.ps1 -Mode $Mode</Command>
  </LogonCommand>
</Configuration>
"@ | Set-Content -Path $wsb -Encoding UTF8

Write-Host "Sandbox ${Mode}: apro..."
Start-Process WindowsSandbox.exe -ArgumentList $wsb

$deadline = (Get-Date).AddMinutes($MinutesAtMost)
$done = $false
while ((Get-Date) -lt $deadline) {
    if (Test-Path $log) {
        if ((Get-Content $log -Raw -EA SilentlyContinue) -match 'Finito\.') { $done = $true; break }
    }
    Start-Sleep -Seconds 5
}

Write-Host "Sandbox ${Mode}: chiudo..."
Get-Process WindowsSandboxRemoteSession -EA SilentlyContinue | Stop-Process -Force
$closing = (Get-Date).AddSeconds(90)
while ((Get-Date) -lt $closing -and (Get-Process vmmemWindowsSandbox -EA SilentlyContinue)) {
    Start-Sleep -Seconds 3
}

if (Test-Path $log) { Get-Content $log } else { Write-Host "Nessun referto: lo script dentro non e' partito." }
if (-not $done) { Write-Host "`n(la prova non ha scritto 'Finito': quello sopra e' quanto era arrivato)" }
