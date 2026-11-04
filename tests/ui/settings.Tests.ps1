# Copyright (c) 2026 John Law
# SPDX-License-Identifier: MIT

# The settings window: add a question and save it, and place the popup with the picker.

. (Join-Path $PSScriptRoot 'common.ps1')

function Test-AddQuestion {
    $dir = New-JikSandbox
    $jik = $null
    try {
        $jik = Start-Jik $dir @('--settings')
        $dialog = Wait-JikWindow $jik 'TraykitDialog'
        Start-Sleep -Milliseconds 300
        Save-JikScreenshot 'settings-1-open' $dialog

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
        Start-Sleep -Milliseconds 200
        Save-JikScreenshot 'settings-2-new-question' $dialog
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

function Test-PlacePopupWithThePicker {
    $dir = New-JikSandbox
    $jik = $null
    try {
        $jik = Start-Jik $dir @('--settings')
        $dialog = Wait-JikWindow $jik 'TraykitDialog'
        Start-Sleep -Milliseconds 300

        # A click on an empty place in the picture brings the center of the popup there.
        # The popup starts at the bottom right, so the top left of the picture is empty.
        $picker = (Get-JikChildren $dialog 'TraykitPlacement')[0]
        $size = Get-JikClientSize $picker
        $x = [int]($size.Width * 0.3)
        $y = [int]($size.Height * 0.3)
        $position = [IntPtr](($y -shl 16) -bor $x)
        [JikUi.Native]::SendMessageW($picker, $WM_LBUTTONDOWN, [IntPtr]1, $position) | Out-Null
        [JikUi.Native]::SendMessageW($picker, $WM_LBUTTONUP, [IntPtr]0, $position) | Out-Null
        Start-Sleep -Milliseconds 200
        Save-JikScreenshot 'settings-3-popup-placed' $dialog
        [JikUi.Native]::SendMessageW((Get-JikButton $dialog 'Save'), $BM_CLICK, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
        Assert-Equal 0 (Wait-JikExit $jik 5000) 'exit code of the settings window'

        $config = Get-Content -Raw (Join-Path $dir 'config.toml')
        # Only a line that starts with the key: the comment above it has an example position.
        $saved = [regex]::Match($config, '(?m)^position = \{ x = ([0-9.]+), y = ([0-9.]+) \}')
        Assert-True $saved.Success 'a custom position is saved'
        $fx = [double]::Parse($saved.Groups[1].Value, [Globalization.CultureInfo]::InvariantCulture)
        $fy = [double]::Parse($saved.Groups[2].Value, [Globalization.CultureInfo]::InvariantCulture)
        Assert-True ($fx -gt 0 -and $fx -lt 0.5 -and $fy -gt 0 -and $fy -lt 0.5) "position ($fx, $fy) is in the top left"

        # The popup shows at the same place on the real screen.
        $jik = Start-Jik $dir @('--once')
        $popup = Wait-JikWindow $jik 'TraykitChoicePopup'
        Start-Sleep -Milliseconds 500
        Save-JikScreenshot 'settings-4-popup-at-the-place'
        $rect = Get-JikWindowRect $popup
        $area = Get-JikWorkArea
        $margin = [Math]::Round(12 * [JikUi.Native]::GetDpiForWindow($popup) / 96)
        $width = $rect.Right - $rect.Left
        $height = $rect.Bottom - $rect.Top
        $expectedX = $area.Left + $margin + $fx * ($area.Width - 2 * $margin - $width)
        $expectedY = $area.Top + $margin + $fy * ($area.Height - 2 * $margin - $height)
        Assert-True ([Math]::Abs($rect.Left - $expectedX) -le 2) "popup left $($rect.Left), expected $expectedX"
        Assert-True ([Math]::Abs($rect.Top - $expectedY) -le 2) "popup top $($rect.Top), expected $expectedY"

        $client = Get-JikClientSize $popup
        Send-JikClick $popup ($client.Width - 40) ($client.Height - 35)
        Assert-Equal 0 (Wait-JikExit $jik 5000) 'exit code of the popup'
    } finally {
        Remove-JikSandbox $dir $jik
    }
}

Test-AddQuestion
Write-Host 'PASS settings: a new question is saved'
Test-PlacePopupWithThePicker
Write-Host 'PASS settings: the picker places the popup'
