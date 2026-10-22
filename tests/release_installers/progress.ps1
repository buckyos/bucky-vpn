param([string] $Mode, [string] $Repository, [string] $Work)
$ErrorActionPreference = 'Stop'
function Assert($Condition, [string] $Message) { if (-not $Condition) { throw $Message } }
Add-Type -AssemblyName System.Net.Http
# A real HttpClient reads these controlled response streams. No network, installer,
# certificate override or production test hook is used.
Add-Type -ReferencedAssemblies System.Net.Http -TypeDefinition @'
using System;
using System.IO;
using System.Net;
using System.Net.Http;
using System.Threading;
using System.Threading.Tasks;
public class BuckyProgressHandler : HttpMessageHandler {
    public static string Scenario;
    public static int Received;
    public static bool StreamDisposed;
    public static bool ReadHasDeadline;
    public static bool HeaderHasDeadline;
    public static int Requests;
    protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken token) {
        Requests++;
        HeaderHasDeadline = token.CanBeCanceled;
        var response = new HttpResponseMessage(HttpStatusCode.OK);
        if (Scenario == "http-error") response.StatusCode = HttpStatusCode.NotFound;
        if (Scenario == "downgrade") {
            response.StatusCode = HttpStatusCode.Redirect;
            response.Headers.Location = new Uri("http://example.invalid/package");
        }
        if (Scenario == "redirect-loop") {
            response.StatusCode = HttpStatusCode.Redirect;
            response.Headers.Location = new Uri("https://example.invalid/again");
        }
        response.Content = new StreamContent(new BuckyProgressStream());
        if (Scenario != "unknown" && Scenario != "fast-unknown")
            response.Content.Headers.ContentLength = Scenario == "empty" ? 0 : Scenario == "incomplete" ? 32769 : 32768;
        return Task.FromResult(response);
    }
}
public class BuckyProgressStream : Stream {
    public override bool CanRead { get { return true; } }
    public override bool CanSeek { get { return false; } }
    public override bool CanWrite { get { return false; } }
    public override long Length { get { throw new NotSupportedException(); } }
    public override long Position { get { return BuckyProgressHandler.Received; } set { throw new NotSupportedException(); } }
    public override void Flush() { }
    public override long Seek(long offset, SeekOrigin origin) { throw new NotSupportedException(); }
    public override void SetLength(long value) { throw new NotSupportedException(); }
    public override void Write(byte[] buffer, int offset, int count) { throw new NotSupportedException(); }
    public override int Read(byte[] buffer, int offset, int count) { throw new NotSupportedException(); }
    public override Task<int> ReadAsync(byte[] buffer, int offset, int count, CancellationToken token) {
        BuckyProgressHandler.ReadHasDeadline = token.CanBeCanceled;
        return Task.Factory.StartNew(() => {
            string scenario = BuckyProgressHandler.Scenario;
            if (scenario == "stall") Thread.Sleep(2000); // Deliberately ignore cancellation.
            else if (scenario != "fast-unknown" && scenario != "empty") Thread.Sleep(120);
            if (scenario == "failure" && BuckyProgressHandler.Received >= 16384) throw new IOException("injected body failure");
            if (scenario == "empty" || BuckyProgressHandler.Received >= 32768) return 0;
            int size = Math.Min(count, 8192);
            for (int i = 0; i < size; i++) buffer[offset + i] = (byte)((BuckyProgressHandler.Received + i) % 251);
            BuckyProgressHandler.Received += size;
            return size;
        });
    }
    protected override void Dispose(bool disposing) { BuckyProgressHandler.StreamDisposed = true; base.Dispose(disposing); }
}
'@
$original = [IO.File]::ReadAllText((Join-Path $Repository 'install.ps1'))
$anchor = "    `$handler = New-Object Net.Http.HttpClientHandler`r`n    `$handler.AllowAutoRedirect = `$false"
if (-not $original.Contains($anchor)) { $anchor = $anchor.Replace("`r`n", "`n") }
Assert ($original.Contains($anchor)) 'Controlled transport anchor drifted.'
$source = $original.Replace($anchor, '    $handler = New-Object BuckyProgressHandler')
$copyPath = Join-Path $Work 'controlled-progress.ps1'
[IO.File]::WriteAllText($copyPath, $source)
. $copyPath
function Write-Progress {
    param($Id, $Activity, $Status, $PercentComplete = -1, [switch] $Completed)
    [void]$script:events.Add([pscustomobject]@{
        Percent = $PercentComplete; Status = $Status; Completed = $Completed.IsPresent
        Received = [BuckyProgressHandler]::Received
    })
}
foreach ($scenario in @('known', 'unknown', 'fast-unknown', 'empty', 'failure', 'incomplete', 'http-error', 'downgrade', 'redirect-loop', 'metadata', 'stall')) {
    [BuckyProgressHandler]::Scenario = $scenario
    [BuckyProgressHandler]::Received = 0
    [BuckyProgressHandler]::StreamDisposed = $false
    [BuckyProgressHandler]::ReadHasDeadline = $false
    [BuckyProgressHandler]::Requests = 0
    $script:events = New-Object 'Collections.Generic.List[object]'
    $destination = Join-Path $Work ($scenario + '.bin')
    if ($scenario -eq 'stall') {
        # Keep production's one 15-minute deadline; shorten it only in this disposable copy.
        Assert ($source.Contains('$cancel.CancelAfter([TimeSpan]::FromMinutes(15))')) 'Deadline anchor drifted.'
        [IO.File]::WriteAllText($copyPath, $source.Replace('$cancel.CancelAfter([TimeSpan]::FromMinutes(15))', '$cancel.CancelAfter([TimeSpan]::FromMilliseconds(150))'))
        . $copyPath
    }
    $failed = $false
    $watch = [Diagnostics.Stopwatch]::StartNew()
    try { Save-BuckyHttpsFile -Uri 'https://example.invalid/package' -Destination $destination -ShowProgress:($scenario -ne 'metadata') }
    catch { $failed = $true }
    $watch.Stop()
    $expectedFailure = $scenario -in @('failure', 'incomplete', 'http-error', 'downgrade', 'redirect-loop', 'stall')
    Assert ($failed -eq $expectedFailure) "Unexpected transfer outcome: $scenario"
    Assert ([BuckyProgressHandler]::HeaderHasDeadline) "Headers lost cancellation: $scenario"
    Assert ([BuckyProgressHandler]::StreamDisposed) "Response stream leaked: $scenario"
    if ($scenario -eq 'metadata') { Assert ($events.Count -eq 0) 'Metadata unexpectedly displayed progress.' }
    else {
        Assert ($events.Count -gt 0 -and $events[$events.Count - 1].Completed) "Progress not cleared: $scenario"
        Assert (@($events | Where-Object { $_.Completed }).Count -eq 1) "Progress cleared more than once: $scenario"
    }
    if ($scenario -in @('known', 'unknown', 'failure')) {
        Assert (@($events | Where-Object { -not $_.Completed -and $_.Received -gt 0 -and $_.Received -lt 32768 }).Count -gt 0) "No live intermediate progress: $scenario"
    }
    if ($scenario -eq 'known') {
        Assert (@($events | Where-Object { $_.Percent -gt 0 -and $_.Percent -lt 100 }).Count -gt 0) 'Known size never showed intermediate percent.'
        Assert ($events[$events.Count - 2].Percent -eq 100) 'Known size did not reach 100 percent.'
    }
    if ($scenario -in @('unknown', 'fast-unknown')) {
        Assert (@($events | Where-Object { -not $_.Completed -and $_.Percent -ne -1 }).Count -eq 0) 'Unknown size fabricated percent.'
        Assert ($events[$events.Count - 2].Status -eq '32768 bytes downloaded') 'Unknown size lost final byte feedback.'
    }
    if (-not $expectedFailure) {
        $bytes = [IO.File]::ReadAllBytes($destination)
        Assert ($bytes.Length -eq $(if ($scenario -eq 'empty') { 0 } else { 32768 })) "Wrong downloaded size: $scenario"
        for ($i = 0; $i -lt $bytes.Length; $i++) { Assert ($bytes[$i] -eq ($i % 251)) 'Download content changed.' }
    }
    if ($scenario -eq 'stall') {
        Assert ($watch.ElapsedMilliseconds -lt 1500) 'Blocked body read escaped whole-download deadline.'
        Assert ([BuckyProgressHandler]::ReadHasDeadline) 'Body reads lost cancellation token.'
    }
    if ($scenario -eq 'downgrade') { Assert ([BuckyProgressHandler]::Requests -eq 1) 'Followed an HTTP redirect.' }
    if ($scenario -eq 'redirect-loop') { Assert ([BuckyProgressHandler]::Requests -eq 11) 'Redirect limit changed.' }
    Write-Host "PowerShell controlled progress passed: $scenario ($($watch.ElapsedMilliseconds) ms)"
}
