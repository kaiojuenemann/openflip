# Runs openflip.scr the way Windows does: as screensaver (/s), settings (/c)
# and as preview (/p) inside another process's window, which must end the
# preview when it closes; also the app fullscreen and windowed. Every run
# must keep running and be able to draw into its window.
#
#   powershell -File platform\windows\smoke-test.ps1 target\windows\openflip.scr

param([Parameter(Mandatory)] [string] $Scr)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
$env:RUST_BACKTRACE = '1'
$env:RUST_LOG = 'warn,openflip=info'
$failures = [System.Collections.Generic.List[string]]::new()
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

# Records a failure and goes on, so one run reports every broken mode.
function Fail([string] $Message) {
    Write-Output "FAILED: $Message"
    $failures.Add($Message)
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

function Expect-Running([System.Diagnostics.Process] $Process, [string] $What) {
    if ($Process.HasExited) {
        Fail "$What exited early with code $($Process.ExitCode)"
    } else {
        Write-Output "$What is running"
    }
}

function Stop-Openflip([System.Diagnostics.Process] $Process, [string] $Name, [string] $What) {
    if (-not $Process.HasExited) { Stop-Process -Id $Process.Id }
    $Process.WaitForExit()
    Show-Output $Name
    $err = Join-Path $logs "$Name.err"
    if ((Test-Path $err) -and (Select-String -Path $err -SimpleMatch 'cannot draw into the window' -Quiet)) {
        Fail "$What could not draw into its window"
    }
}

# Runs one mode for a few seconds, expecting it to stay up and draw.
function Test-Mode([string[]] $Arguments, [string] $Name, [string] $What) {
    $process = Start-Openflip $Arguments $Name
    Pump 5
    Expect-Running $process $What
    Stop-Openflip $process $Name $What
}

Test-Mode '/s' 'saver' 'screensaver (/s)'
Test-Mode '/c' 'settings' 'settings (/c)'
Test-Mode '--fullscreen' 'app' 'app (fullscreen)'
Test-Mode '--windowed' 'windowed' 'app (windowed)'

$form = New-Object System.Windows.Forms.Form
$form.Text = 'preview host'
$form.ClientSize = New-Object System.Drawing.Size(152, 112)
$form.Show()
Pump 1
$preview = Start-Openflip @('/p', $form.Handle.ToInt64()) 'preview'
Pump 5
Expect-Running $preview 'preview (/p)'
$form.Close()
Pump 5
if ($preview.HasExited) {
    Write-Output 'preview ended with its window'
} else {
    Fail 'preview kept running after its window closed'
}
Stop-Openflip $preview 'preview' 'preview (/p)'

if ($failures.Count -gt 0) {
    Write-Error "smoke test failed: $($failures -join '; ')"
    exit 1
}
Write-Output 'OK'
