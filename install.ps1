#requires -Version 5.1
<# Installs the latest official Bucky VPN release; a non-Administrator session requests UAC elevation automatically. #>
$BuckyOfficialScriptUrl = 'https://raw.githubusercontent.com/buckyos/bucky-vpn/master/install.ps1'
$script:DelegatedToElevated = $false
$script:BuckyRequestUri = $null
$script:BuckyLastRequestUri = $null

function Test-BuckyAdministrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Get-BuckyElevationLaunch {
    param([string] $ScriptPath)

    if ([string]::IsNullOrEmpty($ScriptPath)) {
        # Script text is unavailable under iex, so the child fetches the official copy.
        $payload = '$env:BUCKY_VPN_ELEVATED=''1''; irm ''' + $BuckyOfficialScriptUrl + ''' | iex'
        return [pscustomobject]@{ Arguments = '-NoProfile -ExecutionPolicy Bypass -Command "' + $payload + '"' }
    }
    if (-not [IO.File]::Exists($ScriptPath)) { throw "Cannot elevate: the install script was not found at $ScriptPath" }
    return [pscustomobject]@{ Arguments = '-NoProfile -ExecutionPolicy Bypass -File "' + $ScriptPath + '"' }
}

function Invoke-BuckyElevation {
    param([string] $ScriptPath)

    $launch = Get-BuckyElevationLaunch -ScriptPath $ScriptPath
    Write-Host 'Administrator rights are required. Requesting UAC elevation...'
    try {
        $process = Start-Process -FilePath 'powershell.exe' -ArgumentList $launch.Arguments -Verb RunAs -Wait -PassThru -ErrorAction Stop
    }
    catch {
        $cancelled = $false
        $exception = $_.Exception
        while ($null -ne $exception) {
            if ($exception -is [ComponentModel.Win32Exception] -and $exception.NativeErrorCode -eq 1223) { $cancelled = $true; break }
            $exception = $exception.InnerException
        }
        if ($cancelled) { Write-Warning 'Elevation was canceled; the Bucky VPN installation was not started.' }
        else { Write-Warning ('Could not start the elevated installer: ' + $_.Exception.Message) }
        return 1
    }
    if ($null -eq $process -or $null -eq $process.ExitCode) {
        Write-Warning 'The elevated installer did not return an exit code.'
        return 1
    }
    $script:DelegatedToElevated = $true
    $result = [int]$process.ExitCode
    Write-Host "The elevated installer finished with exit code $result."
    return $result
}

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
            $script:BuckyLastRequestUri = $Uri.AbsoluteUri
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

function Get-BuckyExceptionDetail {
    param([Exception] $Exception, [switch] $IncludeStack)

    if ($null -eq $Exception) { return '' }
    $lines = New-Object 'System.Collections.Generic.List[string]'
    $current = $Exception
    for ($depth = 0; $null -ne $current -and $depth -lt 32; $depth++) {
        try {
            $lines.Add('[' + $depth + '] ' + $current.GetType().FullName + ': ' + $current.Message)
            if ($current -is [ComponentModel.Win32Exception]) {
                $lines.Add('    NativeErrorCode: ' + $current.NativeErrorCode)
            }
            if ($IncludeStack) {
                foreach ($frame in ([string]$current.StackTrace -split "`r?`n")) {
                    if ($frame.Trim()) { $lines.Add('    ' + $frame.Trim()) }
                }
            }
        }
        catch {
            $lines.Add('[' + $depth + '] detail unavailable: ' + $_.Exception.Message)
            break
        }
        $current = $current.InnerException
    }
    return ($lines -join [Environment]::NewLine)
}

function Write-BuckyFailureReport {
    param([Exception] $Exception, [string] $RequestUri, [string] $InitialRequestUri, [string] $InstallLog, [string] $ScriptStack)

    try {
        $osVersion = [Environment]::OSVersion.VersionString
        if ([string]::IsNullOrWhiteSpace($osVersion)) { $osVersion = [Environment]::OSVersion.ToString() }
        $summary = 'unknown'
        if ($null -ne $Exception) { $summary = $Exception.Message }
        $lines = New-Object 'System.Collections.Generic.List[string]'
        $lines.Add('Bucky VPN installation failure report')
        $lines.Add('Time: ' + (Get-Date).ToString('o'))
        $lines.Add('PowerShell version: ' + $PSVersionTable.PSVersion.ToString())
        $lines.Add('OS version: ' + $osVersion)
        $lines.Add('User: ' + [Environment]::UserName)
        $lines.Add('Initial request: ' + $InitialRequestUri)
        $lines.Add('Request URL: ' + $RequestUri)
        if ($InstallLog) { $lines.Add('Installation log: ' + $InstallLog) }
        $lines.Add('Summary: ' + $summary)
        $lines.Add('Exception chain:')
        $lines.Add((Get-BuckyExceptionDetail -Exception $Exception -IncludeStack))
        if ($ScriptStack) {
            # PowerShell-thrown exceptions carry the script location here, not in StackTrace.
            $lines.Add('Script stack:')
            foreach ($frame in ($ScriptStack -split "`r?`n")) {
                if ($frame.Trim()) { $lines.Add('    ' + $frame.Trim()) }
            }
        }
        $path = Join-Path ([IO.Path]::GetTempPath()) ('bucky-vpn-error-' + [Guid]::NewGuid().ToString('N') + '.log')
        [IO.File]::WriteAllText($path, (($lines -join [Environment]::NewLine) + [Environment]::NewLine), [Text.Encoding]::UTF8)
        return $path
    }
    catch {
        Write-Warning ('Could not write the failure report: ' + $_.Exception.Message)
        return $null
    }
}

function Invoke-BuckyInstall {
    $ErrorActionPreference = 'Stop'
    $temporaryDirectory = $null
    $logPath = $null
    $result = 1
    # A reused session must never report a URL from an earlier attempt.
    $script:BuckyRequestUri = $null
    $script:BuckyLastRequestUri = $null
    try {
        if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) { throw 'This script supports Windows x64 only.' }
        $nativeArchitecture = $env:PROCESSOR_ARCHITEW6432
        if (-not $nativeArchitecture) { $nativeArchitecture = $env:PROCESSOR_ARCHITECTURE }
        if ($nativeArchitecture -ne 'AMD64') { throw 'Only Windows x64 is supported; ARM64 and x86 are not supported.' }
        if (-not (Test-BuckyAdministrator)) { return (Invoke-BuckyElevation -ScriptPath $PSCommandPath) }

        $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
        $temporaryDirectory = Join-Path $tempRoot ('bucky-vpn-download-' + [Guid]::NewGuid().ToString('N'))
        [void][IO.Directory]::CreateDirectory($temporaryDirectory)
        $metadataPath = Join-Path $temporaryDirectory 'release.json'
        Write-Host 'Looking up the latest official Bucky VPN release...'
        $releaseUri = 'https://api.github.com/repos/buckyos/bucky-vpn/releases/latest'
        $script:BuckyRequestUri = $releaseUri
        Save-BuckyHttpsFile -Uri $releaseUri -Destination $metadataPath
        # ConvertFrom-Json errors are terminating under this function's ErrorActionPreference.
        $metadata = [IO.File]::ReadAllText($metadataPath)
        if (-not $metadata.TrimStart().StartsWith('{')) { throw 'Expected a GitHub release JSON object.' }
        $release = ConvertFrom-Json -InputObject $metadata
        $asset = Get-BuckyReleaseAsset -Release $release -Platform windows
        $packagePath = Join-Path $temporaryDirectory $asset.Name
        $script:BuckyRequestUri = $asset.Url
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
        $failure = $_.Exception
        $requestUri = $script:BuckyLastRequestUri
        if ([string]::IsNullOrEmpty($requestUri)) { $requestUri = $script:BuckyRequestUri }
        $detail = Get-BuckyExceptionDetail -Exception $failure
        $reportPath = Write-BuckyFailureReport -Exception $failure -RequestUri $requestUri -InitialRequestUri $script:BuckyRequestUri -InstallLog $logPath -ScriptStack $_.ScriptStackTrace
        $summary = 'unknown error'
        if ($null -ne $failure) { $summary = $failure.Message }
        Write-Warning ('Bucky VPN installation failed: ' + $summary)
        if ($requestUri) { Write-Warning ('Request URL: ' + $requestUri) }
        if ($detail) { Write-Warning ('Exception chain:' + [Environment]::NewLine + $detail) }
        if ($reportPath) { Write-Warning ('Failure details were written to ' + $reportPath) }
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
    $result = 1
    if ($args.Count -ne 0) { Write-Warning 'Usage: powershell -File install.ps1 (no arguments)' }
    else { $result = Invoke-BuckyInstall }
    $elevatedChild = $env:BUCKY_VPN_ELEVATED -eq '1'
    # Empty $MyInvocation.Line means no calling session: -File, double-click or iex.
    $scriptHosted = [string]::IsNullOrEmpty($MyInvocation.Line)
    if ($scriptHosted -and -not $elevatedChild -and [string]::IsNullOrEmpty($PSScriptRoot)) {
        # irm | iex belongs to the caller's session; never exit here or that window closes.
        $global:LASTEXITCODE = $result
    }
    elseif ($scriptHosted -and -not $script:DelegatedToElevated -and -not [Console]::IsInputRedirected) {
        # This console belongs to the process and would disappear with it.
        Write-Host 'Press any key to close this window.'
        try { [void][Console]::ReadKey($true) }
        catch { Write-Host 'Press Enter to close this window.'; [void](Read-Host) }
        exit $result
    }
    else { exit $result }
}
