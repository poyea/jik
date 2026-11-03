# Copyright (c) 2026 John Law
# SPDX-License-Identifier: MIT

# The settings window: add a question, save, and find it in questions.toml.

. (Join-Path $PSScriptRoot 'common.ps1')

function Test-AddQuestion {
    $dir = New-JikSandbox
    $jik = $null
    try {
        $jik = Start-Jik $dir @('--settings')
        $dialog = Wait-JikWindow $jik 'WinkitDialog'
        Start-Sleep -Milliseconds 300

        # The edit boxes in order: auto-skip, question, choices.
        # The drop-downs in order: popup position, correct answer.
        $edits = Get-JikChildren $dialog 'Edit'
        $dropDowns = Get-JikChildren $dialog 'ComboBox'
        Assert-Equal 3 $edits.Count 'edit boxes'
        Assert-Equal 2 $dropDowns.Count 'drop-downs'

        [JikUi.Native]::SendMessageW((Get-JikButton $dialog 'Add'), $BM_CLICK, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
        Send-JikText $edits[1] 'What is 2 + 2?'
        Send-JikText $edits[2] "3`r`n4`r`n5"
        Select-JikItem $dialog $dropDowns[1] 1
        [JikUi.Native]::SendMessageW((Get-JikButton $dialog 'Save'), $BM_CLICK, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null

        Assert-Equal 0 (Wait-JikExit $jik 5000) 'exit code'
        $questions = Get-Content -Raw (Join-Path $dir 'questions.toml')
        Assert-Equal 5 ([regex]::Matches($questions, '(?m)^\[\[question\]\]').Count) 'questions in the file'
        $added = 'id = "what-is-2-2"\s+text = "What is 2 \+ 2\?"\s+choices = \["3", "4", "5"\]\s+answer = "4"'
        Assert-True ($questions -match $added) 'the new question is in questions.toml'

        $config = Get-Content -Raw (Join-Path $dir 'config.toml')
        Assert-True ($config -match '# jik configuration\.') 'the comments in config.toml stay'
        Assert-True ($config -match 'auto_skip_seconds = 30') 'auto-skip is saved'
    } finally {
        Remove-JikSandbox $dir $jik
    }
}

Test-AddQuestion
Write-Host 'PASS settings: a new question is saved'
