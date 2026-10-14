#!/bin/bash
set -e
ROOT="$(cd "$(dirname "$0")" && pwd)"
ARCH="$(uname -m)"
export BAPI_TESSDATA="${BAPI_TESSDATA:-/usr/share/tesseract-ocr/5/tessdata}"
export BAPI_FONTS="${BAPI_FONTS:-$ROOT/fonts}"
SYS=0
if command -v pkg-config > /dev/null && pkg-config --exists vips-cpp tesseract libavformat libavcodec libavutil libswscale libswresample 2>/dev/null; then
  SYS=1
fi
if [ "$SYS" = "0" ] && [ -d "$ROOT/thirdparty/usr/include/vips" ]; then
  if [ "$ARCH" = "x86_64" ]; then
    export LD_LIBRARY_PATH="$ROOT/thirdparty/usr/lib/x86_64-linux-gnu:${LD_LIBRARY_PATH:-}"
    export PKG_CONFIG_PATH="$ROOT/thirdparty/usr/lib/x86_64-linux-gnu/pkgconfig:${PKG_CONFIG_PATH:-}"
    if [ -d "$ROOT/thirdparty/usr/share/tesseract-ocr" ]; then
      export BAPI_TESSDATA="$ROOT/thirdparty/usr/share/tesseract-ocr/5/tessdata"
    fi
  fi
fi
if ! command -v pkg-config > /dev/null || ! pkg-config --exists vips-cpp tesseract libavformat libavcodec libavutil libswscale libswresample 2>/dev/null; then
  if [ ! -f "$ROOT/thirdparty/usr/include/vips/VImage8.h" ] || [ "$ARCH" != "x86_64" ]; then
    DISTRO="$(grep -oP '^ID=\K.*' /etc/os-release 2>/dev/null || echo unknown)"
    echo "no system vips/tesseract and no usable thirdparty dir."
    case "$DISTRO" in
      ubuntu|debian|raspbian|linuxmint|pop)
        echo "run: apt install libtesseract-dev libleptonica-dev tesseract-ocr-eng libvips-dev libavformat-dev libavcodec-dev libavutil-dev libswscale-dev libswresample-dev ffmpeg yt-dlp" ;;
      fedora|rhel|centos|rocky|almalinux)
        echo "run: dnf install tesseract-devel leptonica-devel tesseract-tessdata-eng libvips-devel libavformat-devel libavcodec-devel libavutil-devel libswscale-devel libswresample-devel ffmpeg yt-dlp" ;;
      arch|manjaro|endeavouros|cachyos)
        echo "run: pacman -S tesseract leptonica tesseract-data-eng libvips ffmpeg yt-dlp" ;;
      alpine)
        echo "run: apk add tesseract-ocr tesseract-ocr-data-eng leptonica-dev vips-dev ffmpeg-dev yt-dlp" ;;
      opensuse*|sles)
        echo "run: zypper install tesseract-ocr-devel libvips-devel libavformat-devel libavcodec-devel libavutil-devel libswscale-devel libswresample-devel ffmpeg yt-dlp" ;;
      *)
        echo "install: tesseract (+eng data), leptonica, libvips dev libs, ffmpeg dev libs (libavformat/libavcodec/libavutil/libswscale/libswresample), yt-dlp" ;;
    esac
    echo "or: ./thirdparty/fetch.sh (debian x86_64 only)"
    exit 1
  fi
fi
if [ -z "$IMAGEIO_FFMPEG_EXE" ]; then
  for c in "$ROOT/../.venv/lib/python3.13/site-packages/imageio_ffmpeg/binaries/ffmpeg-"* /usr/bin/ffmpeg; do
    [ -x "$c" ] && [ -f "$c" ] && export IMAGEIO_FFMPEG_EXE="$c" && break
  done
fi
if ! command -v uv > /dev/null 2>&1; then
  if command -v curl > /dev/null 2>&1; then
    echo "== uv not found; installing via astral.sh =="
    curl -LsSf https://astral.sh/uv/install.sh | sh 2>/dev/null || true
    export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
  elif command -v pip3 > /dev/null 2>&1; then
    python3 -m pip install --user --quiet uv 2>/dev/null || true
  fi
fi
if command -v uv > /dev/null 2>&1; then
  uv run --with ddgs python -c "import ddgs" >/dev/null 2>&1 || true
else
  for py in python3 python; do
    if command -v $py > /dev/null 2>&1; then
      $py -c "import ddgs" >/dev/null 2>&1 || $py -m pip install --user --quiet ddgs 2>/dev/null || true
      break
    fi
  done
fi

echo "== arch: $ARCH =="
echo "== C++ =="
cmake -S "$ROOT/cpp" -B "$ROOT/cpp/build" -DCMAKE_BUILD_TYPE=Release > /dev/null
cmake --build "$ROOT/cpp/build" -j
"$ROOT/cpp/build/bimg_selfcheck" 2>/dev/null || "$ROOT/cpp/build/bimgcheck"

echo "== Rust (lib + serve) =="
export BAPI_LIB="$ROOT/cpp/build"
cargo build --release --manifest-path "$ROOT/crates/bapi/Cargo.toml" \
  --target-dir "$ROOT/target" 2>&1 | tail -1
cargo test --release --manifest-path "$ROOT/crates/bapi/Cargo.toml" \
  --target-dir "$ROOT/target" 2>&1 | grep -E "test result" | head -1

echo "== link =="
SUFFIX=$(python3 -c 'import sysconfig;print(sysconfig.get_config_var("EXT_SUFFIX"))')
cp "$ROOT/target/release/libbapi.so" "$ROOT/python/bapi$SUFFIX"
cp "$ROOT/cpp/build/libbimg.so" "$ROOT/python/"
cp "$ROOT/target/release/serve" "$ROOT/serve"
cd "$ROOT/python" && python3 -c "import bapi; print('routes:', len(bapi.routes()))"
python3 -c "
import sys; sys.path.insert(0, '$ROOT/python')
import test_bridge as t
n = 0
for f in [v for k, v in vars(t).items() if k.startswith('test_')]:
    f(); n += 1
print(f'checks: {n} passed')
"
echo "BUILD OK"
