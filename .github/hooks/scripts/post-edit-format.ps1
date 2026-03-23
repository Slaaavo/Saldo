# Post-edit formatting hook
# Runs prettier + eslint for frontend files; cargo fmt + clippy for Rust files.
# Captures any warnings/errors and emits them as additionalContext so the agent can fix them.

$raw = [Console]::In.ReadToEnd()
if (-not $raw) { exit 0 }

try {
    $data = $raw | ConvertFrom-Json
} catch {
    exit 0
}

$toolName = $data.tool_name
$filePaths = @()

switch ($toolName) {
    { $_ -in @("replace_string_in_file", "create_file") } {
        if ($data.tool_input.filePath) {
            $filePaths += $data.tool_input.filePath
        }
    }
    "multi_replace_string_in_file" {
        if ($data.tool_input.replacements) {
            foreach ($r in $data.tool_input.replacements) {
                if ($r.filePath) { $filePaths += $r.filePath }
            }
        }
    }
    default { exit 0 }
}

$filePaths = $filePaths | Select-Object -Unique
if ($filePaths.Count -eq 0) { exit 0 }

$issues = [System.Collections.Generic.List[string]]::new()
$hasRustFile = $false
$hasTsFile = $false

function Invoke-Tool {
    param([string]$Label, [scriptblock]$Cmd)
    $output = & $Cmd 2>&1 | Out-String
    $exitCode = $LASTEXITCODE
    if ($exitCode -ne 0 -and $output.Trim()) {
        $issues.Add("[${Label}]`n$($output.Trim())")
    }
}

foreach ($filePath in $filePaths) {
    if (-not (Test-Path $filePath)) { continue }

    $ext = [System.IO.Path]::GetExtension($filePath).ToLower()

    if ($ext -in @('.ts', '.tsx', '.js', '.jsx', '.css', '.json')) {
        Invoke-Tool "prettier: $filePath" { pnpm exec prettier --write $filePath }
        if ($ext -in @('.ts', '.tsx', '.js', '.jsx')) {
            Invoke-Tool "eslint: $filePath" { pnpm exec eslint --fix $filePath }
        }
        if ($ext -in @('.ts', '.tsx')) {
            $hasTsFile = $true
        }
    } elseif ($ext -eq '.rs') {
        $hasRustFile = $true
    }
}

if ($hasRustFile) {
    $tauriDir = Join-Path $PSScriptRoot ".." ".." ".." "src-tauri"
    Push-Location $tauriDir
    Invoke-Tool "cargo fmt" { cargo fmt }
    Invoke-Tool "cargo clippy" { cargo clippy -- -D warnings }
    Pop-Location
}

if ($hasTsFile) {
    Invoke-Tool "tsc" { pnpm exec tsc --noEmit }
}

if ($issues.Count -gt 0) {
    $context = "Post-edit linting/formatting found issues that need fixing:`n`n" + ($issues -join "`n`n")
    $payload = [pscustomobject]@{
        hookSpecificOutput = [pscustomobject]@{
            hookEventName    = "PostToolUse"
            additionalContext = $context
        }
    }
    $payload | ConvertTo-Json -Depth 5 -Compress | Write-Output
}
