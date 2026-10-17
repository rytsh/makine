# Build Doom for makine on Windows. Same as build.sh: downloads doomgeneric
# (GPL-2.0) and the shareware DOOM1.WAD, then compiles with the makine SDK.
#   powershell -ExecutionPolicy Bypass -File examples\doom\build.ps1
# Output: build\doom.elf. Needs LLVM (clang, ld.lld) and git on PATH.
$ErrorActionPreference = 'Stop'
$Here = $PSScriptRoot
$B = "$Here/build"
New-Item -ItemType Directory -Force $B | Out-Null

$rev = 'e257a09527062bfb986b6e5e0780839b6cbd5b71'
$wadSha1 = '5b2e249b9c5133ec987b3ea77596381dc0d6bc1d'
$wadUrl = if ($env:WAD_URL) { $env:WAD_URL } else {
    'https://raw.githubusercontent.com/Akbar30Bill/DOOM_wads/master/doom1.wad'
}

if (-not (Test-Path "$B/doomgeneric")) {
    git clone -q https://github.com/ozkl/doomgeneric.git "$B/doomgeneric"
    if ($LASTEXITCODE) { throw 'git clone başarısız' }
    git -C "$B/doomgeneric" checkout -q $rev
}
if (-not (Test-Path "$B/doom1.wad")) {
    Invoke-WebRequest -UseBasicParsing -OutFile "$B/doom1.wad" $wadUrl
}
$hash = (Get-FileHash -Algorithm SHA1 "$B/doom1.wad").Hash.ToLower()
if ($hash -ne $wadSha1) { throw "doom1.wad beklenen dosya değil (SHA-1 $hash)" }

$dg = "$B/doomgeneric/doomgeneric"
$line = (Get-Content "$dg/Makefile" | Where-Object { $_ -like 'SRC_DOOM = *' })
$srcs = $line.Substring(11).Split(' ', [StringSplitOptions]::RemoveEmptyEntries) |
    Where-Object { $_ -notlike '*xlib*' } |
    ForEach-Object { "$dg/" + [IO.Path]::ChangeExtension($_, '.c') }

# The WAD path goes into an assembler string; use forward slashes.
$wad = "$B/doom1.wad" -replace '\\', '/'
$flags = @('-w', "-I$dg", '-DNORMALUNIX', '-DLINUX', '-DDOOMGENERIC_RESX=320',
    '-DDOOMGENERIC_RESY=200', "-DWAD_PATH=`"$wad`"")
& "$Here/../../sdk/build.ps1" -Out "$B/doom.elf" -Flags $flags `
    @srcs "$Here/doomgeneric_makine.c" "$Here/wad.S"
