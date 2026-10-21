# Runs openflip.scr the way Windows does: as screensaver (/s), settings (/c)
# and as preview (/p) inside another process's window, which must end the
# preview when it closes.
#
#   powershell -File platform\windows\smoke-test.ps1 target\windows\openflip.scr

param([Parameter(Mandatory)] [string] $Scr)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms

function Fail([string] $Message) {
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

function Expect-Running([System.Diagnostics.Process] $Process, [string] $What) {
    if ($Process.HasExited) { Fail "$What exited early with code $($Process.ExitCode)" }
    Write-Output "$What is running"
}

$saver = Start-Process -FilePath $Scr -ArgumentList '/s' -PassThru
Pump 5
Expect-Running $saver 'screensaver (/s)'
Stop-Process -Id $saver.Id

$settings = Start-Process -FilePath $Scr -ArgumentList '/c' -PassThru
Pump 5
Expect-Running $settings 'settings (/c)'
Stop-Process -Id $settings.Id

$form = New-Object System.Windows.Forms.Form
$form.Text = 'preview host'
$form.ClientSize = New-Object System.Drawing.Size(152, 112)
$form.Show()
Pump 1
$preview = Start-Process -FilePath $Scr -ArgumentList '/p', $form.Handle.ToInt64() -PassThru
Pump 5
Expect-Running $preview 'preview (/p)'
$form.Close()
Pump 5
if (-not $preview.HasExited) {
    Stop-Process -Id $preview.Id
    Fail 'preview kept running after its window closed'
}
Write-Output 'preview ended with its window'
Write-Output 'OK'
