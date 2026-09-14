[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidatePattern("^[A-Za-z0-9_-]+$")]
    [string]$Environment,

    [Parameter(Position = 1, ValueFromRemainingArguments = $true)]
    [AllowEmptyCollection()]
    [string[]]$SurrealKitArguments = @()
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($Environment) -or $SurrealKitArguments.Count -eq 0) {
    Write-Host @"
Usage: .\scripts\db.ps1 <environment> <surrealkit arguments...>

Examples:
  .\scripts\db.ps1 staging status
  .\scripts\db.ps1 staging sync --fail-fast
  .\scripts\db.ps1 staging seed
  .\scripts\db.ps1 staging rollout status
  .\scripts\db.ps1 staging rollout plan --name add-tags
"@
    return
}

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$envFile = Join-Path $repoRoot ".env.surrealkit.$Environment"

if (-not (Test-Path -LiteralPath $envFile -PathType Leaf)) {
    throw "Database environment file not found: $envFile. Copy .env.surrealkit.template and fill in its values."
}

$allowedKeys = @(
    "G3_DATABASE_ENVIRONMENT",
    "SURREALDB_HOST",
    "SURREALDB_USER",
    "SURREALDB_PASSWORD",
    "SURREALDB_NAMESPACE",
    "SURREALDB_NAME",
    "SURREALDB_AUTH_LEVEL",
    "SURREALDB_FOLDER"
)

$settings = [ordered]@{}

foreach ($rawLine in Get-Content -LiteralPath $envFile) {
    $line = $rawLine.Trim()

    if (-not $line -or $line.StartsWith("#")) {
        continue
    }

    if ($line.StartsWith("export ")) {
        $line = $line.Substring(7).TrimStart()
    }

    $separator = $line.IndexOf("=")
    if ($separator -le 0) {
        throw "Invalid line in ${envFile}: $rawLine"
    }

    $key = $line.Substring(0, $separator).Trim()
    $value = $line.Substring($separator + 1).Trim()

    if ($key -notin $allowedKeys -and $key -notmatch "^SURREALKIT_VAR_[A-Z0-9_]+$") {
        throw "Unsupported variable '$key' in $envFile. Keep application settings in the normal .env files."
    }

    if ($value.Length -ge 2) {
        $first = $value[0]
        $last = $value[$value.Length - 1]
        if (($first -eq '"' -and $last -eq '"') -or ($first -eq "'" -and $last -eq "'")) {
            $value = $value.Substring(1, $value.Length - 2)
        }
    }

    if ($settings.Contains($key)) {
        throw "Duplicate variable '$key' in $envFile."
    }

    $settings[$key] = $value
}

$requiredKeys = @(
    "G3_DATABASE_ENVIRONMENT",
    "SURREALDB_HOST",
    "SURREALDB_USER",
    "SURREALDB_PASSWORD",
    "SURREALDB_NAMESPACE",
    "SURREALDB_NAME"
)

foreach ($key in $requiredKeys) {
    if (-not $settings.Contains($key) -or [string]::IsNullOrWhiteSpace($settings[$key])) {
        throw "Required variable '$key' is missing or empty in $envFile."
    }
}

if ($settings["G3_DATABASE_ENVIRONMENT"] -ne $Environment) {
    throw "Environment mismatch: requested '$Environment', but $envFile declares '$($settings["G3_DATABASE_ENVIRONMENT"])'."
}

$placeholderPattern = "(?i)(replace[_-]?me|your[-.]tailnet|your-service|example_.*do_not_use)"
foreach ($key in @("SURREALDB_HOST", "SURREALDB_PASSWORD", "SURREALDB_NAMESPACE", "SURREALDB_NAME")) {
    if ($settings[$key] -match $placeholderPattern) {
        throw "Replace the placeholder value for '$key' in $envFile before running SurrealKit."
    }
}

$surrealKit = Get-Command surrealkit -CommandType Application -ErrorAction Stop
$forwardedKeys = @(
    $settings.Keys | Where-Object {
        $_ -ne "G3_DATABASE_ENVIRONMENT"
    }
)

$previous = @{}
foreach ($key in $forwardedKeys) {
    $previous[$key] = [pscustomobject]@{
        Exists = Test-Path -LiteralPath "Env:$key"
        Value = [Environment]::GetEnvironmentVariable($key, "Process")
    }
    [Environment]::SetEnvironmentVariable($key, $settings[$key], "Process")
}

$locationPushed = $false
$exitCode = 0

try {
    Push-Location $repoRoot
    $locationPushed = $true

    Write-Host "SurrealKit target: $Environment -> $($settings["SURREALDB_HOST"]) ($($settings["SURREALDB_NAMESPACE"])/$($settings["SURREALDB_NAME"]))"
    & $surrealKit.Source @SurrealKitArguments
    $exitCode = $LASTEXITCODE
}
finally {
    if ($locationPushed) {
        Pop-Location
    }

    foreach ($key in $forwardedKeys) {
        if ($previous[$key].Exists) {
            [Environment]::SetEnvironmentVariable($key, $previous[$key].Value, "Process")
        }
        else {
            [Environment]::SetEnvironmentVariable($key, $null, "Process")
        }
    }
}

if ($exitCode -ne 0) {
    throw "SurrealKit exited with code $exitCode."
}
