pub mod config;
mod crypto;
mod extract;
mod ffi;
mod net;
mod render;
mod web;

use serde_json::{json, Value};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

async fn download(p: &Value, prefix: &str, audio: bool) -> Value {
    let v = extract::download_video(p, &dl_prefix(prefix), audio).await;
    if p.get("catbox").and_then(|v| v.as_bool()).unwrap_or(false) {
        let path = match v.get("path").and_then(|x| x.as_str()) {
            Some(s) => s.to_string(),
            None => return v,
        };
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => return json!({"error": e.to_string()}),
        };
        let name = std::path::Path::new(&path).file_name().and_then(|s| s.to_str()).unwrap_or("file").to_string();
        let _ = std::fs::remove_file(&path);
        use base64::Engine;
        let uh = p
            .get("userhash")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .or_else(|| {
                let c = config::get("catbox_userhash");
                if c.is_empty() {
                    None
                } else {
                    Some(c)
                }
            });
        return web::catbox(&name, &base64::engine::general_purpose::STANDARD.encode(&bytes), uh.as_deref()).await;
    }
    v
}

fn normalize(p: &mut Value) {
    let bools = ["catbox", "shorts", "flip", "blur", "grayscale"];
    let ints = ["count", "max_results", "chapter", "surah", "ayah", "page", "max_pages", "xp", "level", "rank", "progress_xp", "needed_xp", "total_messages", "old_level", "new_level"];
    if let Some(m) = p.as_object_mut() {
        for k in bools {
            if let Some(Value::String(s)) = m.get(k).cloned() {
                m.insert(k.to_string(), Value::Bool(s == "true" || s == "1"));
            }
        }
        for k in ints {
            if let Some(Value::String(s)) = m.get(k).cloned() {
                if let Ok(n) = s.parse::<i64>() {
                    m.insert(k.to_string(), json!(n));
                }
            }
        }
    }
}

pub async fn dispatch(route: &str, p: &Value) -> Value {
    let mut p = p.clone();
    normalize(&mut p);
    let s = |k: &str| p.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let n = |k: &str, d: u64| p.get(k).and_then(|v| v.as_u64()).unwrap_or(d);
    match route {
        "search/web" => web::ddg_text(&s("query"), n("max_results", 3) as usize).await,
        "search/images" => web::ddg_images(&s("query"), n("max_results", 10) as usize).await,
        "spotify/token" => web::spotify_token().await,
        "religion/bible" => {
            web::bible(&s("book"), p.get("chapter").and_then(|v| v.as_i64()).unwrap_or(1), &s("verse"), &s("version").ware_or("kjv")).await
        }
        "religion/bible/random" => web::bible_random(&s("version").ware_or("kjv")).await,
        "religion/quran" => {
            web::quran(p.get("surah").and_then(|v| v.as_i64()).unwrap_or(1), p.get("ayah").and_then(|v| v.as_i64()).unwrap_or(1)).await
        }
        "religion/quran/random" => web::quran_random().await,
        "youtube/search" => extract::youtube_search(&p).await,
        "youtube/video" => extract::youtube_video(&p).await,
        "youtube/download" => download(&p, "yt", false).await,
        "soundcloud/search" => extract::soundcloud_search(&p).await,
        "soundcloud/track" => {
            let v = extract::sc_extract(&s("url"), &p, false).await;
            if v.is_null() { json!({"error": "track not found"}) } else { extract::norm_sc_track(&v) }
        }
        "soundcloud/user" => {
            let v = extract::sc_extract(&s("url"), &p, true).await;
            if v.is_null() { json!({"error": "user not found"}) } else { extract::norm_sc_user(&v, &s("url")) }
        }
        "soundcloud/playlist" => {
            let v = extract::sc_extract(&s("url"), &p, true).await;
            if v.is_null() { json!({"error": "playlist not found"}) } else { extract::norm_sc_playlist(&v, &s("url")) }
        }
        "soundcloud/usertracks" => extract::soundcloud_usertracks(&p).await,
        "soundcloud/related" => extract::soundcloud_related(&p).await,
        "soundcloud/download/audio" => download(&p, "sca", true).await,
        "tiktok/latest" => extract::tiktok_latest(&p).await,
        "reddit/media" => extract::reddit_media(&p).await,
        "reddit/download" => download(&p, "rd", false).await,
        "crypto/check" => crypto::check_remote(&p).await,
        "util/catbox" => {
            web::catbox(&s("filename"), &s("data_b64"), p.get("userhash").and_then(|v| v.as_str())).await
        }
        "util/imgbb" => {
            use base64::Engine;
            match base64::engine::general_purpose::STANDARD.decode(s("data_b64")) {
                Ok(b) => match render::imgbb(&b, &s("filename"), &s("key").ware_or("838a32e629bbf51f4fbec13e9d3f7863")).await {
                    Some(u) => json!({"url": u}),
                    None => json!({"error": "upload failed"}),
                },
                Err(_) => json!({"error": "bad base64"}),
            }
        }
        "util/dominant-color" => render::util_dominant(&p).await,
        "util/svg" => render::util_svg(&p),
        "util/zip" => render::util_zip(&p).await,
        "fun/caption" => render::fun_caption(&p).await,
        "fun/img2gif" => render::fun_img2gif(&p).await,
        "fun/vid2gif" => render::fun_vid2gif(&p).await,
        "fun/quote" => render::fun_quote(&p).await,
        "fun/quote/fonts" => json!({"fonts": ["poppins.semibold.ttf", "Banita.ttf", "Qilka-Bold copy.otf", "impact.ttf", "DancingScript.ttf"]}),
        "levelling/card" => render::lev_card(&p).await,
        "levelling/levelup" => render::lev_levelup(&p).await,
        "levelling/leaderboard" => render::lev_leaderboard(&p).await,
        "stats/dashboard" => render::stats_dashboard(&p).await,
        _ => json!({"error": format!("unknown route: {route}")}),
    }
}

fn dl_prefix(tag: &str) -> String {
    let dir = std::env::var("BAPI_TMP").ok().filter(|s| !s.is_empty()).or_else(|| {
        let c = config::get("dl_dir");
        if c.is_empty() { None } else { Some(c) }
    }).unwrap_or_else(|| std::env::temp_dir().to_string_lossy().into_owned());
    format!("{}/{}_{}", dir.trim_end_matches('/'), tag, now_unix())
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

trait WareOr {
    fn ware_or(&self, d: &str) -> String;
}
impl WareOr for String {
    fn ware_or(&self, d: &str) -> String {
        if self.is_empty() { d.to_string() } else { self.clone() }
    }
}

pub fn call(route: &str, params_json: &str) -> String {
    let p: Value = serde_json::from_str(params_json).unwrap_or(Value::Null);
    let timeout = match route {
        "tiktok/latest" => 25,
        "youtube/download" | "soundcloud/download/audio" | "reddit/download" => 320,
        "fun/caption" | "fun/vid2gif" | "crypto/check" => 120,
        _ => 60,
    };
    let t = net::now_ms();
    let v = net::run(timeout, async { Ok::<Value, String>(dispatch(route, &p).await) })
        .unwrap_or_else(|e| json!({"error": e}));
    let ms = net::now_ms() - t;
    let outcome = if v.get("error").is_some() {
        format!("err {}", v.get("error").and_then(|e| e.as_str()).unwrap_or("?"))
    } else if let Some(a) = v.as_array() {
        format!("ok:{}", a.len())
    } else {
        "ok".to_string()
    };
    net::log("api", &format!("{route} {ms}ms {outcome} :: {}", net::summarize(&p)));
    v.to_string()
}

#[unsafe(no_mangle)]
pub extern "C" fn apicall(route: *const c_char, params: *const c_char) -> *mut c_char {
    let r = unsafe { CStr::from_ptr(route) }.to_string_lossy().into_owned();
    let pj = if params.is_null() {
        "{}".to_string()
    } else {
        unsafe { CStr::from_ptr(params) }.to_string_lossy().into_owned()
    };
    match CString::new(call(&r, &pj)) {
        Ok(s) => s.into_raw(),
        Err(_) => CString::new(r#"{"error":"utf8"}"#).unwrap().into_raw(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn apifree(s: *mut c_char) {
    if !s.is_null() {
        unsafe { drop(CString::from_raw(s)) };
    }
}

#[pyo3::pymodule]
mod bapi {
    use pyo3::prelude::*;

    #[pyfunction]
    fn call(py: pyo3::Python<'_>, route: &str, params_json: &str) -> String {
        py.allow_threads(|| crate::call(route, params_json))
    }

    #[pyfunction]
    fn apicall(py: pyo3::Python<'_>, route: &str, params_json: &str) -> String {
        py.allow_threads(|| crate::call(route, params_json))
    }

    #[pyfunction]
    fn routes() -> Vec<String> {
        vec![
            "search/web", "search/images", "spotify/token",
            "religion/bible", "religion/bible/random", "religion/quran", "religion/quran/random",
            "youtube/search", "youtube/video", "youtube/download",
            "soundcloud/search", "soundcloud/track", "soundcloud/user", "soundcloud/playlist",
            "soundcloud/usertracks", "soundcloud/related", "soundcloud/download/audio",
            "tiktok/latest", "reddit/media", "reddit/download",
            "crypto/check", "util/catbox", "util/imgbb", "util/dominant-color", "util/svg", "util/zip",
            "fun/caption", "fun/img2gif", "fun/vid2gif", "fun/quote", "fun/quote/fonts",
            "levelling/card", "levelling/levelup", "levelling/leaderboard",
            "stats/dashboard",
        ]
        .into_iter()
        .map(str::to_string)
        .collect()
    }
}
