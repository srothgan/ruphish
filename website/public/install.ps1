[CmdletBinding()]
param(
    [string]$Version,
    [string]$InstallDir,
    [switch]$IncludePreviews,
    [switch]$AllowDowngrade,
    [switch]$NoModifyPath,
    [switch]$Uninstall,
    [switch]$Help
)

# Child scope preserves the caller's preferences when invoked through irm | iex.
# Progress presentation inspired by claude-code-rust (Apache-2.0), independently implemented.
& {
    param($Version, $InstallDir, $IncludePreviews, $AllowDowngrade, $NoModifyPath, $Uninstall, $Help)
    Set-StrictMode -Version Latest
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'
    $useColor = -not $env:NO_COLOR -and -not [Console]::IsErrorRedirected
    $live = -not [Console]::IsErrorRedirected -and -not $env:CI -and $env:TERM -ne 'dumb'
    function Write-Step([string]$Text, [ConsoleColor]$Color = 'Cyan') {
        if ($useColor) { Write-Host $Text -ForegroundColor $Color } else { [Console]::Error.WriteLine($Text) }
    }
    function Test-Version([string]$Value) {
        return $Value -cmatch '\A(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-dev\.([1-9]\d*))?\z'
    }
    function Compare-Version([string]$Left, [string]$Right) {
        function Get-Fields([string]$Value) {
            $parts = $Value -split '[-.]'
            [System.Numerics.BigInteger]::Parse($parts[0])
            [System.Numerics.BigInteger]::Parse($parts[1])
            [System.Numerics.BigInteger]::Parse($parts[2])
            if ($parts.Count -eq 3) { [System.Numerics.BigInteger]1; [System.Numerics.BigInteger]0 }
            else { [System.Numerics.BigInteger]0; [System.Numerics.BigInteger]::Parse($parts[4]) }
        }
        $a = @(Get-Fields $Left); $b = @(Get-Fields $Right)
        for ($i = 0; $i -lt 5; $i++) { if ($a[$i] -ne $b[$i]) { return $a[$i].CompareTo($b[$i]) } }
        return 0
    }
    if ($Help) {
        @'
Usage: install.ps1 [options]
  -IncludePreviews        Include dev previews when selecting the latest version.
  -Version VERSION       Install this exact published version (optional leading v).
  -InstallDir DIRECTORY  Managed directory; defaults to %LOCALAPPDATA%\Programs\Ruphish.
  -NoModifyPath          Do not change user PATH.
  -AllowDowngrade        Accept a program-only downgrade, including unattended runs.
  -Uninstall             Remove managed program files; retain all application data.
  -Help                  Show this help.
'@
        return
    }
    if ($Uninstall -and ($Version -or $IncludePreviews -or $AllowDowngrade)) { throw '-Uninstall cannot be combined with version selection or -AllowDowngrade' }
    if ($Version -and $IncludePreviews) { throw 'Choose -Version or -IncludePreviews' }
    if ($Version) { $Version = $Version -creplace '^v', ''; if (-not (Test-Version $Version)) { throw 'Invalid version; use X.Y.Z or X.Y.Z-dev.N' } }
    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) { throw 'Use install.sh on Linux or macOS' }
    if ($PSVersionTable.PSVersion -lt [version]'5.1') { throw 'Windows PowerShell 5.1 or PowerShell 7 is required' }
    Add-Type -AssemblyName System.Net.Http
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    Add-Type -AssemblyName System.Numerics
    $architecture = [Environment]::GetEnvironmentVariable('PROCESSOR_ARCHITECTURE', 'Machine')
    if (-not $architecture) { $architecture = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE } }
    $target = switch ($architecture) {
        'AMD64' { 'x86_64-pc-windows-msvc' }
        'ARM64' { 'aarch64-pc-windows-msvc' }
        default { throw "Unsupported Windows architecture: $architecture" }
    }
    if (-not $InstallDir) { $InstallDir = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Programs\Ruphish' }
    if (-not [IO.Path]::IsPathRooted($InstallDir) -or $InstallDir.StartsWith('\\')) { throw '-InstallDir must be an absolute local path' }
    $root = [IO.Path]::GetFullPath($InstallDir).TrimEnd('\')
    if ($root -match "[\r\n;']" -or $root -eq [IO.Path]::GetPathRoot($root).TrimEnd('\') -or $root -eq $env:USERPROFILE -or $root -eq (Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'ruphish')) { throw 'Choose a dedicated program directory, separate from application data' }
    $current = Join-Path $root 'current'; $previous = Join-Path $root 'previous'; $staging = Join-Path $root 'staging'
    $marker = Join-Path $root '.owner'; $pathRecord = Join-Path $root '.path'
    $utf8 = New-Object Text.UTF8Encoding($false)
    $files = @('ruphish.exe', 'LICENSE', 'THIRD-PARTY-NOTICES.md', 'THIRD-PARTY-NOTICES.txt', 'build.json')
    function Assert-Plain([string]$Path) {
        if ((Get-Item -LiteralPath $Path -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Refusing a symbolic link or junction: $Path" }
    }
    function Assert-Package([string]$Path) {
        Assert-Plain $Path
        foreach ($entry in Get-ChildItem -LiteralPath $Path -Force) {
            Assert-Plain $entry.FullName
            if ($entry.PSIsContainer -or $entry.Name -cnotin ($files + '.version')) { throw "Unmanaged package entry: $($entry.FullName)" }
        }
    }
    function Remove-Package([string]$Path) {
        if (Test-Path -LiteralPath $Path) {
            Assert-Package $Path
            foreach ($entry in Get-ChildItem -LiteralPath $Path -Force) { Remove-Item -LiteralPath $entry.FullName -Force }
            [IO.Directory]::Delete($Path)
        }
    }
    function Get-ProgramVersion([string]$Directory) {
        $start = New-Object Diagnostics.ProcessStartInfo
        $start.FileName = Join-Path $Directory 'ruphish.exe'; $start.Arguments = '--version'
        $start.UseShellExecute = $false; $start.CreateNoWindow = $true
        $start.RedirectStandardOutput = $true; $start.RedirectStandardError = $true
        $process = New-Object Diagnostics.Process
        $process.StartInfo = $start
        try {
            if (-not $process.Start()) { throw 'Unable to start executable' }
            $stdout = $process.StandardOutput.ReadToEndAsync(); $stderr = $process.StandardError.ReadToEndAsync()
            if (-not $process.WaitForExit(15000)) { $process.Kill(); throw 'Executable version check timed out' }
            $output = ($stdout.GetAwaiter().GetResult() + $stderr.GetAwaiter().GetResult()).Trim()
            if ($process.ExitCode -ne 0) { throw "Executable version check failed: $output. Check the Microsoft Visual C++ runtime for $architecture." }
            return $output
        } finally { $process.Dispose() }
    }
    function Show-Download([long]$Bytes, [long]$Total) {
        if (-not $live) { return }
        if ($Total -gt 0) {
            $percent = [int][Math]::Min(100, [Math]::Floor(100.0 * $Bytes / $Total))
            $filled = [int][Math]::Floor($percent / 5)
            $text = ('Downloading [{0}{1}] {2,3}%' -f ('=' * $filled), ('-' * (20 - $filled)), $percent)
        } else { $text = "Downloading $Bytes bytes" }
        if ([Console]::WindowWidth -lt 42) { $text = "Downloading $Bytes bytes" }
        $previousColor = [Console]::ForegroundColor
        try {
            if ($useColor) { [Console]::ForegroundColor = [ConsoleColor]::Cyan }
            [Console]::Error.Write("`r$text   ")
        } finally { if ($useColor) { [Console]::ForegroundColor = $previousColor } }
    }
    $handler = New-Object Net.Http.HttpClientHandler
    $handler.AllowAutoRedirect = $false
    $client = New-Object Net.Http.HttpClient($handler)
    $client.Timeout = [TimeSpan]::FromMinutes(10)
    function Receive-File([string]$Url, [string]$Destination, [bool]$Progress = $false) {
        Write-Step "Downloading $(([uri]$Url).AbsolutePath.Split('/')[-1])"
        for ($attempt = 0; $attempt -lt 3; $attempt++) {
            $response = $null; $stream = $null; $output = $null; $timeout = $null
            try {
                $next = [uri]$Url
                for ($redirect = 0; ; $redirect++) {
                    if ($next.Scheme -ne 'https' -or $next.UserInfo) { throw 'Download redirects must use HTTPS without credentials' }
                    if ($redirect -gt 5) { throw 'Too many download redirects' }
                    $response = $client.GetAsync($next, [Net.Http.HttpCompletionOption]::ResponseHeadersRead).GetAwaiter().GetResult()
                    if ([int]$response.StatusCode -notin @(301, 302, 303, 307, 308)) { break }
                    if (-not $response.Headers.Location) { throw 'Download redirect has no destination' }
                    $next = New-Object Uri($next, $response.Headers.Location)
                    $response.Dispose(); $response = $null
                }
                if (-not $response.IsSuccessStatusCode) {
                    $detail = if ($Url.StartsWith('https://www.ruphish.com/api/releases')) { $response.Content.ReadAsStringAsync().GetAwaiter().GetResult() } else { '' }
                    throw "HTTP $([int]$response.StatusCode): $detail"
                }
                $total = $response.Content.Headers.ContentLength
                $stream = $response.Content.ReadAsStreamAsync().GetAwaiter().GetResult()
                $output = [IO.File]::Open($Destination, [IO.FileMode]::Create, [IO.FileAccess]::Write, [IO.FileShare]::Read)
                $buffer = New-Object byte[] 65536; [long]$received = 0
                $watch = [Diagnostics.Stopwatch]::StartNew(); $lastDraw = -200
                while ($true) {
                    $timeout = New-Object Threading.CancellationTokenSource(30000)
                    $read = $stream.ReadAsync($buffer, 0, $buffer.Length, $timeout.Token).GetAwaiter().GetResult()
                    $timeout.Dispose(); $timeout = $null
                    if ($read -eq 0) { break }
                    $received += $read
                    if ($received -gt 1GB -or $watch.Elapsed.TotalMinutes -gt 10) { throw 'Download exceeded the size or time limit' }
                    $output.Write($buffer, 0, $read)
                    if ($Progress -and $watch.ElapsedMilliseconds - $lastDraw -ge 200) { Show-Download $received $(if ($null -eq $total) { 0 } else { $total }); $lastDraw = $watch.ElapsedMilliseconds }
                }
                if ($null -ne $total -and $received -ne $total) { throw 'Truncated download' }
                return
            } catch {
                if ($attempt -eq 2 -or $_.Exception.Message -match 'HTTP 4\d\d') { throw "Download failed: $Url. $($_.Exception.Message)" }
            } finally {
                if ($timeout) { $timeout.Dispose() }
                if ($output) { $output.Dispose() }
                if ($stream) { $stream.Dispose() }
                if ($response) { $response.Dispose() }
                if ($live -and $Progress) { [Console]::Error.Write("`r$(' ' * 50)`r") }
            }
        }
    }
    $lock = $null; $temporary = $null
    try {
        [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($root)) | Out-Null
        # Exclusive OS handle: a crash releases it without stale-PID guesses.
        $lockPath = "$root.lock"
        if (Test-Path -LiteralPath $lockPath) { Assert-Plain $lockPath }
        try { $lock = [IO.File]::Open($lockPath, [IO.FileMode]::OpenOrCreate, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None) }
        catch { throw 'Another installer is running, or the installation lock is inaccessible' }
        if (Test-Path -LiteralPath $root) {
            Assert-Plain $root
            if (-not (Test-Path -LiteralPath $marker -PathType Leaf)) { throw "Unmanaged installation directory: $root" }
            Assert-Plain $marker
            if ([IO.File]::ReadAllText($marker).Trim() -cne 'ruphish-installer-v1') { throw "Unmanaged installation directory: $root" }
            foreach ($entry in Get-ChildItem -LiteralPath $root -Force) {
                Assert-Plain $entry.FullName
                if ($entry.Name -cnotin @('.owner', '.path', 'current', 'previous', 'staging')) { throw "Unmanaged installation entry: $($entry.FullName)" }
            }
        } elseif ($Uninstall) { Write-Step 'No managed installation found'; return }
        else { [IO.Directory]::CreateDirectory($root) | Out-Null; [IO.File]::WriteAllText($marker, "ruphish-installer-v1`n", $utf8) }
        if (-not (Test-Path -LiteralPath $current) -and (Test-Path -LiteralPath $previous)) {
            Assert-Package $previous; [IO.Directory]::Move($previous, $current)
            Write-Step 'WARNING: Recovered program files from an interrupted replacement' Yellow
        }
        Remove-Package $staging
        $installed = ''
        if (Test-Path -LiteralPath $current) {
            Assert-Package $current
            $installed = [IO.File]::ReadAllText((Join-Path $current '.version')).Trim()
            if (-not (Test-Version $installed)) { throw 'Installed version record is invalid' }
        }
        Remove-Package $previous
        if ($Uninstall) {
            if (Test-Path -LiteralPath $pathRecord) {
                if (-not $NoModifyPath) {
                    if ([IO.File]::ReadAllText($pathRecord) -cne $current) { throw 'Unrecognized managed PATH record' }
                    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
                    $remaining = @($userPath -split ';' | Where-Object { $_.TrimEnd('\') -ine $current })
                    [Environment]::SetEnvironmentVariable('Path', ($remaining -join ';'), 'User')
                } else { Write-Step 'WARNING: User PATH left unchanged by -NoModifyPath' Yellow }
                Remove-Item -LiteralPath $pathRecord
            }
            Remove-Package $current
            Remove-Item -LiteralPath $marker
            [IO.Directory]::Delete($root)
            Write-Step 'Removed Ruphish program files. Configuration, databases, certificates, and logs were retained.' Green
            return
        }
        $temporary = Join-Path ([IO.Path]::GetTempPath()) ("ruphish-install-" + [guid]::NewGuid().ToString('N'))
        [IO.Directory]::CreateDirectory($temporary) | Out-Null
        if (-not $Version) {
            Write-Step 'Resolving latest published release'
            $selection = Join-Path $temporary 'version'
            Receive-File "https://www.ruphish.com/api/releases?format=text&includePreviews=$(([bool]$IncludePreviews).ToString().ToLowerInvariant())" $selection
            $Version = [IO.File]::ReadAllText($selection).TrimEnd("`r", "`n")
            if (-not (Test-Version $Version)) { throw 'The release selector returned an invalid version' }
            if (-not $IncludePreviews -and $Version.Contains('-')) { throw 'The full-release selector returned a preview' }
        }
        Write-Step "Selected Ruphish $Version ($target)"
        if ($Version.Contains('-')) { Write-Step 'WARNING: Preview release: use disposable data; upgrades are not guaranteed' Yellow }
        if ($installed -and (Compare-Version $Version $installed) -lt 0) {
            Write-Step "WARNING: DOWNGRADE: $installed -> $Version. Only program files will change. Database migrations and configuration changes are NOT reversed." Yellow
            if (-not $AllowDowngrade) {
                if (-not $env:CI -and [Environment]::UserInteractive -and -not [Console]::IsInputRedirected -and -not [Console]::IsErrorRedirected) {
                    $answer = Read-Host 'Stop is the default. Proceed with this downgrade? [y/N]'
                    if ($answer -notmatch '\A(y|yes)\z') { throw 'Downgrade cancelled; existing installation retained' }
                } else { throw 'Downgrade requires confirmation. For an intentional unattended downgrade use -AllowDowngrade' }
            }
        }
        if ($Version -ne $installed) {
            $name = "ruphish-$Version-$target"; $archiveName = "$name.zip"
            $base = "https://github.com/srothgan/ruphish/releases/download/v$Version"
            $archive = Join-Path $temporary 'archive.zip'; $sums = Join-Path $temporary 'SHA256SUMS'
            Receive-File "$base/$archiveName" $archive $true
            Receive-File "$base/SHA256SUMS" $sums
            Write-Step 'Verifying archive checksum'
            $matching = @([IO.File]::ReadAllLines($sums) | Where-Object { ($_ -split '\s+')[-1] -ceq $archiveName })
            if ($matching.Count -ne 1 -or $matching[0] -cnotmatch ('\A([0-9a-fA-F]{64})[ \t]+' + [regex]::Escape($archiveName) + '\z')) { throw 'Expected exactly one valid checksum for the archive' }
            $expected = $Matches[1]
            if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ine $expected) { throw 'Archive checksum mismatch; existing installation retained' }
            $zip = [IO.Compression.ZipFile]::OpenRead($archive)
            try {
                $seen = New-Object 'System.Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
                [long]$expanded = 0
                foreach ($entry in $zip.Entries) {
                    if (-not $seen.Add($entry.FullName)) { throw 'Duplicate archive path' }
                    $kind = ($entry.ExternalAttributes -shr 16) -band 0xF000
                    if ($entry.FullName -ceq "$name/" -and $kind -in @(0, 0x4000)) { continue }
                    if ($entry.FullName -cnotin @($files | ForEach-Object { "$name/$_" }) -or $kind -notin @(0, 0x8000)) { throw 'Unexpected archive paths, links, or contents' }
                    $expanded += $entry.Length
                    if ($expanded -gt 1GB) { throw 'Expanded archive exceeds the size limit' }
                }
                foreach ($file in $files) { if (-not $seen.Contains("$name/$file")) { throw "Archive is missing $file" } }
                [IO.Directory]::CreateDirectory($staging) | Out-Null
                foreach ($file in $files) { [IO.Compression.ZipFileExtensions]::ExtractToFile($zip.GetEntry("$name/$file"), (Join-Path $staging $file), $false) }
            } finally { $zip.Dispose() }
            Write-Step 'Verifying staged executable'
            $reported = Get-ProgramVersion $staging
            if ($reported -cne "Ruphish $Version") { throw "Unexpected executable version: $reported" }
            [IO.File]::WriteAllText((Join-Path $staging '.version'), "$Version`n", $utf8)
            if (Test-Path -LiteralPath $current) {
                try {
                    $probe = [IO.File]::Open((Join-Path $current 'ruphish.exe'), [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::None)
                    $probe.Dispose()
                } catch { throw 'The installed executable is locked. Stop Ruphish and retry; the current installation was retained.' }
                [IO.Directory]::Move($current, $previous)
            }
            try { [IO.Directory]::Move($staging, $current) }
            catch {
                if (Test-Path -LiteralPath $previous) { [IO.Directory]::Move($previous, $current) }
                throw 'Program replacement failed; close Ruphish and retry'
            }
            Write-Step "Installed Ruphish $Version" Green
            Remove-Package $previous
        } else {
            if ((Get-ProgramVersion $current) -cne "Ruphish $Version") { throw 'Installed executable and version record disagree' }
            Write-Step "Ruphish $Version is already installed" Green
        }
        $executable = Join-Path $current 'ruphish.exe'
        Write-Step "Program files are ready at $executable"
        if (-not $NoModifyPath) {
            $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
            $entries = @($userPath -split ';')
            if (-not @($entries | Where-Object { $_.TrimEnd('\') -ieq $current }).Count) {
                # Write ownership first, so a later run can recover an interrupted PATH update.
                [IO.File]::WriteAllText($pathRecord, $current, $utf8)
                $updatedPath = if ([string]::IsNullOrEmpty($userPath)) { $current } else { "$userPath;$current" }
                [Environment]::SetEnvironmentVariable('Path', $updatedPath, 'User')
            }
        }
        $existing = Get-Command ruphish -ErrorAction SilentlyContinue
        if ($existing -and $existing.Source -ine $executable) { Write-Step "WARNING: Another ruphish is first on PATH: $($existing.Source)" Yellow }
        Write-Step "`nStart now: & '$executable'`nIn a new shell with PATH configured: ruphish"
        Write-Step 'The installer has not started the server. Ruphish creates its own configuration and data on first launch.'
    } finally {
        if ($lock) { $lock.Dispose() }
        # The lock file is intentionally retained: deleting it after releasing the handle races other installers.
        if ($temporary -and [IO.Path]::GetDirectoryName($temporary) -eq [IO.Path]::GetTempPath().TrimEnd('\')) {
            Remove-Item -LiteralPath $temporary -Recurse -Force
        }
        $client.Dispose()
    }
} @PSBoundParameters
