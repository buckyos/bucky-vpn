#requires -Version 5.1
<# Run from an Administrator PowerShell session. #>

function Get-BuckyReleaseAsset {
    param([object] $Release, [string] $Platform)

    if ($null -eq $Release -or $Release -is [array] -or
        $Release.draft -isnot [bool] -or $Release.draft -or
        $Release.prerelease -isnot [bool] -or $Release.prerelease -or
        $Release.tag_name -isnot [string] -or
        $Release.tag_name -cnotmatch '^v?([0-9]+\.[0-9]+\.[0-9]+)$' -or
        $Release.assets -isnot [array]) {
        throw 'GitHub returned invalid latest-release metadata (expected a published vN.N.N release).'
    }
    $version = $Release.tag_name -creplace '^v', ''
    switch ($Platform) {
        'windows' { $name = "BuckyVPN_${version}_amd64_Setup.exe" }
        'linux' { $name = "bucky-vpn_${version}_amd64.deb" }
        'macos' { $name = "BuckyVPN-${version}.pkg" }
        default { throw "Unsupported platform: $Platform" }
    }
    $assets = @($Release.assets | Where-Object { $_.name -ceq $name })
    if ($assets.Count -ne 1) { throw "Expected exactly one release asset named $name." }
    $asset = $assets[0]
    $expectedUrl = "https://github.com/buckyos/bucky-vpn/releases/download/$($Release.tag_name)/$name"
    if ($asset.state -cne 'uploaded' -or $asset.digest -isnot [string] -or
        $asset.digest -cnotmatch '^sha256:[0-9a-fA-F]{64}$' -or
        $asset.browser_download_url -isnot [string] -or
        $asset.browser_download_url -cne $expectedUrl) {
        throw "Asset $name has no valid SHA-256 digest or official download URL, or is not uploaded."
    }
    [pscustomobject]@{
        Name = $name; Version = $version; Url = $expectedUrl
        Sha256 = $asset.digest.Substring(7)
    }
}

function Save-BuckyHttpsFile {
    param([uri] $Uri, [string] $Destination, [switch] $ShowProgress)

    Add-Type -AssemblyName System.Net.Http
    # PowerShell 5.1 may otherwise negotiate obsolete TLS defaults.
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    $handler = New-Object Net.Http.HttpClientHandler
    $handler.AllowAutoRedirect = $false
    $client = New-Object Net.Http.HttpClient($handler)
    $client.Timeout = [TimeSpan]::FromMinutes(15)
    $cancel = New-Object Threading.CancellationTokenSource
    $cancel.CancelAfter([TimeSpan]::FromMinutes(15))
    $response = $null
    $activity = 'Downloading ' + [IO.Path]::GetFileName($Destination)
    try {
        [void]$client.DefaultRequestHeaders.TryAddWithoutValidation('User-Agent', 'BuckyVPN-installer')
        [void]$client.DefaultRequestHeaders.TryAddWithoutValidation('Accept', 'application/vnd.github+json')
        [void]$client.DefaultRequestHeaders.TryAddWithoutValidation('X-GitHub-Api-Version', '2022-11-28')
        for ($redirects = 0; $redirects -le 10; $redirects++) {
            if (-not $Uri.IsAbsoluteUri -or $Uri.Scheme -cne 'https' -or $Uri.UserInfo) {
                throw 'Refusing a non-HTTPS or credential-bearing download URL.'
            }
            $response = $client.GetAsync($Uri, [Net.Http.HttpCompletionOption]::ResponseHeadersRead, $cancel.Token).GetAwaiter().GetResult()
            $status = [int]$response.StatusCode
            if ($status -in @(301, 302, 303, 307, 308)) {
                $location = $response.Headers.Location
                if ($null -eq $location -or $redirects -eq 10) { throw 'Invalid or excessive HTTPS redirects.' }
                $Uri = New-Object Uri($Uri, $location)
                $response.Dispose()
                $response = $null
                continue
            }
            if ($status -ne 200) { throw "HTTPS request failed with HTTP $status. GitHub may be unavailable or rate limited." }
            $output = [IO.File]::Open($Destination, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
            $inputStream = $null
            try {
                $inputStream = $response.Content.ReadAsStreamAsync().GetAwaiter().GetResult()
                $buffer = New-Object byte[] 65536
                $received = [long]0
                $total = $response.Content.Headers.ContentLength
                $progressClock = [Diagnostics.Stopwatch]::StartNew()
                if ($ShowProgress) {
                    $initialPercent = -1
                    if ($total -gt 0) { $initialPercent = 0 }
                    Write-Progress -Id 1 -Activity $activity -Status '0 bytes downloaded' -PercentComplete $initialPercent
                }
                while ($true) {
                    $readTask = $inputStream.ReadAsync($buffer, 0, $buffer.Length, $cancel.Token)
                    # Framework streams may not honor ReadAsync cancellation while blocked.
                    # Bound the wait too; finally disposes the response on timeout.
                    $readTask.Wait($cancel.Token)
                    $count = $readTask.GetAwaiter().GetResult()
                    if ($count -eq 0) { break }
                    $cancel.Token.ThrowIfCancellationRequested()
                    $output.Write($buffer, 0, $count)
                    $received += $count
                    if ($ShowProgress -and ($progressClock.ElapsedMilliseconds -ge 200 -or ($total -gt 0 -and $received -ge $total))) {
                        $percent = -1
                        $statusText = "$received bytes downloaded"
                        if ($total -gt 0) {
                            $percent = [int][Math]::Min(100, [Math]::Floor(100.0 * $received / $total))
                            $statusText += " of $total bytes"
                        }
                        Write-Progress -Id 1 -Activity $activity -Status $statusText -PercentComplete $percent
                        $progressClock.Restart()
                    }
                }
                $cancel.Token.ThrowIfCancellationRequested()
                if ($null -ne $total -and $received -ne $total) { throw 'Incomplete HTTPS response body.' }
                if ($ShowProgress) {
                    $finalPercent = -1
                    if ($null -ne $total) { $finalPercent = 100 }
                    Write-Progress -Id 1 -Activity $activity -Status "$received bytes downloaded" -PercentComplete $finalPercent
                }
            }
            finally {
                if ($null -ne $inputStream) { $inputStream.Dispose() }
                $output.Dispose()
            }
            return
        }
    }
    finally {
        if ($null -ne $response) { $response.Dispose() }
        $cancel.Dispose()
        $client.Dispose()
        $handler.Dispose()
        if ($ShowProgress) { Write-Progress -Id 1 -Activity $activity -Completed }
    }
}

function Invoke-BuckyInstall {
    $ErrorActionPreference = 'Stop'
    $temporaryDirectory = $null
    $logPath = $null
    $result = 1
    try {
        if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) { throw 'This script supports Windows x64 only.' }
        $nativeArchitecture = $env:PROCESSOR_ARCHITEW6432
        if (-not $nativeArchitecture) { $nativeArchitecture = $env:PROCESSOR_ARCHITECTURE }
        if ($nativeArchitecture -ne 'AMD64') { throw 'Only Windows x64 is supported; ARM64 and x86 are not supported.' }
        $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
        $principal = New-Object Security.Principal.WindowsPrincipal($identity)
        if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
            throw 'Open PowerShell as Administrator, then run this script again.'
        }

        $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
        $temporaryDirectory = Join-Path $tempRoot ('bucky-vpn-download-' + [Guid]::NewGuid().ToString('N'))
        [void][IO.Directory]::CreateDirectory($temporaryDirectory)
        $metadataPath = Join-Path $temporaryDirectory 'release.json'
        Write-Host 'Looking up the latest official Bucky VPN release...'
        Save-BuckyHttpsFile -Uri 'https://api.github.com/repos/buckyos/bucky-vpn/releases/latest' -Destination $metadataPath
        # ConvertFrom-Json errors are terminating under this function's ErrorActionPreference.
        $metadata = [IO.File]::ReadAllText($metadataPath)
        if (-not $metadata.TrimStart().StartsWith('{')) { throw 'Expected a GitHub release JSON object.' }
        $release = ConvertFrom-Json -InputObject $metadata
        $asset = Get-BuckyReleaseAsset -Release $release -Platform windows
        $packagePath = Join-Path $temporaryDirectory $asset.Name
        Write-Host "Downloading $($asset.Name)..."
        Save-BuckyHttpsFile -Uri $asset.Url -Destination $packagePath -ShowProgress
        $actualDigest = (Get-FileHash -LiteralPath $packagePath -Algorithm SHA256).Hash
        if ($actualDigest -ine $asset.Sha256) { throw 'SHA-256 mismatch. The installer will not be executed.' }

        $logPath = Join-Path $tempRoot ('bucky-vpn-install-' + [Guid]::NewGuid().ToString('N') + '.log')
        Write-Host "Installing Bucky VPN $($asset.Version) silently. Installation log: $logPath"
        $arguments = @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/SP-', '/NORESTART', '/RESTARTEXITCODE=3010', ('/LOG="' + $logPath + '"'))
        $process = Start-Process -FilePath $packagePath -ArgumentList $arguments -Wait -PassThru
        if ($null -eq $process -or $null -eq $process.ExitCode) { throw 'The installer did not return an exit code.' }
        $result = [int]$process.ExitCode
        if ($result -ne 0 -and $result -ne 3010) { Write-Warning "Bucky VPN installation failed with exit code $result. See $logPath" }
    }
    catch {
        Write-Warning ("Bucky VPN installation failed: " + $_.Exception.Message)
        if ($logPath) { Write-Warning "Installation log (if created): $logPath" }
        $result = 1
    }
    finally {
        if ($temporaryDirectory -and [IO.Directory]::Exists($temporaryDirectory)) {
            # This random directory is created by this invocation beneath the resolved temp root.
            $resolvedDirectory = [IO.Path]::GetFullPath($temporaryDirectory)
            $expectedParent = $tempRoot.TrimEnd('\', '/')
            if ([IO.Directory]::GetParent($resolvedDirectory).FullName.TrimEnd('\', '/') -ne $expectedParent -or
                [IO.Path]::GetFileName($resolvedDirectory) -notmatch '^bucky-vpn-download-[0-9a-f]{32}$') {
                Write-Warning 'Refusing to clean an unexpected temporary directory.'
                if ($result -eq 0 -or $result -eq 3010) { $result = 1 }
            }
            else {
                try { Remove-Item -LiteralPath $resolvedDirectory -Recurse -Force -ErrorAction Stop }
                catch {
                    Write-Warning "Could not remove temporary downloads at $resolvedDirectory : $($_.Exception.Message)"
                    if ($result -eq 0 -or $result -eq 3010) { $result = 1 }
                }
            }
        }
    }
    if ($result -eq 0) { Write-Host 'Bucky VPN installed successfully. Open a new terminal to use bucky-vpn.' }
    elseif ($result -eq 3010) { Write-Host 'Bucky VPN installed successfully. A restart is required; no restart was performed.' }
    return $result
}

if ($MyInvocation.InvocationName -ne '.') {
    if ($args.Count -ne 0) { Write-Warning 'Usage: powershell -File install.ps1 (no arguments)'; exit 1 }
    exit (Invoke-BuckyInstall)
}
