# Drive the dev window with simulated input, behind the user's windows. It
# needs a dev build with the `sim` feature:
#
#   cargo run --features sim
#   & ./scripts/sim.ps1 -OutDir <scratch> -Steps @('keys:sopr','shot:narrowed','keys:{ESC}','shot:cleared')
#
# The steps are the ones drive.ps1 takes, at the same window coordinates. Each
# reply is printed: the picture path for `shot`, the result of `hover`, or an
# error. `hover` cannot read the cursor.

param(
    [Parameter(Mandatory = $true)][string[]]$Steps,
    [string]$OutDir = $env:TEMP,
    [int]$TimeoutMs = 5000
)

$ErrorActionPreference = 'Stop'
if (-not (Test-Path $OutDir)) { New-Item -ItemType Directory -Path $OutDir | Out-Null }
$OutDir = (Resolve-Path $OutDir).Path

$pipe = New-Object System.IO.Pipes.NamedPipeClientStream '.', 'book-buddy-dev-sim', 'InOut'
try {
    $pipe.Connect($TimeoutMs)
}
catch [TimeoutException] {
    throw 'no sim pipe: run a dev build with --features sim'
}
$writer = New-Object System.IO.StreamWriter $pipe
$writer.AutoFlush = $true
$reader = New-Object System.IO.StreamReader $pipe

try {
    foreach ($step in $Steps) {
        $kind, $value = $step -split ':', 2
        if ($kind -eq 'shot') { $step = "shot:$(Join-Path $OutDir "$value.png")" }
        $writer.WriteLine($step)
        $reply = $reader.ReadLine()
        if ($null -eq $reply) { throw 'the dev build closed the pipe' }
        if ($reply.StartsWith('error:')) { throw "${step}: $reply" }
        if ($kind -in 'shot', 'hover') { Write-Output $reply }
    }
}
finally {
    $pipe.Dispose()
}
