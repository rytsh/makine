# Compile C programs for makine (RV32IM) with clang and ld.lld. Windows
# version of build.sh; see there for details.
#   sdk\build.ps1 prog.c [more.c ...] [-Out out.elf] [-Flags '-DFOO','-O3']
# Needs LLVM (clang, ld.lld) on PATH: winget install LLVM.LLVM
param(
    [Parameter(Mandatory, Position = 0, ValueFromRemainingArguments)] [string[]] $Sources,
    [string] $Out,
    [string[]] $Flags = @()
)
$ErrorActionPreference = 'Stop'
$Sdk = $PSScriptRoot
$srcs = $Sources
$extra = $Flags
$out = if ($Out) { $Out } else { [IO.Path]::ChangeExtension($srcs[0], '.elf') }

function Need($tool) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        throw "$tool bulunamadı. LLVM'i kur (winget install LLVM.LLVM) ve PATH'e ekle."
    }
}
Need clang
Need ld.lld

$clangInc = Join-Path (& clang -print-resource-dir).Trim() 'include'
$cflags = @('--target=riscv32-unknown-elf', '-march=rv32im', '-mabi=ilp32', '-O2',
    '-ffreestanding', '-fno-builtin', '-nostdinc', '-isystem', "$Sdk/include", '-isystem', $clangInc)

function Compile($src, $obj, [string[]] $more) {
    & clang @cflags @more -c $src -o $obj
    if ($LASTEXITCODE) { throw "derlenemedi: $src" }
}

# Runtime: libc, startup and compiler-rt builtins, built once into rt\.
$rt = "$Sdk/rt"
$llvmTag = 'llvmorg-18.1.3'
$crt = 'adddf3 addsf3 divdf3 divsf3 divdi3 moddi3 udivdi3 umoddi3 udivmoddi4 extendsfdf2 fixdfdi fixdfsi fixsfsi fixunsdfdi fixunsdfsi floatdidf floatsidf floatsisf floatundidf floatunsidf comparedf2 comparesf2 muldf3 mulsf3 subdf3 subsf3 truncdfsf2 fp_mode'.Split(' ')
$crtH = 'fp_lib.h fp_add_impl.inc fp_div_impl.inc fp_mul_impl.inc fp_extend.h fp_extend_impl.inc fp_trunc.h fp_trunc_impl.inc fp_fixint_impl.inc fp_fixuint_impl.inc fp_compare_impl.inc int_lib.h int_types.h int_endianness.h int_util.h int_div_impl.inc fp_mode.h int_math.h'.Split(' ')
New-Item -ItemType Directory -Force "$rt/src", "$rt/obj" | Out-Null
foreach ($f in $crtH + ($crt | ForEach-Object { "$_.c" })) {
    if (-not (Test-Path "$rt/src/$f")) {
        Invoke-WebRequest -UseBasicParsing -OutFile "$rt/src/$f" `
            "https://raw.githubusercontent.com/llvm/llvm-project/$llvmTag/compiler-rt/lib/builtins/$f"
    }
}
$rtObjs = @()
function Rt($src, $obj) {
    if (-not (Test-Path $obj) -or (Get-Item $src).LastWriteTime -gt (Get-Item $obj).LastWriteTime) {
        Compile $src $obj @('-w')
    }
    $script:rtObjs += $obj
}
foreach ($c in $crt) { Rt "$rt/src/$c.c" "$rt/obj/$c.o" }
Rt "$Sdk/src/libc.c" "$rt/obj/libc.o"
Rt "$Sdk/src/start.S" "$rt/obj/start.o"

$tmp = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid())
New-Item -ItemType Directory $tmp | Out-Null
try {
    $objs = @()
    foreach ($s in $srcs) {
        $o = Join-Path $tmp ((Split-Path $s -Leaf) + '.o')
        Compile $s $o $extra
        $objs += $o
    }
    & ld.lld -m elf32lriscv -T "$Sdk/link.ld" --gc-sections @rtObjs @objs -o $out
    if ($LASTEXITCODE) { throw 'bağlanamadı (ld.lld)' }
    Write-Output $out
} finally {
    Remove-Item -Recurse -Force $tmp
}
