#!/usr/bin/env bash
# Compile C programs for makine (RV32IM) with clang and ld.lld.
#   sdk/build.sh prog.c [more.c ...] [-o out.elf] [-- extra clang flags]
# Programs start at address 0 with 16 MB of RAM; see include/makine.h for
# the devices. MK_RAM=32K sdk/build.sh ... links for less RAM (an FPGA).
#
# The first run downloads the soft-float and 64-bit division routines from
# LLVM compiler-rt (Apache-2.0 WITH LLVM-exception) into sdk/rt and builds
# them. Needs clang, ld.lld and curl.
set -euo pipefail
SDK=$(cd "$(dirname "$0")" && pwd)
OUT=
SRCS=()
EXTRA=()
while [ $# -gt 0 ]; do
    case $1 in
        -o) OUT=$2; shift 2 ;;
        --) shift; EXTRA=("$@"); break ;;
        *) SRCS+=("$1"); shift ;;
    esac
done
[ ${#SRCS[@]} -gt 0 ] || { echo "usage: $0 prog.c [-o out.elf] [-- clang flags]" >&2; exit 1; }
OUT=${OUT:-${SRCS[0]%.*}.elf}

CFLAGS="--target=riscv32-unknown-elf -march=rv32im -mabi=ilp32 -O2 -ffreestanding -fno-builtin
        -nostdinc -isystem $SDK/include -isystem $(clang -print-resource-dir)/include"

# Runtime: libc, startup and compiler-rt builtins, built once into rt/.
RT=$SDK/rt
LLVM_TAG=llvmorg-18.1.3
CRT="adddf3 addsf3 divdf3 divsf3 divdi3 moddi3 udivdi3 umoddi3 udivmoddi4 extendsfdf2
     fixdfdi fixdfsi fixsfsi fixunsdfdi fixunsdfsi floatdidf floatsidf floatsisf floatundidf
     floatunsidf comparedf2 comparesf2 muldf3 mulsf3 subdf3 subsf3 truncdfsf2 fp_mode"
CRT_H="fp_lib.h fp_add_impl.inc fp_div_impl.inc fp_mul_impl.inc fp_extend.h fp_extend_impl.inc
       fp_trunc.h fp_trunc_impl.inc fp_fixint_impl.inc fp_fixuint_impl.inc fp_compare_impl.inc
       int_lib.h int_types.h int_endianness.h int_util.h int_div_impl.inc fp_mode.h int_math.h"
mkdir -p "$RT/src" "$RT/obj"
for f in $CRT_H $(for c in $CRT; do echo "$c.c"; done); do
    [ -f "$RT/src/$f" ] || curl -sfL -o "$RT/src/$f" \
        "https://raw.githubusercontent.com/llvm/llvm-project/$LLVM_TAG/compiler-rt/lib/builtins/$f"
done
RT_OBJS=()
rt() { # source object
    if [ ! -f "$2" ] || [ "$1" -nt "$2" ]; then clang $CFLAGS -w -c "$1" -o "$2"; fi
    RT_OBJS+=("$2")
}
for c in $CRT; do rt "$RT/src/$c.c" "$RT/obj/$c.o"; done
rt "$SDK/src/libc.c" "$RT/obj/libc.o"
rt "$SDK/src/start.S" "$RT/obj/start.o"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
OBJS=()
for s in "${SRCS[@]}"; do
    o=$TMP/$(basename "$s").o
    clang $CFLAGS ${EXTRA[@]+"${EXTRA[@]}"} -c "$s" -o "$o"
    OBJS+=("$o")
done
RAM=${MK_RAM:-16M}
ld.lld -m elf32lriscv --defsym=__ram_size=$(echo "$RAM" | sed "s/K$/*1024/; s/M$/*1048576/") -T "$SDK/link.ld" --gc-sections "${RT_OBJS[@]}" "${OBJS[@]}" -o "$OUT"
echo "$OUT"
