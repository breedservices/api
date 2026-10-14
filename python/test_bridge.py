import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", ".."))

from native.python import client as bn


def test_route_table_covers_all_api_routes():
    for p in ["/youtube/search", "/youtube/video", "/youtube/download",
              "/soundcloud/search", "/soundcloud/track", "/soundcloud/user",
              "/soundcloud/playlist", "/soundcloud/usertracks", "/soundcloud/related",
              "/soundcloud/download/audio", "/tiktok/latest",
              "/reddit/media", "/reddit/download", "/crypto/check",
              "/search/web", "/search/images", "/spotify/token",
              "/religion/bible", "/religion/bible/random",
              "/religion/quran", "/religion/quran/random",
              "/utility/dominant-color", "/utility/svg", "/utility/zip",
              "/fun/caption", "/fun/img2gif", "/fun/vid2gif",
              "/fun/quote", "/fun/quote/fonts",
              "/levelling/card", "/levelling/levelup", "/levelling/leaderboard",
              "/stats/dashboard"]:
        assert p in bn._PATH_TO_ROUTE, p


def test_native_presence_flags_are_bool():
    assert isinstance(bn.native_available(), bool)
    assert isinstance(bn.img_available(), bool)


def test_param_formatting_parity():
    assert bn._fmt({"a": True, "b": 5}) == {"a": "true", "b": "5"}
