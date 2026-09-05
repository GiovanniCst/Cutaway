# What a clean Windows checks about a package, without a person in front of it.
#
# Runs inside Windows Sandbox as the logon command of the configuration run.ps1
# writes next to the report. Everything it learns goes to C:\report, which is a folder on
# the host: the sandbox is thrown away the moment its window closes, so a
# finding that stays inside it never happened.
#
# The screenshot flow is the part worth automating. On the 1.6 it was done by
# hand, twice, and the defect that mattered - PyInstaller's bootloader refusing
# to start the editor the agent had launched - only appeared because somebody
# actually pressed the key. Synthetic input is safe here in a way it is not on
# the host: the sandbox has its own input queue, so a keystroke sent in here
# cannot land on whatever the host is doing.

param(
    [ValidateSet('installed', 'portable')]
    [string]$Mode = 'installed'
)

$ErrorActionPreference = 'Continue'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

$report = 'C:\report'
Start-Transcript -Path (Join-Path $report "$Mode-trascrizione.txt") -Force | Out-Null
$log = Join-Path $report "$Mode.txt"
$lines = New-Object System.Collections.Generic.List[string]

function Say([string]$text) {
    $lines.Add($text)
    # Appending, not rewriting: the whole-file rewrite met itself between two
    # checks and threw "the file is being used by another process", which the
    # caller then reported as a failed check.
    for ($try = 0; $try -lt 5; $try++) {
        try { Add-Content -Path $log -Value $text -Encoding UTF8; return } catch { Start-Sleep -Milliseconds 120 }
    }
}

function Check([string]$what, [scriptblock]$test) {
    try {
        $answer = & $test
        if ($answer -is [bool]) {
            Say ("{0}  {1}" -f $(if ($answer) { 'OK  ' } else { 'NO  ' }), $what)
        } else {
            Say ("{0}  {1}: {2}" -f 'OK  ', $what, $answer)
        }
    } catch {
        Say ("NO    {0}: {1}" -f $what, $_.Exception.Message)
    }
}

function Shoot([string]$name) {
    $b = [System.Windows.Forms.SystemInformation]::VirtualScreen
    $bmp = New-Object System.Drawing.Bitmap($b.Width, $b.Height)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($b.X, $b.Y, 0, 0, $bmp.Size)
    $bmp.Save((Join-Path $report "$Mode-$name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
}

# Keyboard and mouse, and the two window calls the About check needs. PostMessage
# rather than a synthetic right-click for the tray menu: the menu item is a
# WM_COMMAND, and sending the command is what the menu does anyway.
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class Input {
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindowW(string cls, string title);
    [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p, IntPtr l);
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, System.Text.StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, System.Text.StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    public const uint KEYUP = 2, LEFTDOWN = 2, LEFTUP = 4;
    public const byte CTRL = 0x11, SNAPSHOT = 0x2C;
}
'@

# Runs something and waits for that process only.
#
# Start-Process -Wait waits for the process *and its descendants*, which here
# means forever: the setup leaves the agent resident behind it, and a resident
# program is exactly one that never exits. The first run of this script sat on a
# finished installation for nine minutes waiting for the agent to die.
function Wait-Just([string]$path, [string[]]$argv, [int]$seconds = 180) {
    $p = if ($argv.Count) { Start-Process -FilePath $path -ArgumentList $argv -PassThru }
         else { Start-Process -FilePath $path -PassThru }
    if (-not $p.WaitForExit($seconds * 1000)) {
        Say ("   ... {0} non e- uscito in {1} s" -f (Split-Path $path -Leaf), $seconds)
        return $false
    }
    return $true
}

# The agent's window by its class.
#
# It is an ordinary window that is never shown - message-only would not receive
# the session-change and power broadcasts it listens for - so it has a class and
# a title but no pixels. Walked rather than looked up with FindWindow, because
# when FindWindow came back empty there was no way to tell "not there" from
# "asked wrongly", and the first run of this script could not tell them apart.
# The dialog the agent has open, if it has one.
#
# Every dialog in Windows is class #32770 and there are always several about,
# most of them invisible and belonging to other programs. Asking for "the first
# #32770" therefore returns somebody else's: the OK meant for the removal notice
# went to a hidden window of another process, the notice stayed up, and the agent
# sat inside its modal loop looking - from outside - like a program that would
# not quit. Visible, and belonging to this process, is the whole fix.
function Dialog-Of([int]$owner) {
    $box = @([IntPtr]::Zero)
    $walk = [Input+EnumProc]{
        param($h, $l)
        if ($box[0] -ne [IntPtr]::Zero) { return $true }
        if (-not [Input]::IsWindowVisible($h)) { return $true }
        $sb = New-Object System.Text.StringBuilder 260
        [Input]::GetClassNameW($h, $sb, 260) | Out-Null
        if ($sb.ToString() -ne '#32770') { return $true }
        $whose = 0
        [Input]::GetWindowThreadProcessId($h, [ref]$whose) | Out-Null
        if ($whose -eq $owner) { $box[0] = $h }
        return $true
    }.GetNewClosure()
    [Input]::EnumWindows($walk, [IntPtr]::Zero) | Out-Null
    return $box[0]
}

function Title-Of([IntPtr]$h) {
    $sb = New-Object System.Text.StringBuilder 260
    [Input]::GetWindowTextW($h, $sb, 260) | Out-Null
    return $sb.ToString()
}

function Agent-Window {
    # A one-element array, because the callback runs in its own scope: writing
    # $script:something from inside it and reading a local of the same name
    # outside is two variables, and the second one is always zero. That is what
    # made this report "no agent window" while the agent was demonstrably
    # answering its hotkey.
    $box = @([IntPtr]::Zero)
    $walk = [Input+EnumProc]{
        param($h, $l)
        $sb = New-Object System.Text.StringBuilder 260
        [Input]::GetClassNameW($h, $sb, 260) | Out-Null
        if ($sb.ToString() -eq 'CutawayAgentWindow') { $box[0] = $h }
        return $true
    }.GetNewClosure()
    [Input]::EnumWindows($walk, [IntPtr]::Zero) | Out-Null
    return $box[0]
}

Say "Cutaway - $Mode"
Say (Get-Date -Format 'yyyy-MM-dd HH:mm:ss')
Say ((Get-CimInstance Win32_OperatingSystem).Caption + '  build ' + [System.Environment]::OSVersion.Version)
Say ''

# --- the package goes on ------------------------------------------------------

$appDir = ''
if ($Mode -eq 'installed') {
    $setup = 'C:\packages\Cutaway-Setup.exe'
    Say ("Setup: {0} ({1:N2} MB)" -f $setup, ((Get-Item $setup).Length / 1MB))
    $clock = [Diagnostics.Stopwatch]::StartNew()
    Wait-Just $setup @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/LOG=$report\setup.log") 300 | Out-Null
    $clock.Stop()
    Say ("Installato in {0:N1} s" -f $clock.Elapsed.TotalSeconds)
    $appDir = Join-Path $env:LOCALAPPDATA 'Programs\Cutaway'
} else {
    $zip = Get-ChildItem 'C:\packages\*portable*.zip' | Select-Object -First 1
    Say ("Portable: {0} ({1:N2} MB)" -f $zip.Name, ($zip.Length / 1MB))
    $where = Join-Path $env:USERPROFILE 'Desktop\Cutaway-portable'
    Expand-Archive -Path $zip.FullName -DestinationPath $where -Force
    $appDir = Join-Path $where 'Cutaway'
    # Nothing starts the agent for a portable copy: the editor does, on its
    # first run, and that is exactly what wants checking.
    Start-Process -FilePath (Join-Path $appDir 'Cutaway.exe')
    Start-Sleep -Seconds 6
}
Say "Cartella: $appDir"
Say ''

Check 'Cutaway.exe c-e' { Test-Path (Join-Path $appDir 'Cutaway.exe') }
Check 'CutawayAgent.exe c-e' { Test-Path (Join-Path $appDir 'CutawayAgent.exe') }
Check 'versione editor' { (Get-Item (Join-Path $appDir 'Cutaway.exe')).VersionInfo.FileVersion }
Check 'versione agente' { (Get-Item (Join-Path $appDir 'CutawayAgent.exe')).VersionInfo.FileVersion }
Check "icona nell'exe" {
    [System.Drawing.Icon]::ExtractAssociatedIcon((Join-Path $appDir 'Cutaway.exe')).Width -gt 0
}
if ($Mode -eq 'installed') {
    Check 'voce nel menu Start' {
        Test-Path (Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Cutaway.lnk')
    }
    Check 'disinstallazione registrata' {
        $k = Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall' -EA SilentlyContinue |
            ForEach-Object { Get-ItemProperty $_.PSPath } |
            Where-Object { $_.DisplayName -like 'Cutaway*' }
        if ($k) { "$($k.DisplayName) $($k.DisplayVersion)" } else { $false }
    }
    Check 'associazione .png' {
        (Get-ItemProperty 'HKCU:\Software\Classes\.png' -EA SilentlyContinue).'(default)'
    }
}

# --- the agent ----------------------------------------------------------------

Say ''
Start-Sleep -Seconds 3
Check 'agente residente' {
    $p = Get-Process CutawayAgent -EA SilentlyContinue
    if ($p) { "pid $($p.Id), $([math]::Round($p.WorkingSet64/1MB,1)) MB" } else { $false }
}
Check 'chiave di avvio automatico' {
    (Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -EA SilentlyContinue).Cutaway
}
Check 'finestra dell-agente' {
    $h = Agent-Window
    if ($h -ne [IntPtr]::Zero) { "hwnd $h" } else { $false }
}

# --- the editor draws ---------------------------------------------------------
#
# This is the question the sandbox exists to answer. The native build draws
# through OpenGL, and a machine with no 3D driver - a sandbox, a Remote Desktop
# session, an old Intel chip - may not have a context to give it. CUTAWAY_SHOT
# makes the program write its own frame buffer out and leave, so the file either
# appears or the window never drew.

Say ''
$shot = Join-Path $report "$Mode-finestra.png"
$env:CUTAWAY_SHOT = $shot
$clock = [Diagnostics.Stopwatch]::StartNew()
Wait-Just (Join-Path $appDir 'Cutaway.exe') @() 90 | Out-Null
$clock.Stop()
Check 'la finestra si disegna (OpenGL)' {
    if (Test-Path $shot) {
        $i = [System.Drawing.Image]::FromFile($shot)
        $size = "$($i.Width)x$($i.Height) in $([math]::Round($clock.Elapsed.TotalMilliseconds)) ms"
        $i.Dispose()
        $size
    } else { $false }
}
$env:CUTAWAY_SHOT = $null

$env:CUTAWAY_SHOT = Join-Path $report "$Mode-crediti.png"
$env:CUTAWAY_TOOL = 'about'
Wait-Just (Join-Path $appDir 'Cutaway.exe') @() 90 | Out-Null
Check "i crediti dell-editor" { Test-Path $env:CUTAWAY_SHOT }
$env:CUTAWAY_SHOT = $null
$env:CUTAWAY_TOOL = $null

# --- the shortcut, pressed for real -------------------------------------------

Say ''
$captures = Join-Path $env:LOCALAPPDATA 'Cutaway\captures'
$before = @(Get-ChildItem $captures -Directory -EA SilentlyContinue).Count

[Input]::keybd_event([Input]::CTRL, 0, 0, [UIntPtr]::Zero)
[Input]::keybd_event([Input]::SNAPSHOT, 0, 0, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 60
[Input]::keybd_event([Input]::SNAPSHOT, 0, [Input]::KEYUP, [UIntPtr]::Zero)
[Input]::keybd_event([Input]::CTRL, 0, [Input]::KEYUP, [UIntPtr]::Zero)
Start-Sleep -Seconds 2
Shoot 'overlay'

# A rectangle, dragged the way a hand drags one.
[Input]::SetCursorPos(300, 220) | Out-Null
Start-Sleep -Milliseconds 200
[Input]::mouse_event([Input]::LEFTDOWN, 0, 0, 0, [UIntPtr]::Zero)
foreach ($step in 1..12) {
    [Input]::SetCursorPos(300 + $step * 40, 220 + $step * 20) | Out-Null
    Start-Sleep -Milliseconds 30
}
[Input]::mouse_event([Input]::LEFTUP, 0, 0, 0, [UIntPtr]::Zero)
Start-Sleep -Seconds 6

Check 'la scorciatoia ha catturato' {
    if (-not [System.Windows.Forms.Clipboard]::ContainsImage()) { return $false }
    $i = [System.Windows.Forms.Clipboard]::GetImage()
    $i.Save((Join-Path $report "$Mode-ritaglio.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    $size = "$($i.Width)x$($i.Height)"
    $i.Dispose()
    # The rectangle dragged was 480 by 240 device-independent pixels.
    $size
}
Check 'la consegna non ha lasciato niente dietro' {
    # The editor deletes the folder the agent handed it, which is why the check
    # above reads the clipboard instead of looking for one.
    @(Get-ChildItem $captures -Directory -EA SilentlyContinue).Count -le $before
}
Check "l-editor si e- aperto sul ritaglio" {
    $p = Get-Process Cutaway -EA SilentlyContinue
    if ($p) { "$($p.Count) finestra/e" } else { $false }
}
Start-Sleep -Seconds 2
Shoot 'editor-sul-ritaglio'

# --- the agent says who made it ----------------------------------------------
#
# Last, because the dialog is modal and blocks the agent's message loop until it
# is dismissed. WM_COMMAND with the menu item's id: the same message the tray
# menu sends, without having to drive a menu.

Say ''
$hwnd = Agent-Window
Check 'la finestra dell-agente risponde ancora' { $hwnd -ne [IntPtr]::Zero }
if ($hwnd -ne [IntPtr]::Zero) {
    # 6 is the About item, 7 is "where Cutaway is". Both are modal and both
    # stop the agent's message loop until they are closed, so each is opened,
    # photographed and shut before the next.
    $owner = (Get-Process CutawayAgent -EA SilentlyContinue | Select-Object -First 1).Id
    foreach ($item in @(@{Id = 6; Name = 'crediti-agente'}, @{Id = 7; Name = 'dove-sta-cutaway'})) {
        [Input]::PostMessageW($hwnd, 0x0111, [IntPtr]$item.Id, [IntPtr]::Zero) | Out-Null
        Start-Sleep -Seconds 3
        $dlg = Dialog-Of $owner
        Shoot $item.Name
        Check ("l-agente apre " + $item.Name) {
            if ($dlg -eq [IntPtr]::Zero) { return $false }
            Title-Of $dlg
        }
        if ($dlg -ne [IntPtr]::Zero) {
            [Input]::PostMessageW($dlg, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
            Start-Sleep -Seconds 2
        }
    }
}

# --- and off again ------------------------------------------------------------
#
# Half of a package is how it leaves. What is looked for afterwards is what a
# person would find months later: a folder still there, an icon coming back at
# every logon, a file type still opening with a program that is gone.

Say ''
Say '--- disinstallazione'

Get-Process Cutaway -EA SilentlyContinue | Stop-Process -Force -EA SilentlyContinue
Start-Sleep -Seconds 1

if ($Mode -eq 'installed') {
    $unins = Get-ChildItem $appDir -Filter 'unins*.exe' -EA SilentlyContinue | Select-Object -First 1
    Check "l-uninstaller c-e" { $null -ne $unins }
    if ($unins) {
        $clock = [Diagnostics.Stopwatch]::StartNew()
        Wait-Just $unins.FullName @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/LOG=$report\uninstall.log") 300 | Out-Null
        $clock.Stop()
        # The uninstaller starts a second process to remove its own file and
        # returns before that one has finished.
        Start-Sleep -Seconds 6
        Say ("Disinstallato in {0:N1} s" -f $clock.Elapsed.TotalSeconds)
    }
} else {
    # The portable has no uninstaller of its own: what it has is the tray's
    # "Remove Cutaway from this computer", which is item 4. It asks first, then
    # says it is done, so both boxes get an OK - IDOK is 1 - and only then does
    # the folder go, by hand, the way a person would.
    $hwnd = Agent-Window
    $owner = (Get-Process CutawayAgent -EA SilentlyContinue | Select-Object -First 1).Id
    Check "la tray offre di rimuovere" { $hwnd -ne [IntPtr]::Zero }
    if ($hwnd -ne [IntPtr]::Zero) {
        [Input]::PostMessageW($hwnd, 0x0111, [IntPtr]4, [IntPtr]::Zero) | Out-Null
        Start-Sleep -Seconds 3
        $ask = Dialog-Of $owner
        Check "chiede conferma" {
            if ($ask -eq [IntPtr]::Zero) { return $false }
            Shoot 'chiede-conferma'
            Title-Of $ask
        }
        if ($ask -ne [IntPtr]::Zero) {
            [Input]::PostMessageW($ask, 0x0111, [IntPtr]1, [IntPtr]::Zero) | Out-Null
            # Then the one that says it is done. Pressed until no box is left:
            # each of these blocks the agent's message loop, so one still open
            # is an agent that never gets as far as closing itself - which is
            # what the first run of this looked like, and it looked like the
            # product failing rather than the test.
            $shot = $false
            foreach ($again in 1..12) {
                Start-Sleep -Seconds 1
                if (-not (Get-Process -Id $owner -EA SilentlyContinue)) { break }
                $box = Dialog-Of $owner
                if ($box -eq [IntPtr]::Zero) { break }
                if (-not $shot) { Shoot 'rimosso'; $shot = $true }
                # Both, because one of them is not enough. WM_COMMAND with IDOK
                # presses the button, which is what the OK/Cancel box needs; the
                # box that only says "done" answers to WM_CLOSE, which for a
                # single-button message box is the same as pressing it.
                [Input]::PostMessageW($box, 0x0111, [IntPtr]1, [IntPtr]::Zero) | Out-Null
                [Input]::PostMessageW($box, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
            }
            Check "nessuna finestra rimasta aperta" {
                -not (Get-Process -Id $owner -EA SilentlyContinue) -or
                (Dialog-Of $owner) -eq [IntPtr]::Zero
            }
        }
        Start-Sleep -Seconds 6
    }
    Check "l-agente se n-e- andato" { $null -eq (Get-Process CutawayAgent -EA SilentlyContinue) }
    Remove-Item (Split-Path $appDir -Parent) -Recurse -Force -EA SilentlyContinue
    # The deferred sweep the agent leaves behind waits a few seconds for its own
    # executable to be unmapped before removing its folder.
    Start-Sleep -Seconds 12
}

Start-Sleep -Seconds 2
Check "la cartella non c-e- piu-" { -not (Test-Path (Join-Path $appDir 'Cutaway.exe')) }
Check "nessun agente in esecuzione" { $null -eq (Get-Process CutawayAgent -EA SilentlyContinue) }
Check "nessuna chiave di avvio automatico" {
    $null -eq (Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run' -EA SilentlyContinue).Cutaway
}
Check "niente resta sotto LOCALAPPDATA" {
    # Waited for rather than looked at once: the agent cannot remove the folder
    # its own executable is running from, so it leaves a detached command that
    # waits for the process to be unmapped and then sweeps. Two attempts, about
    # nine seconds apart.
    $root = Join-Path $env:LOCALAPPDATA 'Cutaway'
    # Whatever belongs to the person stays, and should: settings and API keys
    # are theirs, and an uninstall that took them would be a bug.
    $theirs = @('settings.json', 'keys.json', 'models.json', 'measurements.json')
    $deadline = (Get-Date).AddSeconds(30)
    do {
        $left = @(Get-ChildItem $root -Recurse -EA SilentlyContinue |
            Where-Object { $theirs -notcontains $_.Name })
        if ($left.Count -eq 0) { return $true }
        Start-Sleep -Seconds 2
    } while ((Get-Date) -lt $deadline)
    # Carried out of the sandbox, because the sandbox is about to stop existing
    # and "something was left behind" without saying what is not a finding.
    foreach ($file in $left | Where-Object { -not $_.PSIsContainer }) {
        Copy-Item $file.FullName (Join-Path $report ("resto-" + $file.Name)) -EA SilentlyContinue
    }
    "restano $($left.Count) elementi: " + (($left | Select-Object -First 6 -ExpandProperty Name) -join ', ')
}
if ($Mode -eq 'installed') {
    Check "nessuna voce nel menu Start" {
        -not (Test-Path (Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Cutaway.lnk'))
    }
    Check "nessuna associazione .png rimasta" {
        (Get-ItemProperty 'HKCU:\Software\Classes\.png' -EA SilentlyContinue).'(default)' -ne 'Cutaway.Image'
    }
    Check "nessuna voce di disinstallazione" {
        $k = Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall' -EA SilentlyContinue |
            ForEach-Object { Get-ItemProperty $_.PSPath } |
            Where-Object { $_.DisplayName -like 'Cutaway*' }
        $null -eq $k
    }
}

Say ''
Say 'Finito. Le immagini sono accanto a questo file.'
Shoot 'fine'

# The whole thing, written once at the end.
#
# Appending is what the running log does, and a write that lost a race with the
# one before it lost that line for good: a check ran, passed, and simply was not
# in the report. Everything said is still in memory here, so the file is put
# right before the sandbox goes.
for ($try = 0; $try -lt 20; $try++) {
    try { Set-Content -Path $log -Value $lines -Encoding UTF8; break } catch { Start-Sleep -Milliseconds 150 }
}
Stop-Transcript | Out-Null
