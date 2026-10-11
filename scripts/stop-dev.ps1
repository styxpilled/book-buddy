# Stop the development build and its worker processes, and nothing else. The
# user's own release build has the same process names, `book-buddy`,
# so a process is picked by the full path of its executable, never by name.
#
#   & ./scripts/stop-dev.ps1

$dir = (Resolve-Path (Join-Path $PSScriptRoot '..\target\debug')).Path + '\'
Get-Process book-buddy -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -and $_.Path.StartsWith($dir, [StringComparison]::OrdinalIgnoreCase) } |
    ForEach-Object {
        Write-Output "stop $($_.Id) $($_.Path)"
        Stop-Process -Id $_.Id -Force
    }
