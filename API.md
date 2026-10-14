# Bot API — HTTP Reference

Run `./serve` (or use the in-process Python client). Default base URL:

```
http://127.0.0.1:5461
```

All responses are JSON. Errors are returned as:

```json
{ "error": "human readable message" }
```

## Quick start

Health check:

```bash
curl http://127.0.0.1:5461/health
```

Example (search):

```bash
curl -X POST http://127.0.0.1:5461/search/web \
  -H "Content-Type: application/json" \
  -d '{"q":"rust axum","count":5}'
```

Python:

```python
import aiohttp, asyncio

async def main():
    async with aiohttp.ClientSession() as s:
        async with s.post("http://127.0.0.1:5461/search/web",
                          json={"q": "rust axum", "count": 5}) as r:
            print(await r.json())

asyncio.run(main())
```

Node:

```js
const res = await fetch("http://127.0.0.1:5461/search/web", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify({ q: "rust axum", count: 5 })
});
console.log(await res.json());
```

## Authentication

Only `POST /crypto/check` is protected. Set `BAPI_CRYPTO_TOKEN` (or `crypto_token` in
`config.json`) and send:

```
Authorization: Bearer <token>
```

Other routes are open; put the server behind a reverse proxy / firewall if you
expose it publicly. The built-in health endpoint is unauthenticated.

## In-process (Python)

No HTTP server required:

```python
import bapi
print(bapi.routes())
out = bapi.call("search/web", '{"q":"x","count":5}')
```

Or via the bundled client (falls back to HTTP automatically):

```python
from python.client import api_get, api_get_list
data = await api_get("/search/web", {"q": "x", "count": 5})
```

## Endpoint list

GET (query string params):

| Route | Purpose |
| --- | --- |
| /health | Liveness |
| /spotify/token | Spotify access token |
| /religion/bible | Bible lookup |
| /religion/bible/random | Random verse |
| /religion/quran | Quran lookup |
| /religion/quran/random | Random ayah |
| /youtube/search | YouTube search |
| /youtube/video | Video metadata |
| /youtube/download | Download media (file response) |
| /soundcloud/search | SoundCloud search |
| /soundcloud/track | Track details |
| /soundcloud/user | User profile |
| /soundcloud/playlist | Playlist details |
| /soundcloud/usertracks | User tracks |
| /soundcloud/related | Related tracks |
| /soundcloud/download/audio | Audio download (file response) |
| /tiktok/latest | Latest TikToks for a user |
| /reddit/media | Reddit media |
| /reddit/download | Reddit download (file response) |
| /utility/dominant-color | Dominant color of an image |
| /fun/quote/fonts | Available quote fonts |

POST (JSON body):

| Route | Purpose |
| --- | --- |
| /search/web | Web search |
| /search/images | Image search |
| /crypto/check | Crypto address check (auth required) |
| /utility/svg | SVG render (image response) |
| /utility/zip | Zip utility (image response) |
| /fun/caption | Image caption |
| /fun/img2gif | Image to GIF (image response) |
| /fun/vid2gif | Video to GIF |
| /fun/quote | Quote image |
| /levelling/card | Level card (image response) |
| /levelling/levelup | Level up image |
| /levelling/leaderboard | Leaderboard image |
| /stats/dashboard | Stats dashboard image |

## Conventions

- Boolean/numeric query params are coerced (`"true"`, `"false"`, numbers).
- File-producing routes return binary bodies unless `catbox=true` is
  passed, in which case JSON with a URL is returned.
- Binary image routes return the rendered image directly.
- Long-running routes have per-route timeouts (25s–320s).

## Configuration

See `config.example.json`. Override the config path with `BAPI_CONFIG`.
Secrets (`crypto_token`, upload keys) should never be committed.

## License / usage

This API is provided as-is for bot and automation use. If you expose it
publicly, add rate limiting and authentication at your reverse proxy.