import asyncio
import ctypes
import json
import logging
import os

logger = logging.getLogger(__name__)

_HERE = os.path.dirname(os.path.abspath(__file__))

try:
    import bapi
    _has_rs = True
except ImportError:
    _has_rs = False

_img = None
try:
    _lib = ctypes.CDLL(os.path.join(_HERE, "libbimg.so"))
    _lib.domcolor.argtypes = [ctypes.c_char_p, ctypes.c_int, ctypes.c_int]
    _lib.domcolor.restype = ctypes.c_uint32
    _lib.phash64.argtypes = [ctypes.c_char_p, ctypes.c_int, ctypes.c_int]
    _lib.phash64.restype = ctypes.c_uint64
    _lib.hammingmatch.argtypes = [ctypes.c_uint64, ctypes.c_void_p, ctypes.c_size_t, ctypes.c_int]
    _lib.hammingmatch.restype = ctypes.c_int
    _lib.keywordscan.argtypes = [ctypes.c_char_p, ctypes.c_char_p]
    _lib.keywordscan.restype = ctypes.c_int
    _img = _lib
except OSError as e:
    logger.debug(f"libbimg not linked: {e}")


def native_available() -> bool:
    return _has_rs


def img_available() -> bool:
    return _img is not None


_PATH_TO_ROUTE = {
    "/search/web": "search/web",
    "/search/images": "search/images",
    "/spotify/token": "spotify/token",
    "/religion/bible": "religion/bible",
    "/religion/bible/random": "religion/bible/random",
    "/religion/quran": "religion/quran",
    "/religion/quran/random": "religion/quran/random",
    "/youtube/search": "youtube/search",
    "/youtube/video": "youtube/video",
    "/youtube/download": "youtube/download",
    "/soundcloud/search": "soundcloud/search",
    "/soundcloud/track": "soundcloud/track",
    "/soundcloud/user": "soundcloud/user",
    "/soundcloud/playlist": "soundcloud/playlist",
    "/soundcloud/usertracks": "soundcloud/usertracks",
    "/soundcloud/related": "soundcloud/related",
    "/soundcloud/download/audio": "soundcloud/download/audio",
    "/tiktok/latest": "tiktok/latest",
    "/reddit/media": "reddit/media",
    "/reddit/download": "reddit/download",
    "/crypto/check": "crypto/check",
    "/utility/dominant-color": "util/dominant-color",
    "/utility/svg": "util/svg",
    "/utility/zip": "util/zip",
    "/fun/caption": "fun/caption",
    "/fun/img2gif": "fun/img2gif",
    "/fun/vid2gif": "fun/vid2gif",
    "/fun/quote": "fun/quote",
    "/fun/quote/fonts": "fun/quote/fonts",
    "/levelling/card": "levelling/card",
    "/levelling/levelup": "levelling/levelup",
    "/levelling/leaderboard": "levelling/leaderboard",
    "/stats/dashboard": "stats/dashboard",
}


def call_sync(route: str, params: dict) -> dict:
    if not _has_rs:
        raise RuntimeError("bapi not built; run native/build.sh")
    out = bapi.call(route, json.dumps(params))
    data = json.loads(out)
    if isinstance(data, dict) and set(data) == {"error"}:
        raise RuntimeError(data["error"])
    return data


async def call(route: str, params: dict) -> dict:
    return await asyncio.to_thread(call_sync, route, params)


def _fmt(params: dict) -> dict:
    return {k: ("true" if v else "false") if isinstance(v, bool) else str(v) for k, v in (params or {}).items()}


import os as _os

async def _http_fallback(path: str, params: dict):
    base = _os.environ.get("BAPI_BASE_URL") or _os.environ.get("SOCIAL_API_BASE_URL") or "http://127.0.0.1:5461"
    base = base.rstrip("/")

    def _do():
        import json as _json
        import urllib.error
        import urllib.parse
        import urllib.request

        qs = urllib.parse.urlencode(_fmt(params or {}))
        url = f"{base}/{path.lstrip('/')}" + (f"?{qs}" if qs else "")
        req = urllib.request.Request(url, headers={"Accept": "application/json"})
        try:
            with urllib.request.urlopen(req, timeout=30) as resp:
                raw = resp.read()
                if not raw:
                    return None
                try:
                    return _json.loads(raw.decode("utf-8", "replace"))
                except Exception:
                    return raw.decode("utf-8", "replace")
        except urllib.error.HTTPError as e:
            if e.code == 404:
                return None
            logger.debug(f"bapi http fallback {path}: HTTP {e.code}")
            return None
        except Exception as e:
            logger.debug(f"bapi http fallback {path} failed: {e}")
            return None

    try:
        return await asyncio.to_thread(_do)
    except Exception as e:
        logger.error(f"bapi client fallback {path} failed: {e}")
        return None


async def api_get(path: str, params: dict):
    route = _PATH_TO_ROUTE.get(path)
    if route and _has_rs:
        try:
            return await call(route, params or {})
        except Exception as e:
            logger.warning(f"native {route} failed, falling back to HTTP: {e}")
    data = await _http_fallback(path, params)
    return data


async def api_get_list(path: str, params: dict):
    data = await api_get(path, params)
    return data if isinstance(data, list) else []


async def download_to_file(path: str, params: dict, dest: str):
    route = _PATH_TO_ROUTE.get(path)
    if route and _has_rs:
        try:
            data = await call(route, params or {})
            src = data.get("path") if isinstance(data, dict) else None
            if src:
                import shutil
                shutil.copyfile(src, dest)
                return dest
            if isinstance(data, dict) and data.get("data_b64"):
                import base64
                with open(dest, "wb") as f:
                    f.write(base64.b64decode(data["data_b64"]))
                return dest
        except Exception as e:
            logger.warning(f"native {route} failed, falling back to HTTP: {e}")
    data = await _http_bytes(path, params)
    if data is None:
        return None
    with open(dest, "wb") as f:
        f.write(data)
    return dest


async def _http_bytes(path: str, params: dict):
    from config import config as bot_config
    from core.utils import get_http_client

    base = bot_config.get("social_api.base_url", "http://127.0.0.1:7861").rstrip("/")
    try:
        resp = await get_http_client().request("GET", f"{base}/{path.lstrip('/')}", params=_fmt(params))
        if resp.status_code != 200:
            return None
        return resp.content
    except Exception as e:
        logger.error(f"bapi client fallback {path} failed: {e}")
        return None


def dominant_color(rgba: bytes, w: int, h: int) -> int:
    if _img is None:
        raise RuntimeError("libbimg not built; run native/build.sh")
    return _img.domcolor(rgba, w, h)


def phash64(rgba: bytes, w: int, h: int) -> int:
    if _img is None:
        raise RuntimeError("libbimg not built; run native/build.sh")
    return _img.phash64(rgba, w, h)


def keyword_hits(text: str, keywords: list) -> int:
    if _img is None:
        raise RuntimeError("libbimg not built; run native/build.sh")
    return _img.keywordscan(text.encode(), "\x1f".join(keywords).encode())
