#!/usr/bin/env bash
# Build Doom for makine. Downloads doomgeneric (GPL-2.0) and the shareware
# DOOM1.WAD (freely redistributable), then compiles them with the makine SDK.
#
# Output: build/doom.elf. In makine: Bilgisayar > Doom (or "Dosyadan yükle").
# Needs: clang, ld.lld, curl, git.
set -euo pipefail
cd "$(dirname "$0")"
HERE=$(pwd)
B=$HERE/build
mkdir -p "$B"

DOOMGENERIC_REV=e257a09527062bfb986b6e5e0780839b6cbd5b71
WAD_SHA1=5b2e249b9c5133ec987b3ea77596381dc0d6bc1d
WAD_URL=${WAD_URL:-https://raw.githubusercontent.com/Akbar30Bill/DOOM_wads/master/doom1.wad}

if [ ! -d "$B/doomgeneric" ]; then
    git clone -q https://github.com/ozkl/doomgeneric.git "$B/doomgeneric"
    git -C "$B/doomgeneric" checkout -q "$DOOMGENERIC_REV"
fi
if [ ! -f "$B/doom1.wad" ]; then
    curl -fL -o "$B/doom1.wad" "$WAD_URL"
fi
echo "$WAD_SHA1  $B/doom1.wad" | sha1sum -c --quiet

DG=$B/doomgeneric/doomgeneric
SRCS=$(sed -n 's/^SRC_DOOM = //p' "$DG/Makefile" | tr ' ' '\n' | grep -v xlib \
    | sed "s|\.o$|.c|; s|^|$DG/|")
../../sdk/build.sh $SRCS "$HERE/doomgeneric_makine.c" "$HERE/wad.S" -o "$B/doom.elf" -- \
    -w -I"$DG" -DNORMALUNIX -DLINUX -DDOOMGENERIC_RESX=320 -DDOOMGENERIC_RESY=200 \
    "-DWAD_PATH=\"$B/doom1.wad\""
