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
    # A JDK **9 or newer**, not a JRE and not a Java 8: everything that has to run
    # in the target is cross-compiled with `--release`, which javac only learned in
    # 9, and the attach helper needs the `jdk.attach` module when the target is not
    # a Java 8.
    #
    # Each candidate is ASKED its version rather than judged by its directory name.
    # The name sort this replaced ranked `jdk-8.0.504.1-hotspot` above
    # `jdk-21.0.12.101-hotspot` — "8" sorts after "2" — so on a machine with both it
    # picked the one JDK here that cannot compile anything, and only an already-set
    # JAVA_HOME hid that.
    $roots = @('C:\Program Files\Eclipse Adoptium', 'C:\Program Files\Java', (Join-Path $env:USERPROFILE '.jdks'))
    $candidates = @($env:JAVA_HOME) + (Get-ChildItem $roots -Directory -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName })
    $best = $null
    foreach ($candidate in $candidates) {
        if (-not $candidate) { continue }
        $javac = Join-Path $candidate 'bin\javac.exe'
        if (-not (Test-Path $javac)) { continue }
        # Java 8 reports `javac 1.8.0_504`, so the leading number is 1 and it is
        # rejected by the same test that accepts `javac 21.0.12.1`.
        #
        # Run in a child scope with the default error preference, because Java 8
        # prints its version to *stderr* (9+ moved it to stdout) and this script's
        # `Stop` preference turns any native stderr output into a terminating
        # error — which would kill the probe on precisely the JVM it exists to
        # reject.
        $reported = & { $ErrorActionPreference = 'Continue'; (& $javac -version 2>&1) | Out-String }
        if ($reported -notmatch 'javac\s+(\d+)') { continue }
        $major = [int]$Matches[1]
        if ($major -lt 9) { continue }
        if (-not $best -or $major -gt $best.Major) { $best = @{ Home = $candidate; Major = $major } }
    }
    if (-not $best) { throw 'no JDK 9+ found (javac must understand --release); set JAVA_HOME to one' }
    Write-Host "JDK:   $($best.Home) (javac $($best.Major))"
    return $best.Home
}

function Stop-Ows {
    # Both JVMs: the launcher, and the application JVM out of the OWS JVM cache.
    Get-CimInstance Win32_Process -Filter "Name='java.exe' OR Name='javaw.exe' OR Name='javaws.exe'" |
        Where-Object { $_.CommandLine -like '*OpenWebStart*' -or $_.CommandLine -like '*jvm-cache*' } |
        ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
}

if (-not (Test-Path $ows)) { throw "OpenWebStart not found at $ows — see README.md" }
$jdk = Find-Jdk
Write-Host "agent: $Jar"

# ---------------------------------------------------------------- build & serve
# Everything that has to *run in the target* is compiled for Java 8. The host
# JDK is whatever is installed; the target JVM is the one the JNLP asked for, and
# a class file it cannot read fails as `AgentLoadException: ... 102`, which reads
# like an attach problem and is not one.
$targetRelease = @('--release', '8')
New-Item -ItemType Directory -Force -Path $site, (Join-Path $work 'classes'), (Join-Path $work 'probe') | Out-Null
& "$jdk\bin\javac.exe" @targetRelease -d (Join-Path $work 'classes') (Join-Path $here 'demo\WebStartDemo.java')
if ($LASTEXITCODE -ne 0) { throw "cannot compile the demo for Java 8 with the javac in $jdk — " +
    'a JDK that has dropped `--release 8` (it is deprecated since 21) cannot host this harness; ' +
    'point JAVA_HOME at an older one' }
& "$jdk\bin\jar.exe" cfe (Join-Path $site 'webstart-demo.jar') platynui.demo.WebStartDemo -C (Join-Path $work 'classes') .
Copy-Item -Force (Join-Path $here 'demo\demo.jnlp') (Join-Path $site 'demo.jnlp')

if ($Probe) {
    & "$jdk\bin\javac.exe" @targetRelease -d (Join-Path $work 'probe') (Join-Path $here 'probe\ProbeAgent.java')
    if ($LASTEXITCODE -ne 0) { throw "cannot compile the probe agent for Java 8 with the javac in $jdk" }
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
    $targetIsJava8 = Test-Path $toolsJar
    $attachJava = if ($targetIsJava8) { Join-Path $targetHome 'bin\java.exe' } else { Join-Path $jdk 'bin\java.exe' }
    $attachCp = if ($targetIsJava8) { "$(Join-Path $work 'probe');$toolsJar" } else { Join-Path $work 'probe' }
    Write-Host "application JVM pid = $appPid ($targetHome)"
    Start-Sleep 8

    # ------------------------------------------------------------------- attach
    # Compiled for whichever runtime will RUN it, which is not always the host JDK.
    # `com.sun.tools.attach` sits in `tools.jar` on Java 8 but in the `jdk.attach`
    # MODULE from 9 on — and a module is not part of the release-8 platform API, so
    # `--release 8` against it fails with "package com.sun.tools.attach does not
    # exist". Hence release-8-plus-tools.jar for a Java 8 target, and the host JDK's
    # own platform otherwise, which is also the runtime that then executes it.
    if ($targetIsJava8) {
        & "$jdk\bin\javac.exe" @targetRelease -cp $toolsJar -d (Join-Path $work 'probe') (Join-Path $here 'probe\Attach.java')
    } else {
        & "$jdk\bin\javac.exe" -d (Join-Path $work 'probe') (Join-Path $here 'probe\Attach.java')
    }
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
