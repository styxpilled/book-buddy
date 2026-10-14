# Samples the memory of book-buddy.exe to a CSV, once per interval, until
# stopped or until -Seconds pass.
#
#   & ./scripts/memory.ps1 -Out <scratch>\idle.csv -Seconds 300
#
# Columns are in MB, except handles. GPU memory comes from the
# "GPU Process Memory" counters, which list each process as pid_<n>_luid_...

param(
    [Parameter(Mandatory)] [string] $Out,
    [int] $Seconds = 0,
    [double] $Interval = 1
)

$names = 'book-buddy'
"time,seconds,process,pid,private_mb,working_set_mb,gpu_dedicated_mb,gpu_shared_mb,handles" |
    Set-Content -Path $Out -Encoding utf8

$start = Get-Date
while ($Seconds -le 0 -or ((Get-Date) - $start).TotalSeconds -lt $Seconds) {
    $tick = Get-Date
    $gpu = @{}
    $samples = (Get-Counter '\GPU Process Memory(*)\Dedicated Usage', '\GPU Process Memory(*)\Shared Usage' -ErrorAction SilentlyContinue).CounterSamples
    foreach ($sample in $samples) {
        if ($sample.InstanceName -match '^pid_(\d+)_') {
            $id = [int]$Matches[1]
            if (-not $gpu[$id]) { $gpu[$id] = @{ dedicated = 0; shared = 0 } }
            $kind = if ($sample.Path -like '*dedicated usage') { 'dedicated' } else { 'shared' }
            $gpu[$id][$kind] += $sample.CookedValue
        }
    }
    $lines = foreach ($process in Get-Process -Name $names -ErrorAction SilentlyContinue) {
        $memory = $gpu[$process.Id]
        '{0},{1:F1},{2},{3},{4:F1},{5:F1},{6:F1},{7:F1},{8}' -f
            $tick.ToString('HH:mm:ss'),
            ($tick - $start).TotalSeconds,
            $process.ProcessName,
            $process.Id,
            ($process.PrivateMemorySize64 / 1MB),
            ($process.WorkingSet64 / 1MB),
            ($(if ($memory) { $memory.dedicated } else { 0 }) / 1MB),
            ($(if ($memory) { $memory.shared } else { 0 }) / 1MB),
            $process.HandleCount
    }
    if ($lines) { $lines | Add-Content -Path $Out -Encoding utf8 }
    $wait = $Interval - ((Get-Date) - $tick).TotalSeconds
    if ($wait -gt 0) { Start-Sleep -Milliseconds ([int]($wait * 1000)) }
}
