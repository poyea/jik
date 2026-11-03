# Copyright (c) 2026 John Law
# SPDX-License-Identifier: MIT

# Helpers for the UI tests. Each test runs a copy of jik.exe in a new empty folder,
# so the tests do not change your questions, settings, or answers.

$ErrorActionPreference = 'Stop'

if (-not ('JikUi.Native' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
namespace JikUi {
    public static class Native {
        [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        public static extern IntPtr FindWindowW(string cls, IntPtr name);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        public static extern IntPtr FindWindowExW(IntPtr parent, IntPtr after, string cls, string name);
        // PowerShell gives an empty string for $null. A null name must be a null pointer.
        [DllImport("user32.dll", CharSet = CharSet.Unicode, EntryPoint = "FindWindowExW")]
        public static extern IntPtr FindWindowOfClass(IntPtr parent, IntPtr after, string cls, IntPtr name);
        [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
        [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
        [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
        [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
        [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
        [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
        [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr h);
        [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
        [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
        [DllImport("shell32.dll")] public static extern int SHQueryUserNotificationState(out int state);
        [DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr h);
        [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
        [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr h, uint m, IntPtr w, IntPtr l);
        [DllImport("user32.dll", CharSet = CharSet.Unicode)]
        public static extern IntPtr SendMessageW(IntPtr h, uint m, IntPtr w, string l);
        [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    }
}
'@
}
# Client coordinates in messages are physical pixels, so this process must not be scaled.
[JikUi.Native]::SetProcessDPIAware() | Out-Null

$script:WM_CLOSE = 0x0010
$script:WM_COMMAND = 0x0111
$script:WM_LBUTTONDOWN = 0x0201
$script:WM_LBUTTONUP = 0x0202
$script:EM_REPLACESEL = 0x00C2
$script:BM_CLICK = 0x00F5
$script:CB_SETCURSEL = 0x014E
$script:CBN_SELCHANGE = 1

function Get-JikExe {
    $exe = Join-Path $PSScriptRoot '..\..\target\debug\jik.exe'
    if (-not (Test-Path $exe)) {
        throw "jik.exe not found at $exe. Run 'cargo build' first."
    }
    (Resolve-Path $exe).Path
}

# Make a new empty folder with a copy of jik.exe. jik creates its files there on first run.
function New-JikSandbox {
    if (Get-Process jik -ErrorAction SilentlyContinue) {
        throw 'jik is running. Quit it from its tray menu: only one jik can run at a time.'
    }
    $dir = Join-Path ([IO.Path]::GetTempPath()) ("jik-ui-" + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $dir | Out-Null
    Copy-Item (Get-JikExe) (Join-Path $dir 'jik.exe')
    $dir
}

function Remove-JikSandbox([string] $Dir, $Process) {
    if ($Process -and -not $Process.HasExited) {
        Stop-Process -Id $Process.Id -Force -ErrorAction SilentlyContinue
        $Process.WaitForExit(5000) | Out-Null
    }
    Remove-Item -Recurse -Force $Dir -ErrorAction SilentlyContinue
}

function Start-Jik([string] $Dir, [string[]] $Arguments) {
    $params = @{ FilePath = (Join-Path $Dir 'jik.exe'); WorkingDirectory = $Dir; PassThru = $true }
    if ($Arguments) { $params.ArgumentList = $Arguments }
    Start-Process @params
}

# A top-level window of the process with the given window class, or zero if there is none.
function Find-JikWindow($Process, [string] $Class) {
    $hwnd = [IntPtr]::Zero
    while ($true) {
        $hwnd = [JikUi.Native]::FindWindowOfClass([IntPtr]::Zero, $hwnd, $Class, [IntPtr]::Zero)
        if ($hwnd -eq [IntPtr]::Zero) { return $hwnd }
        $owner = [uint32]0
        [JikUi.Native]::GetWindowThreadProcessId($hwnd, [ref] $owner) | Out-Null
        if ($owner -eq $Process.Id) { return $hwnd }
    }
}

# Wait for a top-level window of the process with the given window class.
function Wait-JikWindow($Process, [string] $Class, [int] $TimeoutMs = 10000) {
    $clock = [Diagnostics.Stopwatch]::StartNew()
    while ($clock.ElapsedMilliseconds -lt $TimeoutMs) {
        $hwnd = Find-JikWindow $Process $Class
        if ($hwnd -ne [IntPtr]::Zero) { return $hwnd }
        if ($Process.HasExited) { throw "jik exited with code $($Process.ExitCode) before it showed $Class." }
        Start-Sleep -Milliseconds 100
    }
    throw "No $Class window after $TimeoutMs ms."
}

function Wait-JikExit($Process, [int] $TimeoutMs = 10000) {
    if (-not $Process.WaitForExit($TimeoutMs)) {
        throw "jik did not exit after $TimeoutMs ms."
    }
    $Process.ExitCode
}

# The child windows of a class, in the order the app made them.
function Get-JikChildren([IntPtr] $Parent, [string] $Class) {
    $found = @()
    $child = [IntPtr]::Zero
    while ($true) {
        $child = [JikUi.Native]::FindWindowOfClass($Parent, $child, $Class, [IntPtr]::Zero)
        if ($child -eq [IntPtr]::Zero) { break }
        $found += $child
    }
    , $found
}

function Get-JikButton([IntPtr] $Parent, [string] $Text) {
    $button = [JikUi.Native]::FindWindowExW($Parent, [IntPtr]::Zero, 'Button', $Text)
    if ($button -eq [IntPtr]::Zero) { throw "No button '$Text'." }
    $button
}

function Get-JikClientSize([IntPtr] $Hwnd) {
    $rect = New-Object JikUi.Native+RECT
    [JikUi.Native]::GetClientRect($Hwnd, [ref] $rect) | Out-Null
    @{ Width = $rect.Right; Height = $rect.Bottom }
}

function Get-JikWindowRect([IntPtr] $Hwnd) {
    $rect = New-Object JikUi.Native+RECT
    [JikUi.Native]::GetWindowRect($Hwnd, [ref] $rect) | Out-Null
    $rect
}

# The work area of the primary monitor, in pixels.
function Get-JikWorkArea {
    Add-Type -AssemblyName System.Windows.Forms
    [System.Windows.Forms.Screen]::PrimaryScreen.WorkingArea
}

# Make a window the foreground window. Windows refuses this to a process that does not own the
# foreground (as on a CI runner), but not just after a key press, so press and release Alt.
function Set-JikForeground([IntPtr] $Hwnd) {
    $alt = 0x12
    $keyUp = 0x2
    [JikUi.Native]::keybd_event($alt, 0, 0, [UIntPtr]::Zero)
    [JikUi.Native]::SetForegroundWindow($Hwnd) | Out-Null
    [JikUi.Native]::keybd_event($alt, 0, $keyUp, [UIntPtr]::Zero)
    [JikUi.Native]::BringWindowToTop($Hwnd) | Out-Null
}

# True while Windows holds its own notifications for a full-screen app (QUNS_BUSY, or
# QUNS_RUNNING_D3D_FULL_SCREEN).
function Test-JikFullScreenBusy {
    $state = 0
    [JikUi.Native]::SHQueryUserNotificationState([ref] $state) | Out-Null
    $state -eq 2 -or $state -eq 3
}

# Save a PNG picture when JIK_UI_SCREENSHOTS names a folder (CI sets it). Without -Hwnd, the
# picture is the whole screen. -FromScreen copies the window from the screen instead of asking
# the window to draw itself: use it for the popup, because Windows composes its glass.
# A problem with a picture gives a warning only: it must not make a test fail.
function Save-JikScreenshot([string] $Name, [IntPtr] $Hwnd = [IntPtr]::Zero, [switch] $FromScreen) {
    $folder = $env:JIK_UI_SCREENSHOTS
    if (-not $folder) { return }
    try {
        Add-Type -AssemblyName System.Drawing
        Add-Type -AssemblyName System.Windows.Forms
        New-Item -ItemType Directory -Force -Path $folder | Out-Null
        if ($Hwnd -eq [IntPtr]::Zero) {
            $area = [System.Windows.Forms.SystemInformation]::VirtualScreen
            $left, $top, $width, $height = $area.Left, $area.Top, $area.Width, $area.Height
        } else {
            $rect = Get-JikWindowRect $Hwnd
            $left, $top = $rect.Left, $rect.Top
            $width, $height = ($rect.Right - $rect.Left), ($rect.Bottom - $rect.Top)
        }
        $picture = New-Object System.Drawing.Bitmap $width, $height
        $graphics = [System.Drawing.Graphics]::FromImage($picture)
        try {
            if ($Hwnd -eq [IntPtr]::Zero -or $FromScreen) {
                $graphics.CopyFromScreen($left, $top, 0, 0, (New-Object System.Drawing.Size $width, $height))
            } else {
                $hdc = $graphics.GetHdc()
                # PW_RENDERFULLCONTENT: also windows that another window covers.
                [JikUi.Native]::PrintWindow($Hwnd, $hdc, 2) | Out-Null
                $graphics.ReleaseHdc($hdc)
            }
            $picture.Save((Join-Path $folder "$Name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
        } finally {
            $graphics.Dispose()
            $picture.Dispose()
        }
    } catch {
        Write-Warning "No screenshot '$Name': $($_.Exception.Message)"
    }
}

# A left click at client coordinates, as the mouse sends it.
function Send-JikClick([IntPtr] $Hwnd, [int] $X, [int] $Y) {
    $position = [IntPtr](($Y -shl 16) -bor ($X -band 0xFFFF))
    [JikUi.Native]::PostMessageW($Hwnd, $script:WM_LBUTTONDOWN, [IntPtr]1, $position) | Out-Null
    [JikUi.Native]::PostMessageW($Hwnd, $script:WM_LBUTTONUP, [IntPtr]0, $position) | Out-Null
}

# Type text into an edit box, as the keyboard does (the edit box tells the app that it changed).
function Send-JikText([IntPtr] $Edit, [string] $Text) {
    [JikUi.Native]::SendMessageW($Edit, $script:EM_REPLACESEL, [IntPtr]1, $Text) | Out-Null
}

# Select an item of a drop-down, as a click in its list does.
function Select-JikItem([IntPtr] $Dialog, [IntPtr] $DropDown, [int] $Index) {
    [JikUi.Native]::SendMessageW($DropDown, $script:CB_SETCURSEL, [IntPtr]$Index, [IntPtr]::Zero) | Out-Null
    $id = [JikUi.Native]::GetDlgCtrlID($DropDown)
    $command = [IntPtr](($script:CBN_SELCHANGE -shl 16) -bor $id)
    [JikUi.Native]::SendMessageW($Dialog, $script:WM_COMMAND, $command, $DropDown) | Out-Null
}

function Get-JikAnswers([string] $Dir) {
    $path = Join-Path $Dir 'answers.jsonl'
    if (-not (Test-Path $path)) { return @() }
    , @(Get-Content $path | Where-Object { $_ } | ForEach-Object { $_ | ConvertFrom-Json })
}

function Assert-Equal($Expected, $Actual, [string] $What) {
    if ($Expected -ne $Actual) {
        throw "$What`: expected '$Expected', got '$Actual'."
    }
}

function Assert-True([bool] $Condition, [string] $What) {
    if (-not $Condition) { throw "$What`: not true." }
}
