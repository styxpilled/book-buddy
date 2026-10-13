# Drive the application window for a check: cover the desktop with the terminal,
# bring the application forward, send keystrokes and save a picture of its
# window. The terminal goes back to the size it had when this started.
#
#   & ./scripts/drive.ps1 -Steps @('keys:sopr','shot:narrowed','keys:{ESC}','shot:cleared')
#
# Steps run in order:
#   keys:<SendKeys string>   what to type. See SendKeys for {ESC}, {DOWN}, {TAB}
#   kdown:<virtual key>      press a key and hold it, by its virtual-key code
#                            in decimal: 32 is space. A second kdown of the
#                            same key is a key repeat
#   kup:<virtual key>        release it
#   click:<x>,<y>            click at a point inside the window, measured from
#                            its top left corner, which is where a picture of
#                            the window measures from too
#   dblclick:<x>,<y>         the same, twice, fast enough to be a double click
#   down:<x>,<y>             press the left button at a point, for a drag
#   move:<x>,<y>             move the pointer there in small steps, so a drag
#                            sees the moves between
#   up                       release the left button
#   mclick:<x>,<y>           click the middle button at a point
#   rclick:<x>,<y>           click the right button at a point
#   bclick:<x>,<y>           click the back button (XBUTTON1) at a point
#   mdown:<x>,<y>            press the middle button at a point
#   mup                      release the middle button
#   scroll:<x>,<y>,<notches> turn the wheel over a point inside the window.
#                            A positive count turns it away from the hand
#   hover:<x>,<y>            move the pointer to a point inside the window and
#                            print the cursor it shows there: hand, arrow,
#                            updown or other. A picture does not hold the cursor
#   shot:<name>              save <OutDir>/<name>.png of the application window
#   wait:<milliseconds>      wait longer than the settle time
#
# The window is picked by its exact title. A debug build is "book-buddy dev",
# and the user's own release build is "book-buddy", so the script never drives
# the user's window. It refuses to start if the dev window is not open.
#
# Windows refuses SetForegroundWindow to a process that is not already in front,
# and it returns False without an error. AttachThreadInput to the thread that
# owns the foreground window lifts that, which is the whole trick here.

param(
    [Parameter(Mandatory = $true)][string[]]$Steps,
    [string]$OutDir = $env:TEMP,
    [string]$Title = 'book-buddy dev',
    [int]$SettleMs = 700
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms, System.Drawing

$signature = @'
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
[DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr hWnd);
[DllImport("user32.dll")] public static extern IntPtr SetActiveWindow(IntPtr hWnd);
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
[DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr hWnd);
[DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hWnd);
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
[DllImport("user32.dll")] public static extern bool AttachThreadInput(uint idAttach, uint idAttachTo, bool fAttach);
[DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT r);
[DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
[DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
[DllImport("user32.dll")] public static extern bool GetCursorInfo(ref CURSORINFO info);
[DllImport("user32.dll")] public static extern IntPtr LoadCursor(IntPtr instance, IntPtr name);
[DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
public struct RECT { public int Left, Top, Right, Bottom; }
public struct POINT { public int X, Y; }
public struct CURSORINFO { public int Size; public int Flags; public IntPtr Cursor; public POINT Position; }
'@
$null = Add-Type -MemberDefinition $signature -Name Win -Namespace Drive -PassThru

$SW_MAXIMIZE = 3
$SW_RESTORE = 9

function Set-Foreground([IntPtr]$hwnd) {
    $foreground = [Drive.Win]::GetForegroundWindow()
    $processId = 0
    $owner = [Drive.Win]::GetWindowThreadProcessId($foreground, [ref]$processId)
    $me = [Drive.Win]::GetCurrentThreadId()
    [Drive.Win]::AttachThreadInput($me, $owner, $true) | Out-Null
    # Only a minimized window is restored. Restoring any other window would
    # shrink it, which is how this script used to uncover the desktop.
    if ([Drive.Win]::IsIconic($hwnd)) { [Drive.Win]::ShowWindow($hwnd, $SW_RESTORE) | Out-Null }
    [Drive.Win]::BringWindowToTop($hwnd) | Out-Null
    $ok = [Drive.Win]::SetForegroundWindow($hwnd)
    [Drive.Win]::SetActiveWindow($hwnd) | Out-Null
    [Drive.Win]::AttachThreadInput($me, $owner, $false) | Out-Null
    return $ok
}

function Get-MainWindow([string]$name) {
    $process = Get-Process $name -ErrorAction SilentlyContinue |
        Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
    if (-not $process) { throw "no window for $name" }
    return $process.MainWindowHandle
}

# The flags of the left button. The middle button is 0x0020 and 0x0040, the
# right button 0x0008 and 0x0010, and an X button 0x0080 and 0x0100 with the
# button number as data.
function Click-InWindow([IntPtr]$hwnd, [int]$x, [int]$y, [int]$count = 1, [uint32]$down = 0x0002, [uint32]$up = 0x0004, [uint32]$data = 0) {
    $rect = New-Object Drive.Win+RECT
    [Drive.Win]::GetWindowRect($hwnd, [ref]$rect) | Out-Null
    [Drive.Win]::SetCursorPos($rect.Left + $x, $rect.Top + $y) | Out-Null
    Start-Sleep -Milliseconds 120
    for ($i = 0; $i -lt $count; $i++) {
        [Drive.Win]::mouse_event($down, 0, 0, $data, [UIntPtr]::Zero)
        [Drive.Win]::mouse_event($up, 0, 0, $data, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 60
    }
}

function Move-InWindow([IntPtr]$hwnd, [int]$x, [int]$y) {
    $rect = New-Object Drive.Win+RECT
    [Drive.Win]::GetWindowRect($hwnd, [ref]$rect) | Out-Null
    $from = [System.Windows.Forms.Cursor]::Position
    $toX = $rect.Left + $x
    $toY = $rect.Top + $y
    for ($i = 1; $i -le 20; $i++) {
        $atX = [int]($from.X + ($toX - $from.X) * $i / 20)
        $atY = [int]($from.Y + ($toY - $from.Y) * $i / 20)
        # SetCursorPos alone sends no move while a button is down, so the
        # move is sent as input.
        [Drive.Win]::SetCursorPos($atX, $atY) | Out-Null
        [Drive.Win]::mouse_event(0x0001, 0, 0, 0, [UIntPtr]::Zero)
        Start-Sleep -Milliseconds 25
    }
}

function Scroll-InWindow([IntPtr]$hwnd, [int]$x, [int]$y, [int]$notches) {
    $rect = New-Object Drive.Win+RECT
    [Drive.Win]::GetWindowRect($hwnd, [ref]$rect) | Out-Null
    [Drive.Win]::SetCursorPos($rect.Left + $x, $rect.Top + $y) | Out-Null
    Start-Sleep -Milliseconds 120
    # MOUSEEVENTF_WHEEL. One notch is WHEEL_DELTA, which is 120. The parameter
    # is unsigned, so a turn towards the hand goes through as two's complement.
    $delta = [BitConverter]::ToUInt32([BitConverter]::GetBytes([int32]($notches * 120)), 0)
    [Drive.Win]::mouse_event(0x0800, 0, 0, $delta, [UIntPtr]::Zero)
}

function Get-CursorAt([IntPtr]$hwnd, [int]$x, [int]$y) {
    $rect = New-Object Drive.Win+RECT
    [Drive.Win]::GetWindowRect($hwnd, [ref]$rect) | Out-Null
    # GPUI picks the cursor in the frame after a move, and Windows asks for it
    # on the next move, so the pointer moves a second time by one pixel.
    [Drive.Win]::SetCursorPos($rect.Left + $x, $rect.Top + $y) | Out-Null
    Start-Sleep -Milliseconds 200
    [Drive.Win]::SetCursorPos($rect.Left + $x + 1, $rect.Top + $y) | Out-Null
    Start-Sleep -Milliseconds 200
    $info = New-Object Drive.Win+CURSORINFO
    $info.Size = [System.Runtime.InteropServices.Marshal]::SizeOf($info)
    [Drive.Win]::GetCursorInfo([ref]$info) | Out-Null
    # IDC_HAND, IDC_ARROW and IDC_SIZENS. The system shares one handle for each.
    switch ($info.Cursor) {
        ([Drive.Win]::LoadCursor([IntPtr]::Zero, [IntPtr]32649)) { return 'hand' }
        ([Drive.Win]::LoadCursor([IntPtr]::Zero, [IntPtr]32512)) { return 'arrow' }
        ([Drive.Win]::LoadCursor([IntPtr]::Zero, [IntPtr]32645)) { return 'updown' }
        default { return 'other' }
    }
}

function Save-WindowPicture([IntPtr]$hwnd, [string]$path) {
    $rect = New-Object Drive.Win+RECT
    [Drive.Win]::GetWindowRect($hwnd, [ref]$rect) | Out-Null
    $bitmap = New-Object System.Drawing.Bitmap ($rect.Right - $rect.Left), ($rect.Bottom - $rect.Top)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $origin = New-Object System.Drawing.Point $rect.Left, $rect.Top
    $graphics.CopyFromScreen($origin, [System.Drawing.Point]::Empty, $bitmap.Size)
    $bitmap.Save($path)
    $graphics.Dispose()
    $bitmap.Dispose()
}

$terminal = Get-MainWindow 'WindowsTerminal'
$terminalWasMaximized = [Drive.Win]::IsZoomed($terminal)
$application = Get-Process book-buddy -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowTitle -ceq $Title } | Select-Object -First 1 -ExpandProperty MainWindowHandle
if (-not $application) { throw "no window titled '$Title'" }
if (-not (Test-Path $OutDir)) { New-Item -ItemType Directory -Path $OutDir | Out-Null }

try {
    # Full screen, so nothing of the desktop is in the picture or under the
    # application window.
    Set-Foreground $terminal | Out-Null
    if (-not $terminalWasMaximized) { [Drive.Win]::ShowWindow($terminal, $SW_MAXIMIZE) | Out-Null }
    Start-Sleep -Milliseconds 300

    if (-not (Set-Foreground $application)) { throw "$Title did not come to the front" }
    Start-Sleep -Milliseconds $SettleMs

    foreach ($step in $Steps) {
        $kind, $value = $step -split ':', 2
        switch ($kind) {
            'keys' {
                [System.Windows.Forms.SendKeys]::SendWait($value)
                Start-Sleep -Milliseconds $SettleMs
            }
            'kdown' {
                [Drive.Win]::keybd_event([byte]$value, 0, 0, [UIntPtr]::Zero)
                Start-Sleep -Milliseconds 30
            }
            'kup' {
                # KEYEVENTF_KEYUP.
                [Drive.Win]::keybd_event([byte]$value, 0, 2, [UIntPtr]::Zero)
                Start-Sleep -Milliseconds $SettleMs
            }
            'shot' {
                $path = Join-Path $OutDir "$value.png"
                Save-WindowPicture $application $path
                Write-Output $path
            }
            'click' {
                $x, $y = $value -split ',', 2
                Click-InWindow $application ([int]$x) ([int]$y)
                Start-Sleep -Milliseconds $SettleMs
            }
            'dblclick' {
                $x, $y = $value -split ',', 2
                Click-InWindow $application ([int]$x) ([int]$y) 2
                Start-Sleep -Milliseconds $SettleMs
            }
            'mclick' {
                $x, $y = $value -split ',', 2
                Click-InWindow $application ([int]$x) ([int]$y) 1 0x0020 0x0040
                Start-Sleep -Milliseconds $SettleMs
            }
            'rclick' {
                $x, $y = $value -split ',', 2
                Click-InWindow $application ([int]$x) ([int]$y) 1 0x0008 0x0010
                Start-Sleep -Milliseconds $SettleMs
            }
            'bclick' {
                $x, $y = $value -split ',', 2
                Click-InWindow $application ([int]$x) ([int]$y) 1 0x0080 0x0100 1
                Start-Sleep -Milliseconds $SettleMs
            }
            { $_ -in 'down', 'mdown' } {
                $flag = if ($kind -eq 'mdown') { 0x0020 } else { 0x0002 }
                $x, $y = $value -split ',', 2
                $rect = New-Object Drive.Win+RECT
                [Drive.Win]::GetWindowRect($application, [ref]$rect) | Out-Null
                [Drive.Win]::SetCursorPos($rect.Left + [int]$x, $rect.Top + [int]$y) | Out-Null
                Start-Sleep -Milliseconds 120
                [Drive.Win]::mouse_event($flag, 0, 0, 0, [UIntPtr]::Zero)
                Start-Sleep -Milliseconds 120
            }
            'move' {
                $x, $y = $value -split ',', 2
                Move-InWindow $application ([int]$x) ([int]$y)
            }
            { $_ -in 'up', 'mup' } {
                $flag = if ($kind -eq 'mup') { 0x0040 } else { 0x0004 }
                [Drive.Win]::mouse_event($flag, 0, 0, 0, [UIntPtr]::Zero)
                Start-Sleep -Milliseconds $SettleMs
            }
            'scroll' {
                $x, $y, $notches = $value -split ',', 3
                Scroll-InWindow $application ([int]$x) ([int]$y) ([int]$notches)
                Start-Sleep -Milliseconds $SettleMs
            }
            'hover' {
                $x, $y = $value -split ',', 2
                Write-Output "hover ${x},${y}: $(Get-CursorAt $application ([int]$x) ([int]$y))"
            }
            'wait' { Start-Sleep -Milliseconds ([int]$value) }
            default { throw "unknown step $step" }
        }
    }
}
finally {
    if (-not $terminalWasMaximized) { [Drive.Win]::ShowWindow($terminal, $SW_RESTORE) | Out-Null }
}
