param([string] $Mode, [string] $Repository, [string] $Work)
$ErrorActionPreference = 'Stop'
. (Join-Path $Repository 'install.ps1')
function Assert($Condition, [string] $Message) { if (-not $Condition) { throw $Message } }
# Reported failure reports are retained for users; tests must remove only the exact reported ones.
# Redirected child hosts wrap long warnings at the console width, so the path may span lines.
function Remove-BuckyReportedFailureLog($Output) {
    $rendered = @($Output) | Out-String -Width 4096
    foreach ($match in [regex]::Matches($rendered, '(?s)Failure details were written to\s+(.+?\.log)')) {
        $reportedPath = ($match.Groups[1].Value -replace "(\r?\n)\s*", '').Trim()
        if ($reportedPath -and (Test-Path -LiteralPath $reportedPath)) {
            Microsoft.PowerShell.Management\Remove-Item -LiteralPath $reportedPath -Force
        }
    }
}

if ($Mode -eq 'unit') {
    # Exercise the original platform guard, with network and process execution denied.
    $savedArchitecture = $env:PROCESSOR_ARCHITEW6432
    $script:guardDownloaded = $false
    $guardMessages = @()
    function Save-BuckyHttpsFile { $script:guardDownloaded = $true; throw 'unexpected download during architecture denial' }
    try {
        $env:PROCESSOR_ARCHITEW6432 = 'ARM64'
        $guardMessages = @(Invoke-BuckyInstall *>&1)
        Assert (@($guardMessages | Where-Object { $_ -is [int] })[-1] -eq 1) 'ARM64 was not rejected.'
        Assert (-not $script:guardDownloaded) 'Unsupported native architecture reached network.'
    }
    finally {
        $env:PROCESSOR_ARCHITEW6432 = $savedArchitecture
        Remove-BuckyReportedFailureLog $guardMessages
    }
    . (Join-Path $Repository 'install.ps1')
    $count = 0
    foreach ($fixture in Get-ChildItem -LiteralPath $Work -Filter '*.json') {
        foreach ($platform in @('windows', 'linux', 'macos')) {
            $accepted = $false
            try {
                $value = ConvertFrom-Json ([IO.File]::ReadAllText($fixture.FullName))
                $selected = Get-BuckyReleaseAsset $value $platform
                $accepted = $true
                Assert ($selected.Version -eq '2.3.4') 'Version was not derived from the tag.'
            }
            catch { if ($fixture.Name.StartsWith('valid')) { throw } }
            Assert ($accepted -eq $fixture.Name.StartsWith('valid')) ("Unexpected selection result: " + $fixture.Name)
            $count++
        }
    }
    $rejected = $false
    try { Save-BuckyHttpsFile -Uri 'http://example.invalid' -Destination (Join-Path $Work 'unsafe') }
    catch { $rejected = $true }
    Assert $rejected 'HTTP input was accepted.'
    Assert (-not (Test-Path (Join-Path $Work 'unsafe'))) 'Unsafe transfer created a file.'
    Write-Host "PowerShell metadata branches: $count plus HTTP rejection passed"

    # Elevation launch construction, delegation and UAC refusal. Network and UAC
    # are replaced by in-process overrides so nothing is downloaded or installed.
    $installerPath = Join-Path $Repository 'install.ps1'
    $savedElevated = $env:BUCKY_VPN_ELEVATED
    $env:BUCKY_VPN_ELEVATED = $null
    try {
        $fileLaunch = Get-BuckyElevationLaunch -ScriptPath $installerPath
        Assert ($fileLaunch.Arguments.Contains('-ExecutionPolicy Bypass -File "' + $installerPath + '"')) 'File elevation did not reuse the script path.'
        Assert (-not $fileLaunch.Arguments.Contains('irm ')) 'File elevation refetched the online script.'
        $spacedPath = Join-Path $Work 'script with space.ps1'
        [IO.File]::WriteAllText($spacedPath, '')
        $spacedLaunch = Get-BuckyElevationLaunch -ScriptPath $spacedPath
        Assert ($spacedLaunch.Arguments.Contains('-File "' + $spacedPath + '"')) 'A script path containing spaces was not quoted.'
        $onlineLaunch = Get-BuckyElevationLaunch -ScriptPath ''
        Assert ($onlineLaunch.Arguments.Contains('-Command')) 'Online elevation did not use -Command.'
        Assert ($onlineLaunch.Arguments.Contains($BuckyOfficialScriptUrl)) 'Online elevation lost the official script URL.'
        Assert ($onlineLaunch.Arguments.Contains('BUCKY_VPN_ELEVATED')) 'Online elevation lost the elevated-child marker.'
        Assert ((Get-BuckyElevationLaunch -ScriptPath $null).Arguments -eq $onlineLaunch.Arguments) 'A null script path did not select the online form.'
        $missingRejected = $false
        try { Get-BuckyElevationLaunch -ScriptPath (Join-Path $Work 'missing-install.ps1') } catch { $missingRejected = $true }
        Assert $missingRejected 'A missing script path was accepted for elevation.'
        Write-Host 'PowerShell elevation launch construction passed'

        # Non-administrator: delegate once and return the elevated child status.
        function Test-BuckyAdministrator { return $false }
        $script:startProcessCalls = @()
        $script:elevationDownloaded = $false
        function Start-Process {
            param($FilePath, $ArgumentList, $Verb, [switch]$Wait, [switch]$PassThru, $ErrorAction)
            $script:startProcessCalls += [pscustomobject]@{
                FilePath = $FilePath; ArgumentList = $ArgumentList; Verb = $Verb
                Wait = $Wait.IsPresent; PassThru = $PassThru.IsPresent
            }
            return [pscustomobject]@{ ExitCode = 7 }
        }
        function Save-BuckyHttpsFile {
            param([uri] $Uri, [string] $Destination, [switch] $ShowProgress)
            $script:elevationDownloaded = $true
            throw 'unexpected download during elevation'
        }
        $elevationMessages = @(Invoke-BuckyInstall *>&1)
        Assert (@($elevationMessages | Where-Object { $_ -is [int] })[-1] -eq 7) 'The elevated child status was not propagated.'
        Assert ($script:startProcessCalls.Count -eq 1) 'Elevation did not start exactly one child process.'
        $call = $script:startProcessCalls[0]
        Assert ($call.FilePath -eq 'powershell.exe') 'Elevation did not launch powershell.exe.'
        Assert ($call.ArgumentList.Contains('-File "' + $installerPath + '"')) 'Elevation did not rerun this install script.'
        Assert ($call.Verb -eq 'RunAs') 'Elevation did not request -Verb RunAs.'
        Assert ($call.Wait -and $call.PassThru) 'Elevation did not wait for the child status.'
        Assert (-not $script:elevationDownloaded) 'Elevation downloaded a package in the parent process.'
        Write-Host 'PowerShell elevation delegation passed'

        # Canceled UAC (Win32 1223) must fail without installing.
        function Start-Process {
            param($FilePath, $ArgumentList, $Verb, [switch]$Wait, [switch]$PassThru, $ErrorAction)
            throw (New-Object ComponentModel.Win32Exception 1223)
        }
        $cancelMessages = @(Invoke-BuckyInstall *>&1)
        Assert (@($cancelMessages | Where-Object { $_ -is [int] })[-1] -eq 1) 'A canceled UAC prompt did not return 1.'
        Assert ((($cancelMessages | Out-String) -match 'canceled')) 'No UAC cancellation warning was reported.'
        Assert (-not $script:elevationDownloaded) 'A canceled UAC prompt started a download.'
        Write-Host 'PowerShell UAC cancellation passed'
    }
    finally {
        $env:BUCKY_VPN_ELEVATED = $savedElevated
        Remove-Item -Path function:Start-Process -Force -ErrorAction SilentlyContinue
        . $installerPath
    }

    # Runner window policy. ARM64 is rejected before any network or UAC work.
    $savedRunnerArchitecture = $env:PROCESSOR_ARCHITEW6432
    try {
        $env:PROCESSOR_ARCHITEW6432 = 'ARM64'
        # In-session execution only ends the script and keeps the shell alive.
        $script:sessionSurvived = $false
        $sessionMessages = @(& $installerPath *>&1)
        $script:sessionSurvived = $true
        Assert ($LASTEXITCODE -eq 1) "In-session execution did not propagate the failure: $LASTEXITCODE"
        Assert ($script:sessionSurvived) 'In-session execution terminated its caller.'
        Remove-BuckyReportedFailureLog $sessionMessages
        Write-Host 'PowerShell in-session runner passed'

        # iex execution must not end the caller's session and must publish LASTEXITCODE.
        $iexLines = @(
            '$env:PROCESSOR_ARCHITEW6432 = ''ARM64''',
            ('Invoke-Expression (Get-Content ''' + $installerPath + ''' -Raw)'),
            'Write-Host ''RUNNER-SURVIVED''',
            'Write-Host (''RUNNER_LASTEXIT='' + $LASTEXITCODE)'
        )
        $iexText = (@($iexLines | & powershell.exe -NoProfile -NoLogo -ExecutionPolicy Bypass 2>&1) | Out-String)
        Assert ($iexText.Contains('RUNNER-SURVIVED')) 'The iex entry terminated its host session.'
        Assert ($iexText.Contains('RUNNER_LASTEXIT=1')) 'The iex entry did not expose the failure through LASTEXITCODE.'
        Remove-BuckyReportedFailureLog $iexText
        Write-Host 'PowerShell iex runner passed'

        # -File with redirected input must not wait for a key and must keep the status.
        $fileWatch = [Diagnostics.Stopwatch]::StartNew()
        $fileMessages = @('' | & powershell.exe -NoProfile -NoLogo -ExecutionPolicy Bypass -File $installerPath 2>&1)
        $fileStatus = $LASTEXITCODE
        $fileWatch.Stop()
        Assert ($fileStatus -eq 1) "The -File entry did not propagate the failure: $fileStatus"
        Assert ($fileWatch.ElapsedMilliseconds -lt 60000) 'The -File entry waited for interactive input.'
        Remove-BuckyReportedFailureLog $fileMessages
        Write-Host "PowerShell -File runner passed ($($fileMessages.Count) messages)"
    }
    finally { $env:PROCESSOR_ARCHITEW6432 = $savedRunnerArchitecture }

    # Failure diagnostics: full InnerException chain, retained report and console output.
    $nativeFailure = [ComponentModel.Win32Exception]::new(10054)
    $innerFailure = [System.IO.IOException]::new('io layer', $nativeFailure)
    $outerFailure = [System.InvalidOperationException]::new('outer layer', $innerFailure)
    $chainText = Get-BuckyExceptionDetail -Exception $outerFailure
    foreach ($expected in @('InvalidOperationException', 'IOException', '10054', 'outer layer', 'io layer')) {
        Assert ($chainText.Contains($expected)) "The exception chain lost $expected"
    }
    $stackedChainText = Get-BuckyExceptionDetail -Exception $outerFailure -IncludeStack
    Assert ($stackedChainText.Length -ge $chainText.Length) 'Stack traces shortened the exception chain.'
    Write-Host 'PowerShell exception chain formatting passed'

    $releaseUrl = 'https://api.github.com/repos/buckyos/bucky-vpn/releases/latest'
    $standaloneReport = $null
    try {
        $standaloneReport = Write-BuckyFailureReport -Exception $outerFailure -RequestUri $releaseUrl -InitialRequestUri $releaseUrl -InstallLog (Join-Path $Work 'install.log')
        Assert ($standaloneReport -and (Test-Path -LiteralPath $standaloneReport)) 'No failure report was written.'
        $reportText = [IO.File]::ReadAllText($standaloneReport)
        foreach ($expected in @($releaseUrl, 'InvalidOperationException', '10054', 'PowerShell', 'Exception chain')) {
            Assert ($reportText.Contains($expected)) "The failure report lost $expected"
        }
    }
    finally {
        if ($standaloneReport -and (Test-Path -LiteralPath $standaloneReport)) {
            Microsoft.PowerShell.Management\Remove-Item -LiteralPath $standaloneReport -Force
        }
    }
    Write-Host 'PowerShell failure report writing passed'

    # The real failure path must print the URL, the chain and the retained report path.
    function Test-BuckyAdministrator { return $true }
    function Save-BuckyHttpsFile {
        param([uri] $Uri, [string] $Destination, [switch] $ShowProgress)
        throw ([System.IO.IOException]::new('transport failed', ([ComponentModel.Win32Exception]::new(10054))))
    }
    $printedReport = $null
    try {
        $failureMessages = @(Invoke-BuckyInstall *>&1)
        $failureOutput = $failureMessages | Out-String -Width 4096
        Assert ($failureOutput.Contains('Request URL: ' + $releaseUrl)) 'The failed request URL was not reported.'
        Assert ($failureOutput.Contains('IOException')) 'The inner transport exception was not reported.'
        Assert ($failureOutput.Contains('Win32Exception')) 'The nested Win32 exception was not reported.'
        Assert ($failureOutput.Contains('Failure details were written to')) 'The failure report path was not reported.'
        $pathMatch = [regex]::Match($failureOutput, 'Failure details were written to (.+?)(\r?\n|$)')
        Assert $pathMatch.Success 'The reported failure report path was not captured.'
        $printedReport = $pathMatch.Groups[1].Value.Trim()
        Assert (Test-Path -LiteralPath $printedReport) 'The reported failure report does not exist.'
        $printedText = [IO.File]::ReadAllText($printedReport)
        Assert ($printedText.Contains($releaseUrl)) 'The failure report lost the request URL.'
        Assert ($printedText.Contains('IOException') -and $printedText.Contains('10054')) 'The failure report lost the exception chain.'
        Assert ($printedText.Contains('Script stack:')) 'The failure report lost the script stack.'
        Assert ((Split-Path -Leaf (Split-Path -Parent $printedReport)) -notmatch '^bucky-vpn-download-') 'The failure report was written into a deleted download directory.'
        Assert ((@($failureMessages | Where-Object { $_ -is [int] })[-1]) -eq 1) 'The failed install did not return 1.'
    }
    finally {
        if ($printedReport -and (Test-Path -LiteralPath $printedReport)) {
            Microsoft.PowerShell.Management\Remove-Item -LiteralPath $printedReport -Force
        }
        Remove-Item -Path function:Start-Process -Force -ErrorAction SilentlyContinue
        . $installerPath
    }
    Write-Host 'PowerShell install failure diagnostics passed'
    exit 0
}

if ($Mode -eq 'integration') {
    $metadata = Join-Path $Work 'live-release.json'
    Save-BuckyHttpsFile -Uri 'https://api.github.com/repos/buckyos/bucky-vpn/releases/latest' -Destination $metadata
    $release = ConvertFrom-Json ([IO.File]::ReadAllText($metadata))
    $records = @()
    foreach ($platform in @('windows', 'linux', 'macos')) {
        $asset = Get-BuckyReleaseAsset $release $platform
        $package = Join-Path $Work $asset.Name
        Save-BuckyHttpsFile -Uri $asset.Url -Destination $package
        $actual = (Get-FileHash -LiteralPath $package -Algorithm SHA256).Hash.ToLowerInvariant()
        Assert ($actual -eq $asset.Sha256) ("Real asset digest mismatch: " + $asset.Name)
        $records += [pscustomobject]@{ name=$asset.Name; sha256=$actual; bytes=(Get-Item $package).Length; url=$asset.Url }
        Write-Host ("Real official asset verified: " + $asset.Name + ' ' + $actual)
    }
    $records | ConvertTo-Json | Set-Content -Encoding UTF8 (Join-Path $Repository '.harness/test-results/release-installers/live-assets.json')
    exit 0
}

Assert ($Mode -eq 'dv') 'Unknown test mode.'
$original = [IO.File]::ReadAllText((Join-Path $Repository 'install.ps1'))
# Replace only OS/admin environment probes in a disposable copy. Network and the
# downloaded executable are controlled below; all selection/hash/status/cleanup logic stays real.
$start = $original.IndexOf('        if ([Environment]::OSVersion.Platform')
$end = $original.IndexOf('        $tempRoot =', $start)
Assert ($start -gt 0 -and $end -gt $start) 'Preflight substitution anchor drifted.'
$copy = $original.Substring(0, $start) + $original.Substring($end)
$copyPath = Join-Path $Work 'isolated-install.ps1'
[IO.File]::WriteAllText($copyPath, $copy)
. $copyPath

# Harmless downloaded EXE: records arguments, writes the requested diagnostic log,
# sleeps to expose waiting bugs, and returns a controlled native status. No install.
$helper = Join-Path $Work 'helper.exe'
Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Threading;
public class InstallerProbe {
    public static int Main(string[] args) {
        File.WriteAllLines(Environment.GetEnvironmentVariable("BUCKY_PROBE_ARGS"), args);
        foreach (string arg in args) {
            if (arg.StartsWith("/LOG=")) File.WriteAllText(arg.Substring(5).Trim('"'), "harmless installer probe");
        }
        Thread.Sleep(350);
        return Int32.Parse(Environment.GetEnvironmentVariable("BUCKY_PROBE_EXIT"));
    }
}
'@ -OutputAssembly $helper -OutputType ConsoleApplication
$helperDigest = (Get-FileHash -LiteralPath $helper -Algorithm SHA256).Hash.ToLowerInvariant()
$script:scenario = ''
$script:downloadDirectory = ''
$script:invoked = $false
function Save-BuckyHttpsFile {
    param([uri] $Uri, [string] $Destination, [switch] $ShowProgress)
    Assert ($ShowProgress.IsPresent -eq (-not $Uri.AbsoluteUri.EndsWith('/releases/latest'))) 'Package progress was not selected at the production call site.'
    $script:downloadDirectory = Split-Path $Destination
    if ($script:scenario -eq 'network') { throw 'injected network failure' }
    if ($Uri.AbsoluteUri.EndsWith('/releases/latest')) {
        $value = ConvertFrom-Json ([IO.File]::ReadAllText((Join-Path $Work 'valid.json')))
        foreach ($asset in $value.assets) { $asset.digest = 'sha256:' + $helperDigest }
        if ($script:scenario -eq 'metadata') { $value.assets = @() }
        if ($script:scenario -eq 'array') { [IO.File]::WriteAllText($Destination, '[' + ($value | ConvertTo-Json -Depth 8 -Compress) + ']'); return }
        [IO.File]::WriteAllText($Destination, ($value | ConvertTo-Json -Depth 8))
    }
    else {
        if ($script:scenario -eq 'mismatch') { [IO.File]::WriteAllText($Destination, 'bad package') }
        else { [IO.File]::Copy($helper, $Destination) }
    }
}
# Real Start-Process retains wait and exit semantics; wrapper observes the boundary.
function Start-Process {
    param($FilePath, $ArgumentList, [switch]$Wait, [switch]$PassThru)
    Assert $Wait.IsPresent 'Production did not request -Wait.'
    Assert $PassThru.IsPresent 'Production did not request native exit status.'
    $script:invoked = $true
    Microsoft.PowerShell.Management\Start-Process -FilePath $FilePath -ArgumentList $ArgumentList -Wait -PassThru
}
function Remove-Item {
    param($LiteralPath, [switch]$Recurse, [switch]$Force, $ErrorAction)
    if ($script:scenario -in @('cleanup', 'failurecleanup')) { throw 'injected cleanup failure' }
    Microsoft.PowerShell.Management\Remove-Item -LiteralPath $LiteralPath -Recurse:$Recurse -Force:$Force -ErrorAction Stop
}

foreach ($scenario in @('success', 'failure', 'reboot', 'mismatch', 'metadata', 'array', 'network', 'cleanup', 'failurecleanup')) {
    $script:scenario = $scenario
    $script:invoked = $false
    $env:BUCKY_PROBE_ARGS = Join-Path $Work ($scenario + '-args.txt')
    $env:BUCKY_PROBE_EXIT = switch ($scenario) { failure {'4'} failurecleanup {'4'} reboot {'3010'} default {'0'} }
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $messages = @(Invoke-BuckyInstall *>&1)
    $watch.Stop()
    # Wide rendering keeps the reported report path on one line for the regex below.
    $output = $messages | Out-String -Width 4096
    $nativeStatus = @($messages | Where-Object { $_ -is [int] })[-1]
    $expected = switch ($scenario) { success {0} failure {4} failurecleanup {4} reboot {3010} default {1} }
    Assert ($nativeStatus -eq $expected) "Unexpected exit status for $scenario : $nativeStatus"
    $shouldLaunch = $scenario -in @('success', 'failure', 'reboot', 'cleanup', 'failurecleanup')
    Assert ($script:invoked -eq $shouldLaunch) "Installer launch ordering violated for $scenario"
    if ($shouldLaunch) {
        Assert ($watch.ElapsedMilliseconds -ge 350) 'Script returned before the native process completed.'
        $arguments = @(Get-Content -LiteralPath $env:BUCKY_PROBE_ARGS)
        foreach ($flag in @('/VERYSILENT','/SUPPRESSMSGBOXES','/SP-','/NORESTART','/RESTARTEXITCODE=3010')) {
            Assert ($arguments -contains $flag) "Missing silent/reboot flag $flag"
        }
        $logArgument = @($arguments | Where-Object { $_.StartsWith('/LOG=') })
        Assert ($logArgument.Count -eq 1) 'Missing unique installation log.'
        $log = $logArgument[0].Substring(5).Trim('"')
        Assert (Test-Path -LiteralPath $log) 'Native log was not retained.'
        Assert ((Split-Path $log) -ne $script:downloadDirectory) 'Log would be deleted with downloads.'
        Microsoft.PowerShell.Management\Remove-Item -LiteralPath $log -Force
    }
    if ($scenario -in @('cleanup', 'failurecleanup')) {
        Assert (-not (($messages | Out-String) -match 'installed successfully')) 'Cleanup failure reported success.'
        Microsoft.PowerShell.Management\Remove-Item -LiteralPath $script:downloadDirectory -Recurse -Force
    }
    else { Assert (-not (Test-Path -LiteralPath $script:downloadDirectory)) "Downloads leaked for $scenario" }
    $reportedReport = [regex]::Match($output, 'Failure details were written to (.+?)(\r?\n|$)')
    $failureReportScenario = $scenario -in @('network', 'metadata', 'array', 'mismatch')
    Assert ($reportedReport.Success -eq $failureReportScenario) "Failure report presence wrong for $scenario"
    if ($reportedReport.Success) {
        $reportedReportPath = $reportedReport.Groups[1].Value.Trim()
        Assert (Test-Path -LiteralPath $reportedReportPath) "The reported failure report is missing for $scenario"
        if ($failureReportScenario) {
            $expectedRequest = switch ($scenario) {
                'mismatch' { 'Request URL: https://github.com/buckyos/bucky-vpn/releases/download/v2.3.4/BuckyVPN_2.3.4_amd64_Setup.exe' }
                default { 'Request URL: https://api.github.com/repos/buckyos/bucky-vpn/releases/latest' }
            }
            Assert ($output.Contains($expectedRequest)) "Unexpected request URL for $scenario"
        }
        Microsoft.PowerShell.Management\Remove-Item -LiteralPath $reportedReportPath -Force
        Assert (-not (Test-Path -LiteralPath $reportedReportPath)) "The failure report leaked for $scenario"
    }
    Write-Host "PowerShell full workflow passed: $scenario ($nativeStatus)"
}
