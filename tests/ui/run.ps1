# Copyright (c) 2026 John Law
# SPDX-License-Identifier: MIT

# Build jik and run all UI tests (*.Tests.ps1). The tests show real windows, so run them
# on an unlocked desktop, and quit jik first. Do not use the mouse while they run: a click
# answers a popup, and the pointer on a popup stops its countdown.
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

$failed = 0
foreach ($test in Get-ChildItem (Join-Path $PSScriptRoot '*.Tests.ps1')) {
    try {
        & $test.FullName
    } catch {
        $failed++
        Write-Host "FAIL $($test.Name): $($_.Exception.Message)" -ForegroundColor Red
    }
}
if ($failed -gt 0) {
    Write-Host "$failed test file(s) failed." -ForegroundColor Red
    exit 1
}
Write-Host 'All UI tests passed.' -ForegroundColor Green
