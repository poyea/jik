# Copyright (c) 2026 John Law
# SPDX-License-Identifier: MIT

# The popup: Skip closes it at once, and auto-skip closes it when the countdown ends.

. (Join-Path $PSScriptRoot 'common.ps1')

function Test-SkipIsRecorded {
    $dir = New-JikSandbox
    $jik = $null
    try {
        $jik = Start-Jik $dir @('--once')
        $popup = Wait-JikWindow $jik 'WinkitChoicePopup'
        Start-Sleep -Milliseconds 500

        # Skip is the small button in the bottom-right corner of the popup.
        $size = Get-JikClientSize $popup
        Send-JikClick $popup ($size.Width - 40) ($size.Height - 35)

        Assert-Equal 0 (Wait-JikExit $jik 5000) 'exit code'
        $answers = Get-JikAnswers $dir
        Assert-Equal 1 $answers.Count 'answer records'
        Assert-Equal 'skipped' $answers[0].outcome 'outcome'
        Assert-Equal 'capital-france' $answers[0].question_id 'question asked first'
        Assert-True ($null -eq $answers[0].choice) 'a skip has no choice'
        Assert-Equal $answers[0].from_box $answers[0].to_box 'box after a skip'
    } finally {
        Remove-JikSandbox $dir $jik
    }
}

function Test-AutoSkipTimesOut {
    $dir = New-JikSandbox
    $jik = $null
    try {
        # A short countdown, so the test does not wait 30 seconds.
        Set-Content -Path (Join-Path $dir 'config.toml') -Value "[popup]`r`nauto_skip_seconds = 2"
        $jik = Start-Jik $dir @('--once')
        Wait-JikWindow $jik 'WinkitChoicePopup' | Out-Null
        $clock = [Diagnostics.Stopwatch]::StartNew()

        Assert-Equal 0 (Wait-JikExit $jik 10000) 'exit code'
        Assert-True ($clock.ElapsedMilliseconds -ge 1500) "closed after $($clock.ElapsedMilliseconds) ms, before the countdown ended"
        $answers = Get-JikAnswers $dir
        Assert-Equal 1 $answers.Count 'answer records'
        Assert-Equal 'timed_out' $answers[0].outcome 'outcome'
    } finally {
        Remove-JikSandbox $dir $jik
    }
}

Test-SkipIsRecorded
Write-Host 'PASS popup: Skip is recorded'
Test-AutoSkipTimesOut
Write-Host 'PASS popup: auto-skip times out'
