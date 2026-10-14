<h1 align="center">Bot API — Rust + C++ Service</h1>

<p align="center">
  <strong>High-performance media/content API for bots, automations, and apps.</strong><br>
  Run it as a local HTTP service or call it in-process from Python.
</p>

<p align="center">
  <img alt="rust" src="https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white">
  <img alt="cpp" src="https://img.shields.io/badge/C%2B%2B-17-00599C?logo=cplusplus&logoColor=white">
  <img alt="rust" src="https://img.shields.io/badge/Rust-2024-000000?logo=rust&logoColor=white">
  <img alt="uv" src="https://img.shields.io/badge/uv-package%20manager-DE5FE9?logo=uv&logoColor=white">
  <img alt="license" src="https://img.shields.io/badge/license-MIT-green">
</p>

<details>
<summary><b>What it does</b></summary>

- Web, image, and media search
- YouTube, SoundCloud, TikTok, Reddit helpers
- Bible and Quran lookups
- Image rendering (cards, banners, SVG, GIF, ZIP)
- Spotify, crypto, and utility endpoints
- In-process Python FFI bindings

</details>

---

## 1. Prerequisites

<table>
<tr><th>OS</th><th>Install command</th></tr>
<tr><td>Debian / Ubuntu / Raspbian / Mint / Pop</td><td><code>apt install libtesseract-dev libleptonica-dev tesseract-ocr-eng libvips-dev ffmpeg yt-dlp</code></td></tr>
<tr><td>Fedora / RHEL / Rocky / Alma</td><td><code>dnf install tesseract-devel leptonica-devel tesseract-tessdata-eng libvips-devel ffmpeg yt-dlp</code></td></tr>
<tr><td>Arch / Manjaro / Endeavour / Cachy</td><td><code>pacman -S tesseract leptonica tesseract-data-eng libvips ffmpeg yt-dlp</code></td></tr>
<tr><td>Alpine</td><td><code>apk add tesseract-ocr tesseract-ocr-data-eng leptonica-dev vips-dev ffmpeg yt-dlp</code></td></tr>
<tr><td>openSUSE / SLES</td><td><code>zypper install tesseract-ocr-devel libvips-devel ffmpeg yt-dlp</code></td></tr>
</table>

## 2. Configuration

```bash
cp config.example.json config.json
```

Edit `config.json` for port, upload keys, and media extractor overrides.

Override the config path with `BAPI_CONFIG`. Keep secrets in `config.json`
(gitignored).

## 3. Environment variables

| Variable | Default | Purpose |
| --- | --- | --- |
| `BAPI_PORT` | `5461` | HTTP port |
| `BAPI_CRYPTO_TOKEN` | — | Protected crypto endpoint |
| `BAPI_BASE_URL` | `http://127.0.0.1:5461` | Client HTTP base |
| `BAPI_CONFIG` | `./config.json` | Config file path |
| `BAPI_FONTS` | `./fonts` | Font directory |
| `BAPI_TESSDATA` | system | Tesseract data |
| `IMAGEIO_FFMPEG_EXE` | auto | ffmpeg binary |

## 4. API reference

See **[API.md](./API.md)** for endpoint tables, auth, examples, and clients.

<details>
<summary><b>Quick example</b></summary>

```bash
curl -X POST http://127.0.0.1:5461/search/web \
  -H "Content-Type: application/json" \
  -d '{"q":"rust axum","count":5}'
```

</details>

## 5. Build

    ./build.sh

Outputs `./serve`, `python/bapi.*.so`, and `python/libbimg.so`.

## 6. Run

**HTTP service:**

    ./serve

**In-process (Python):**

    import bapi
    bapi.routes()
    bapi.call("search/web", '{"q":"rust","count":5}')

**Client with automatic HTTP fallback:**

    from python.client import api_get
    data = await api_get("/search/web", {"q": "rust", "count": 5})

If `bapi` is on disk but Python can't find it, add `python/` to `$PYTHONPATH`
(not `$PATH`):

    # current shell
    export PYTHONPATH="/home/Klaus/projects/ts-rewrite/native/python:$PYTHONPATH"

    # persist for your shell
    echo 'export PYTHONPATH="/home/Klaus/projects/ts-rewrite/native/python:$PYTHONPATH"' >> ~/.bashrc
    source ~/.bashrc

    # system-wide (Debian/Ubuntu Python 3.13)
    echo "$PWD/python" | sudo tee /usr/local/lib/python3.13/dist-packages/native-api.pth

Verify:

    python3 -c "import bapi; print(len(bapi.routes()))"

## 7. Smoke test

    python3 -c "import bapi; print('routes:', len(bapi.routes()))"

## 8. Troubleshooting

- Missing vips/tesseract: install packages or run the thirdparty fetcher.
- `bapi not built`: run `./build.sh`.
- Import failures: ensure `python/` is on `sys.path`.

---

<p align="center">
  <sub>Built with Rust · C++17 ·</sub>
</p>