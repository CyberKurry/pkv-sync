$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location -LiteralPath $repoRoot

$isWindowsRunner = [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
    [System.Runtime.InteropServices.OSPlatform]::Windows
)
$exeName = if ($isWindowsRunner) { "pkvsyncd.exe" } else { "pkvsyncd" }
$bin = Join-Path $repoRoot (Join-Path "target" (Join-Path "release" $exeName))
if (-not (Test-Path -LiteralPath $bin)) {
    throw "Release binary not found at $bin"
}
$curl = if ($isWindowsRunner) { "curl.exe" } else { "curl" }
$nullOutput = if ($isWindowsRunner) { "NUL" } else { "/dev/null" }

$tempBase = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }
$root = Join-Path $tempBase ("pkv-ci-smoke-" + [guid]::NewGuid().ToString("N"))
$data = Join-Path $root "data"
New-Item -ItemType Directory -Force -Path $data | Out-Null

$listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Parse("127.0.0.1"), 0)
$listener.Start()
$port = $listener.LocalEndpoint.Port
$listener.Stop()

$key = ((& $bin genkey) | Select-Object -First 1).Trim()
$cfg = Join-Path $root "config.toml"
$dataToml = $data -replace "\\", "/"
$dbToml = (Join-Path $data "metadata.db") -replace "\\", "/"

@"
[server]
bind_addr = "127.0.0.1:$port"
deployment_key = "$key"

[storage]
data_dir = "$dataToml"
db_path = "$dbToml"

[network]
trusted_proxies = ["127.0.0.1/32"]
"@ | Set-Content -LiteralPath $cfg -Encoding ASCII

& $bin -c $cfg migrate up | Out-Null
"passw0rd!!" | & $bin -c $cfg user add bob | Out-Null
"newpass1234" | & $bin -c $cfg user passwd bob | Out-Null

$stdout = Join-Path $root "serve.out.log"
$stderr = Join-Path $root "serve.err.log"
$proc = $null

try {
    $startArgs = @{
        FilePath = $bin
        ArgumentList = @("-c", $cfg, "serve")
        PassThru = $true
        RedirectStandardOutput = $stdout
        RedirectStandardError = $stderr
    }
    if ($isWindowsRunner) {
        $startArgs.WindowStyle = "Hidden"
    }
    $proc = Start-Process @startArgs

    $base = "http://127.0.0.1:$port"
    $bodyFile = Join-Path $root "body.json"
    $ready = $false

    for ($i = 0; $i -lt 50; $i++) {
        $code = & $curl `
            -sS `
            --max-time 2 `
            -o $nullOutput `
            -w "%{http_code}" `
            -H "user-agent: PKVSync-Plugin/0.1.0" `
            -H "x-pkvsync-deployment-key: $key" `
            "$base/api/health"

        if ($code -eq "200") {
            $ready = $true
            break
        }
        Start-Sleep -Milliseconds 100
    }

    if (-not $ready) {
        $errText = if (Test-Path -LiteralPath $stderr) { Get-Content -Raw -LiteralPath $stderr } else { "" }
        throw "Server did not become ready. stderr: $errText"
    }

    $loginFile = Join-Path $root "login.json"
    '{"username":"bob","password":"newpass1234","device_name":"ci"}' |
        Set-Content -LiteralPath $loginFile -NoNewline -Encoding ASCII

    $code = & $curl `
        -sS `
        --max-time 5 `
        -o $bodyFile `
        -w "%{http_code}" `
        -H "user-agent: PKVSync-Plugin/0.1.0" `
        -H "x-pkvsync-deployment-key: $key" `
        -H "content-type: application/json" `
        --data-binary "@$loginFile" `
        "$base/api/auth/login"

    $loginText = Get-Content -Raw -LiteralPath $bodyFile
    if ($code -ne "200") {
        throw "Login failed with HTTP $code. Body: $loginText"
    }

    $login = $loginText | ConvertFrom-Json
    if (-not $login.token) {
        throw "Login response did not include a token. Body: $loginText"
    }

    $code = & $curl `
        -sS `
        --max-time 5 `
        -o $bodyFile `
        -w "%{http_code}" `
        -H "user-agent: PKVSync-Plugin/0.1.0" `
        -H "x-pkvsync-deployment-key: $key" `
        -H "authorization: Bearer $($login.token)" `
        "$base/api/me"

    $meText = Get-Content -Raw -LiteralPath $bodyFile
    if ($code -ne "200") {
        throw "/api/me failed with HTTP $code. Body: $meText"
    }

    $me = $meText | ConvertFrom-Json
    if ($me.username -ne "bob") {
        throw "/api/me returned the wrong user. Body: $meText"
    }

    Write-Output "Smoke OK: health=200 login_user=$($login.username) me_user=$($me.username) vaults=$($me.vaults.Count)"
} finally {
    if ($proc -and -not $proc.HasExited) {
        Stop-Process -Id $proc.Id -Force
    }
}
