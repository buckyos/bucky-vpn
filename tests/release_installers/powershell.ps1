param([string] $Mode, [string] $Repository, [string] $Work)
$ErrorActionPreference = 'Stop'
. (Join-Path $Repository 'install.ps1')
function Assert($Condition, [string] $Message) { if (-not $Condition) { throw $Message } }

if ($Mode -eq 'unit') {
    # Exercise the original platform guard, with network and process execution denied.
    $savedArchitecture = $env:PROCESSOR_ARCHITEW6432
    $script:guardDownloaded = $false
    function Save-BuckyHttpsFile { $script:guardDownloaded = $true; throw 'unexpected download during architecture denial' }
    try {
        $env:PROCESSOR_ARCHITEW6432 = 'ARM64'
        $guardMessages = @(Invoke-BuckyInstall *>&1)
        Assert (@($guardMessages | Where-Object { $_ -is [int] })[-1] -eq 1) 'ARM64 was not rejected.'
        Assert (-not $script:guardDownloaded) 'Unsupported native architecture reached network.'
    }
    finally { $env:PROCESSOR_ARCHITEW6432 = $savedArchitecture }
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
    Write-Host "PowerShell full workflow passed: $scenario ($nativeStatus)"
}
