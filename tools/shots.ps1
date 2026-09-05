# The eight pictures in the README, taken by the program of itself.
#
#   powershell -File tools\shots.ps1
#
# Two languages, two themes, two views. Taken rather than staged: the window
# writes out its own frame buffer when CUTAWAY_SHOT names a file, and opens a
# panel when CUTAWAY_TOOL names one, so what lands in assets/ is the program as
# it actually draws - including the composition it opens with, which is
# different in every one of them because it is generated on each run.
#
# The settings file is shared with the program and belongs to whoever is using
# it, so it is put back exactly as it was before this script started.

$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) '..')
$exe = Join-Path $root 'editor-rs\target\release\Cutaway.exe'
$assets = Join-Path $root 'assets'
$settings = Join-Path $env:LOCALAPPDATA 'Cutaway\settings.json'

if (-not (Test-Path $exe)) { throw "Manca ${exe}: compila prima con cargo build --release" }

$mine = if (Test-Path $settings) { Get-Content $settings -Raw } else { $null }

try {
    foreach ($language in 'en', 'it') {
        foreach ($theme in 'light', 'dark') {
            # WriteAllText and not Set-Content: Windows PowerShell's UTF8 has
            # a byte order mark, and the program used to stop at it and fall
            # back to its defaults - which is how the light screenshots came
            # out dark and in the wrong language. The program tolerates the
            # mark now; the file still should not have one.
            [System.IO.File]::WriteAllText(
                $settings,
                "{`n  `"language`": `"$language`",`n  `"theme`": `"$theme`"`n}",
                (New-Object System.Text.UTF8Encoding $false)
            )
            foreach ($view in @(@{Tool = ''; Name = 'start'}, @{Tool = 'ai'; Name = 'ai'})) {
                # -it on the Italian ones, nothing on the English: the names the
                # README already points at.
                $suffix = if ($language -eq 'it') { '-it' } else { '' }
                $file = Join-Path $assets "$($view.Name)-$theme$suffix.png"
                $env:CUTAWAY_SHOT = $file
                $env:CUTAWAY_TOOL = $view.Tool
                $p = Start-Process -FilePath $exe -PassThru
                if (-not $p.WaitForExit(60000)) { throw "${file}: la finestra non si e' chiusa" }
                if (-not (Test-Path $file)) { throw "${file}: non e' stato scritto" }
                $size = [System.Drawing.Image]::FromFile($file)
                Write-Host ("{0,-28} {1}x{2}" -f (Split-Path $file -Leaf), $size.Width, $size.Height)
                $size.Dispose()
            }
        }
    }
} finally {
    $env:CUTAWAY_SHOT = $null
    $env:CUTAWAY_TOOL = $null
    if ($null -ne $mine) {
        [System.IO.File]::WriteAllText($settings, $mine, (New-Object System.Text.UTF8Encoding $false))
    }
    else { Remove-Item $settings -EA SilentlyContinue }
}
