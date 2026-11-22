#!/usr/bin/env bash
# Builds the trimmed FFmpeg shared by the Rimelight apps (Rimelight Media and Rimelight Space) for Windows x64: LGPL,
# shared DLLs and MSVC import libraries, cross-compiled on Linux with MinGW-w64. Only what the apps play, probe, trim
# and record is enabled (tools/ffmpeg/options.txt), plus dav1d for AV1. Run by .github/workflows/ffmpeg.yml; output:
# dist/<NAME>.zip. The pin the apps fetch (tools/ffmpeg/pin.txt) names the zip this script makes.
#
# Needs: mingw-w64, nasm, meson, ninja, pkg-config, curl, zip.
set -euo pipefail

FFMPEG_TAG="n9.0.2"     # same FFmpeg release as the vendored ffmpeg-sys-next bindings
DAV1D_TAG="1.5.5"
REVISION="r1"           # bump when options.txt or this script changes
NAME="rimelight-ffmpeg-${FFMPEG_TAG}-${REVISION}-win64-lgpl-shared"

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$PWD"
WORK="${WORK:-$PWD/ffmpeg-work}"
DEPS="$WORK/deps"
OUT="$WORK/$NAME"
CROSS=x86_64-w64-mingw32
mkdir -p "$WORK" "$DEPS" "$OUT" "$ROOT/dist"
cd "$WORK"

# --- dav1d (AV1 decoding), static, folded into avcodec ---------------------------------------------------------
cat > cross.txt <<EOF
[binaries]
c = '${CROSS}-gcc'
ar = '${CROSS}-ar'
strip = '${CROSS}-strip'
windres = '${CROSS}-windres'
pkg-config = 'pkg-config'
[host_machine]
system = 'windows'
cpu_family = 'x86_64'
cpu = 'x86_64'
endian = 'little'
EOF
if [ ! -d dav1d ]; then
  curl -fsSL "https://code.videolan.org/videolan/dav1d/-/archive/${DAV1D_TAG}/dav1d-${DAV1D_TAG}.tar.gz" | tar xz
  mv "dav1d-${DAV1D_TAG}" dav1d
fi
rm -rf dav1d/build
meson setup dav1d/build dav1d --cross-file cross.txt --prefix="$DEPS" --libdir=lib --buildtype=release \
  --default-library=static -Denable_tools=false -Denable_tests=false -Denable_examples=false
ninja -C dav1d/build install

# --- FFmpeg -------------------------------------------------------------------------------------------------------
if [ ! -d ffmpeg ]; then
  curl -fsSL "https://github.com/FFmpeg/FFmpeg/archive/refs/tags/${FFMPEG_TAG}.tar.gz" | tar xz
  mv "FFmpeg-${FFMPEG_TAG}" ffmpeg
fi
cd ffmpeg
mapfile -t OPTIONS < <(grep -vE '^\s*(#|$)' "$HERE/options.txt")
PKG_CONFIG_PATH="$DEPS/lib/pkgconfig" ./configure \
  --prefix="$OUT" \
  --target-os=mingw32 --arch=x86_64 --cross-prefix="${CROSS}-" \
  --pkg-config=pkg-config --pkg-config-flags=--static \
  --enable-shared --disable-static \
  --extra-ldflags="-static-libgcc -static" \
  "${OPTIONS[@]}"
make -j"$(nproc)"
make install

# MSVC-style import libraries (avcodec.lib ...) for rust-lld, from the .def files the build wrote.
for def in "$OUT"/lib/*.def "$OUT"/bin/*.def lib*/*.def; do
  [ -f "$def" ] || continue
  base="$(basename "$def" .def)"             # e.g. avcodec-63
  short="${base%-*}"                          # e.g. avcodec
  "${CROSS}-dlltool" -m i386:x86-64 -d "$def" -l "$OUT/lib/${short}.lib" -D "${base}.dll"
done
rm -f "$OUT"/lib/*.dll.a "$OUT"/lib/*.def "$OUT"/bin/*.def
rm -rf "$OUT/share" "$OUT/lib/pkgconfig"

# Every DLL must load on a plain Windows: only system DLLs and each other.
for dll in "$OUT"/bin/*.dll; do
  echo "== $(basename "$dll")"
  "${CROSS}-objdump" -p "$dll" | grep "DLL Name" | sed 's/^\s*/   /'
  if "${CROSS}-objdump" -p "$dll" | grep "DLL Name" | grep -qiE "libgcc|libwinpthread|libstdc|libdav1d"; then
    echo "error: $(basename "$dll") needs a MinGW runtime DLL" >&2
    exit 1
  fi
done

# Licences and how it was built (LGPL: the source and the exact configuration).
cp COPYING.LGPLv2.1 "$OUT/LICENSE.txt"
cp "$WORK/dav1d/COPYING" "$OUT/dav1d-LICENSE.txt"
{
  echo "FFmpeg ${FFMPEG_TAG} (https://github.com/FFmpeg/FFmpeg/tree/${FFMPEG_TAG}), dav1d ${DAV1D_TAG}"
  echo "Built by Rimelight Media's tools/ffmpeg/build.sh (${REVISION}) with MinGW-w64."
  echo
  echo "configure options:"
  printf '  %s\n' "${OPTIONS[@]}"
} > "$OUT/BUILD-INFO.txt"
echo "$NAME" > "$OUT/PINNED_RELEASE.txt"

cd "$WORK"
du -sh "$OUT"/bin/*.dll
rm -f "$ROOT/dist/$NAME.zip"
zip -qr "$ROOT/dist/$NAME.zip" "$NAME"
sha256sum "$ROOT/dist/$NAME.zip"
