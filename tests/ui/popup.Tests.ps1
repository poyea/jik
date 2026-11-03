# Copyright (c) 2026 John Law
# SPDX-License-Identifier: MIT

# The popup: Skip closes it at once, an answer shows the result, auto-skip closes it when the
# countdown ends, and a due question waits while a full-screen window is in front.

. (Join-Path $PSScriptRoot 'common.ps1')

function Test-SkipIsRecorded {
    $dir = New-JikSandbox
    $jik = $null
    try {
        $jik = Start-Jik $dir @('--once')
        $popup = Wait-JikWindow $jik 'WinkitChoicePopup'
        Start-Sleep -Milliseconds 500
        Save-JikScreenshot 'popup-1-question' $popup -FromScreen

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

function Test-AnswerShowsTheResult {
    $dir = New-JikSandbox
    $jik = $null
    try {
        $jik = Start-Jik $dir @('--once')
        $popup = Wait-JikWindow $jik 'WinkitChoicePopup'
        Start-Sleep -Milliseconds 500

        # The first question of the sample is on one line, so the first choice (Berlin) starts
        # after the padding (18), the line (about 21), and the gap (14), and is 36 high.
        $scale = [JikUi.Native]::GetDpiForWindow($popup) / 96
        $client = Get-JikClientSize $popup
        Send-JikClick $popup ([int]($client.Width / 2)) ([int]((18 + 21 + 14 + 18) * $scale))
        Start-Sleep -Milliseconds 500
        Save-JikScreenshot 'popup-2-wrong-answer' $popup -FromScreen

        Assert-Equal 0 (Wait-JikExit $jik 5000) 'exit code'
        $answers = Get-JikAnswers $dir
        Assert-Equal 1 $answers.Count 'answer records'
        Assert-Equal 'answered' $answers[0].outcome 'outcome'
        Assert-Equal 'Berlin' $answers[0].choice 'choice'
        Assert-Equal $false $answers[0].correct 'correct'
        Assert-Equal 1 $answers[0].to_box 'box after a wrong answer'
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
        $popup = Wait-JikWindow $jik 'WinkitChoicePopup'
        $clock = [Diagnostics.Stopwatch]::StartNew()
        Start-Sleep -Milliseconds 500
        Save-JikScreenshot 'popup-3-countdown' $popup -FromScreen

        Assert-Equal 0 (Wait-JikExit $jik 10000) 'exit code'
        Assert-True ($clock.ElapsedMilliseconds -ge 1500) "closed after $($clock.ElapsedMilliseconds) ms, before the countdown ended"
        $answers = Get-JikAnswers $dir
        Assert-Equal 1 $answers.Count 'answer records'
        Assert-Equal 'timed_out' $answers[0].outcome 'outcome'
    } finally {
        Remove-JikSandbox $dir $jik
    }
}

function Test-WaitsWhileFullScreen {
    Add-Type -AssemblyName System.Windows.Forms
    $dir = New-JikSandbox
    $jik = $null
    $screen = $null
    $oldScale = $env:JIK_TIME_SCALE
    try {
        # A window that covers the screen, as a game or a full-screen video does. It tells
        # the person at the screen what it is, so a dark screen does not look like a fault.
        $screen = New-Object System.Windows.Forms.Form
        $screen.FormBorderStyle = 'None'
        $screen.WindowState = 'Maximized'
        $screen.TopMost = $true
        $screen.BackColor = [System.Drawing.Color]::FromArgb(32, 32, 32)
        $note = New-Object System.Windows.Forms.Label
        $note.Text = 'jik UI test: this full-screen window closes by itself in a few seconds.'
        $note.ForeColor = [System.Drawing.Color]::White
        $note.Font = New-Object System.Drawing.Font('Segoe UI', 20)
        $note.Dock = 'Fill'
        $note.TextAlign = 'MiddleCenter'
        $screen.Controls.Add($note)
        $screen.Show()
        $screen.Activate()
        [System.Windows.Forms.Application]::DoEvents()

        # 60 times faster: the first question is due after 2 seconds, and a held one is
        # checked again each second.
        $env:JIK_TIME_SCALE = '60'
        $jik = Start-Jik $dir @()
        $clock = [Diagnostics.Stopwatch]::StartNew()
        while ($clock.ElapsedMilliseconds -lt 6000) {
            [System.Windows.Forms.Application]::DoEvents()
            $popup = Find-JikWindow $jik 'WinkitChoicePopup'
            Assert-True ($popup -eq [IntPtr]::Zero) "no popup over the full-screen window ($($clock.ElapsedMilliseconds) ms)"
            Assert-True (-not $jik.HasExited) 'jik keeps running'
            Start-Sleep -Milliseconds 100
        }

        Save-JikScreenshot 'popup-4-held-while-full-screen'
        $screen.Close()
        $screen = $null
        $popup = Wait-JikWindow $jik 'WinkitChoicePopup' 10000
        Start-Sleep -Milliseconds 500
        Save-JikScreenshot 'popup-5-shown-after-full-screen'
        $client = Get-JikClientSize $popup
        Send-JikClick $popup ($client.Width - 40) ($client.Height - 35)

        $clock.Restart()
        while ((Get-JikAnswers $dir).Count -eq 0 -and $clock.ElapsedMilliseconds -lt 5000) {
            Start-Sleep -Milliseconds 100
        }
        $answers = Get-JikAnswers $dir
        Assert-Equal 1 $answers.Count 'answer records'
        Assert-Equal 'skipped' $answers[0].outcome 'outcome'
    } finally {
        if ($screen) { $screen.Close() }
        $env:JIK_TIME_SCALE = $oldScale
        Remove-JikSandbox $dir $jik
    }
}

Test-SkipIsRecorded
Write-Host 'PASS popup: Skip is recorded'
Test-AnswerShowsTheResult
Write-Host 'PASS popup: a wrong answer is recorded and shown'
Test-AutoSkipTimesOut
Write-Host 'PASS popup: auto-skip times out'
# This test covers the whole screen for some seconds. It runs in CI, where no person uses the
# screen, and on a local computer only when JIK_UI_FULLSCREEN is 1.
if ($env:CI -eq 'true' -or $env:JIK_UI_FULLSCREEN -eq '1') {
    Test-WaitsWhileFullScreen
    Write-Host 'PASS popup: a due question waits while a full-screen window is in front'
} else {
    Write-Host 'SKIP popup: full-screen test (it covers the screen; set JIK_UI_FULLSCREEN=1 to run it)'
}
