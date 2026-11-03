# Copyright (c) 2026 John Law
# SPDX-License-Identifier: MIT

# Build jik and run all UI tests (*.Tests.ps1). The tests show real windows, so run them
# on an unlocked desktop, and quit jik first. Do not use the mouse while they run: a click
# answers a popup, and the pointer on a popup stops its countdown.
# One test covers the whole screen with a dark window for about 6 seconds, as a full-screen
# game does. It runs only in CI, or when JIK_UI_FULLSCREEN is 1.
# Set JIK_UI_SCREENSHOTS to a folder to keep PNG pictures of the windows (CI does this).
#
#   powershell -ExecutionPolicy Bypass -File tests\ui\run.ps1

$ErrorActionPreference = 'Stop'
Push-Location (Join-Path $PSScriptRoot '..\..')
try {
    cargo build
    if ($LASTEXITCODE -ne 0) { throw 'cargo build failed.' }
} finally {
    Pop-Location
}

. (Join-Path $PSScriptRoot 'common.ps1')

$failed = 0
foreach ($test in Get-ChildItem (Join-Path $PSScriptRoot '*.Tests.ps1')) {
    try {
        & $test.FullName
    } catch {
        $failed++
        Write-Host "FAIL $($test.Name): $($_.Exception.Message)" -ForegroundColor Red
        if ($env:GITHUB_ACTIONS -eq 'true') {
            # An annotation shows the reason on the run page, also without the full log.
            Write-Host "::error title=UI test failed: $($test.Name)::$($_.Exception.Message)"
        }
        # What was on the screen when the test failed, if JIK_UI_SCREENSHOTS is set.
        Save-JikScreenshot "FAIL-$($test.BaseName)"
    }
}
if ($failed -gt 0) {
    Write-Host "$failed test file(s) failed." -ForegroundColor Red
    exit 1
}
Write-Host 'All UI tests passed.' -ForegroundColor Green
exit 0
