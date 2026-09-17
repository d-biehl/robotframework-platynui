<#
.SYNOPSIS
    Launches the demo application through OpenWebStart and attaches a PlatynUI
    agent JAR into it, reporting whether the agent came up and what it sees.

.DESCRIPTION
    The manual counterpart to the automated Web Start coverage: this one uses a
    real OpenWebStart, so it is what tells us whether the model the tests use
    still matches reality. See README.md for the one-time setup.

.PARAMETER Jar
    Agent JAR to attach. Defaults to the one `just build-java-agent` produces.

.PARAMETER Probe
    Attach the AppContext diagnostic agent instead, and dump the JVM's contexts
    and their windows.

.PARAMETER KeepRunning
    Leave the application up after reporting, for poking at it by hand.
#>
[CmdletBinding()]
param(
    [string]$Jar,
    [switch]$Probe,
    [switch]$KeepRunning
)

$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$repo = Resolve-Path (Join-Path $here '..\..')
$work = Join-Path $env:TEMP 'platynui-webstart-repro'
$site = Join-Path $work 'site'
$ows = Join-Path $env:LOCALAPPDATA 'Programs\OpenWebStart\javaws.exe'
if (-not $Jar) { $Jar = Join-Path $repo 'java\agent\build\libs\platynui-agent.jar' }

function Find-Jdk {
    # A JDK, not a JRE: the demo needs javac/jar, and the attach helper needs
    # lib\tools.jar (Java 8) or the jdk.attach module (9+).
    foreach ($candidate in @($env:JAVA_HOME) + (Get-ChildItem 'C:\Program Files\Eclipse Adoptium', 'C:\Program Files\Java' -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending | ForEach-Object { $_.FullName })) {
        if ($candidate -and (Test-Path (Join-Path $candidate 'bin\javac.exe'))) { return $candidate }
    }
    throw 'no JDK found; set JAVA_HOME'
}

function Stop-Ows {
    # Both JVMs: the launcher, and the application JVM out of the OWS JVM cache.
    Get-CimInstance Win32_Process -Filter "Name='java.exe' OR Name='javaw.exe' OR Name='javaws.exe'" |
        Where-Object { $_.CommandLine -like '*OpenWebStart*' -or $_.CommandLine -like '*jvm-cache*' } |
        ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
}

if (-not (Test-Path $ows)) { throw "OpenWebStart not found at $ows — see README.md" }
$jdk = Find-Jdk
Write-Host "JDK:   $jdk"
Write-Host "agent: $Jar"

# ---------------------------------------------------------------- build & serve
# Everything that has to *run in the target* is compiled for Java 8. The host
# JDK is whatever is installed; the target JVM is the one the JNLP asked for, and
# a class file it cannot read fails as `AgentLoadException: ... 102`, which reads
# like an attach problem and is not one.
$targetRelease = @('--release', '8')
New-Item -ItemType Directory -Force -Path $site, (Join-Path $work 'classes'), (Join-Path $work 'probe') | Out-Null
& "$jdk\bin\javac.exe" @targetRelease -d (Join-Path $work 'classes') (Join-Path $here 'demo\WebStartDemo.java')
if ($LASTEXITCODE -ne 0) { throw 'cannot compile the demo for Java 8' }
& "$jdk\bin\jar.exe" cfe (Join-Path $site 'webstart-demo.jar') platynui.demo.WebStartDemo -C (Join-Path $work 'classes') .
Copy-Item -Force (Join-Path $here 'demo\demo.jnlp') (Join-Path $site 'demo.jnlp')

if ($Probe) {
    & "$jdk\bin\javac.exe" @targetRelease -d (Join-Path $work 'probe') (Join-Path $here 'probe\ProbeAgent.java')
    if ($LASTEXITCODE -ne 0) { throw 'cannot compile the probe agent for Java 8' }
    @"
Manifest-Version: 1.0
Agent-Class: platynui.probe.ProbeAgent
Boot-Class-Path: probe-agent.jar

"@ | Set-Content -Encoding ASCII (Join-Path $work 'probe.mf')
    Push-Location (Join-Path $work 'probe')
    & "$jdk\bin\jar.exe" cfm (Join-Path $work 'probe-agent.jar') (Join-Path $work 'probe.mf') platynui
    Pop-Location
    $Jar = Join-Path $work 'probe-agent.jar'
}
# `Attach.java` is compiled after the target is up, against the target's own
# runtime — see the attach step for why.

# ITW caches by URL, so a rebuilt JAR behind the same URL is ignored unless the
# cache goes. Silence here would be the worst kind of bug in a harness: the run
# would quietly test the *previous* demo build and report it as the current one.
Stop-Ows
$itwCache = Join-Path $env:USERPROFILE '.cache\icedtea-web\cache'
if (Test-Path $itwCache) {
    Remove-Item -Recurse -Force $itwCache -ErrorAction SilentlyContinue
    if (Test-Path $itwCache) { throw "cannot clear the IcedTea-Web cache at $itwCache — a stale demo JAR would be served" }
}
$server = Start-Process -FilePath 'py' -ArgumentList @('-3', '-m', 'http.server', '8099', '--directory', $site) -PassThru -WindowStyle Hidden
Start-Sleep 2

try {
    # ------------------------------------------------------------------- launch
    $log = Join-Path $work 'ows.log'
    Start-Process -FilePath $ows -ArgumentList @('http://localhost:8099/demo.jnlp') `
        -RedirectStandardOutput $log -RedirectStandardError "$log.err" | Out-Null

    $app = $null
    for ($i = 0; $i -lt 40 -and -not $app; $i++) {
        Start-Sleep 2
        $app = Get-CimInstance Win32_Process -Filter "Name='java.exe'" |
            Where-Object { $_.CommandLine -like '*jvm-cache*' } | Select-Object -First 1
    }
    if (-not $app) {
        Write-Host 'the application JVM never came up:'
        Get-Content "$log.err" -Tail 20 -ErrorAction SilentlyContinue
        return
    }
    $appPid = $app.ProcessId
    # OpenWebStart caches whole JDKs, so the runtime the application is running in
    # can do the attaching itself. That is not just convenient: an attach reply
    # comes in two dialects — Java 8 answers a bare integer, JDK 9+ answers
    # "return code: N" — and a newer JDK's attach client trips over the older
    # target's reply (`AgentLoadException: Failed to load agent library: 0`) even
    # when the agent loaded perfectly well. PlatynUI's own transport parses both;
    # `com.sun.tools.attach` does not, so match the versions instead.
    $targetHome = Split-Path -Parent (Split-Path -Parent $app.ExecutablePath)
    $toolsJar = Join-Path $targetHome 'lib\tools.jar'
    $attachJava = if (Test-Path $toolsJar) { Join-Path $targetHome 'bin\java.exe' } else { Join-Path $jdk 'bin\java.exe' }
    $attachCp = if (Test-Path $toolsJar) { "$(Join-Path $work 'probe');$toolsJar" } else { Join-Path $work 'probe' }
    Write-Host "application JVM pid = $appPid ($targetHome)"
    Start-Sleep 8

    # ------------------------------------------------------------------- attach
    & "$jdk\bin\javac.exe" @targetRelease -cp $(if (Test-Path $toolsJar) { $toolsJar } else { '.' }) `
        -d (Join-Path $work 'probe') (Join-Path $here 'probe\Attach.java')
    if ($LASTEXITCODE -ne 0) { throw 'cannot compile the attach helper' }

    $probeOut = Join-Path $work 'probe-out.txt'
    if (Test-Path $probeOut) { Clear-Content $probeOut }
    & $attachJava -cp $attachCp Attach $appPid $Jar $probeOut
    Start-Sleep 3

    # ------------------------------------------------------------------- report
    if ($Probe) {
        Write-Host "`n=== AppContexts as an agent thread sees them ==="
        Get-Content $probeOut -ErrorAction SilentlyContinue
    } else {
        $handshake = Join-Path $env:LOCALAPPDATA "PlatynUI\agents\agent-$appPid"
        if (Test-Path $handshake) {
            Write-Host "`n=== HANDSHAKE PRESENT ==="
            Get-Content $handshake
            & py -3 (Join-Path $here 'probe_rpc.py') $handshake ((Get-Content $handshake | ConvertFrom-Json).agentVersion)
        } else {
            Write-Host "`n=== HANDSHAKE ABSENT — the agent died inside the target ==="
        }
    }

    Write-Host "`n=== what the run denied, and what the agent said ==="
    Select-String -Path $log -Pattern 'Denying permission|\[PlatynUI agent\]|\[demo\]' -ErrorAction SilentlyContinue |
        ForEach-Object { $_.Line.Substring(0, [Math]::Min(200, $_.Line.Length)) }

    if ($KeepRunning) { Write-Host "`napplication left running (pid $appPid)"; return }
} finally {
    Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue
    if (-not $KeepRunning) { Stop-Ows }
}
