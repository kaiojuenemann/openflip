# Runs openflip.scr the way Windows does: as screensaver (/s), settings (/c)
# and as preview (/p) inside another process's window, which must end the
# preview when it closes.
#
#   powershell -File platform\windows\smoke-test.ps1 target\windows\openflip.scr

param([Parameter(Mandatory)] [string] $Scr)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
$env:RUST_BACKTRACE = '1'
$env:RUST_LOG = 'warn'
$logs = Join-Path ([System.IO.Path]::GetTempPath()) 'openflip-smoke'
New-Item -ItemType Directory -Force $logs | Out-Null

# Starts openflip with its output captured, so a failure can show why.
function Start-Openflip([string[]] $Arguments, [string] $Name) {
    Start-Process -FilePath $Scr -ArgumentList $Arguments -PassThru `
        -RedirectStandardOutput (Join-Path $logs "$Name.out") `
        -RedirectStandardError (Join-Path $logs "$Name.err")
}

function Show-Output([string] $Name) {
    foreach ($stream in 'out', 'err') {
        $file = Join-Path $logs "$Name.$stream"
        if ((Test-Path $file) -and (Get-Item $file).Length -gt 0) {
            Write-Output "--- $Name std$stream ---"
            Get-Content $file | Write-Output
        }
    }
}

function Fail([string] $Message, [string] $Name) {
    if ($Name) { Show-Output $Name }
    Write-Error "smoke test failed: $Message"
    exit 1
}

# Keeps this process's windows responsive while waiting: the preview's
# window is a child of ours, and Windows routes some of its messages here.
function Pump([double] $Seconds) {
    $until = (Get-Date).AddSeconds($Seconds)
    while ((Get-Date) -lt $until) {
        [System.Windows.Forms.Application]::DoEvents()
        Start-Sleep -Milliseconds 20
    }
}

function Expect-Running([System.Diagnostics.Process] $Process, [string] $What, [string] $Name) {
    if ($Process.HasExited) { Fail "$What exited early with code $($Process.ExitCode)" $Name }
    Write-Output "$What is running"
}

$saver = Start-Openflip '/s' 'saver'
Pump 5
Expect-Running $saver 'screensaver (/s)' 'saver'
Stop-Process -Id $saver.Id
$saver.WaitForExit()
Show-Output 'saver'

$settings = Start-Openflip '/c' 'settings'
Pump 5
Expect-Running $settings 'settings (/c)' 'settings'
Stop-Process -Id $settings.Id
$settings.WaitForExit()
Show-Output 'settings'

$form = New-Object System.Windows.Forms.Form
$form.Text = 'preview host'
$form.ClientSize = New-Object System.Drawing.Size(152, 112)
$form.Show()
Pump 1
$preview = Start-Openflip @('/p', $form.Handle.ToInt64()) 'preview'
Pump 5
Expect-Running $preview 'preview (/p)' 'preview'
$form.Close()
Pump 5
if (-not $preview.HasExited) {
    Stop-Process -Id $preview.Id
    Fail 'preview kept running after its window closed' 'preview'
}
Show-Output 'preview'
Write-Output 'preview ended with its window'
Write-Output 'OK'
