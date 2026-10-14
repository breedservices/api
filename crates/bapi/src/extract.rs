use serde_json::{json, Value};
use std::path::Path;
use tokio::process::Command;

use crate::net;

fn ua_for(url: &str) -> &'static str {
    let l = url.to_lowercase();
    if l.contains("youtube.com") || l.contains("youtu.be") || l.starts_with("ytsearch") {
        net::YT_UA
    } else {
        net::pick_ua()
    }
}

fn cred(p: &Value, key: &str, fb: &str) -> String {
    p.get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| std::env::var(format!("BAPI_{}", key.to_uppercase())).ok())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            let c = crate::config::get(key);
            if c.is_empty() { None } else { Some(c) }
        })
        .unwrap_or_else(|| fb.to_string())
}

fn base_args(url: &str, p: &Value) -> Vec<String> {
    let mut a = vec![
        "--no-warnings".into(),
        "--socket-timeout".into(),
        "15".into(),
        "--force-ipv4".into(),
        "--user-agent".into(),
        ua_for(url).into(),
        "--no-check-certificate".into(),
    ];
    if let Some(c) = p.get("cookies").and_then(|v| v.as_str()) {
        if Path::new(c).exists() {
            a.push("--cookies".into());
            a.push(c.into());
        }
    }
    let mut xa: Vec<String> = vec![];
    let lw = url.to_lowercase();
    if lw.contains("youtube.com") || lw.contains("youtu.be") || lw.starts_with("ytsearch") {
        xa.push(format!("youtube:player_client={}", cred(p, "player_client", "android")));
    }
    let po = cred(p, "po_token", "");
    if !po.is_empty() {
        xa.push(format!("youtube:po_token={po}"));
        let vd = cred(p, "visitor_data", "");
        if !vd.is_empty() {
            xa.push(format!("youtube:visitor_data={vd}"));
        }
    }
    let scid = cred(p, "sc_client_id", "");
    if !scid.is_empty() {
        xa.push(format!("soundcloud:client_id={scid}"));
    }
    if !xa.is_empty() {
        a.push("--extractor-args".into());
        a.push(xa.join(";"));
    }
    a
}

fn redact(args: &[String]) -> Vec<String> {
    args.iter()
        .map(|a| {
            if a.starts_with("youtube:po_token=")
                || a.starts_with("youtube:visitor_data=")
                || a.starts_with("soundcloud:client_id=")
            {
                format!("{}<redacted>", a.split('=').next().unwrap_or("?"))
            } else if a.starts_with("--cookies") {
                a.clone()
            } else {
                a.clone()
            }
        })
        .collect()
}

async fn run_ytdlp(args: &[String]) -> Result<Value, String> {
    let t = net::now_ms();
    let target = args.iter().rev().find(|a| a.starts_with("http") || a.starts_with("ytsearch") || a.starts_with("scsearch")).cloned().unwrap_or_default();
    let out = Command::new("yt-dlp")
        .args(args)
        .output()
        .await
        .map_err(|e| format!("yt-dlp spawn failed: {e}"))?;
    let ms = net::now_ms() - t;
    if !out.status.success() {
        let tail: String = String::from_utf8_lossy(&out.stderr).chars().take(160).collect();
        net::log("ytdlp", &format!("FAIL {ms}ms {target} :: {tail}"));
        return Err(format!("yt-dlp failed: {tail}"));
    }
    net::log("ytdlp", &format!("ok {ms}ms {target} :: {}", redact(args).join(" ")));
    serde_json::from_slice(&out.stdout).map_err(|e| format!("bad yt-dlp json: {e}"))
}


pub(crate) fn norm_video(e: &Value) -> Value {
    let id = e.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let thumb = e
        .get("thumbnail")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg"));
    json!({
        "id": id,
        "title": e.get("title").and_then(|v| v.as_str()).unwrap_or("untitled"),
        "description": e.get("description").and_then(|v| v.as_str()).unwrap_or(""),
        "thumbnail": thumb,
        "uploader": e.get("uploader").and_then(|v| v.as_str()).unwrap_or("unknown"),
        "uploader_id": e.get("channel_id").and_then(|v| v.as_str()).unwrap_or("unknown"),
        "uploader_url": e.get("channel_url").and_then(|v| v.as_str()).unwrap_or(""),
        "view_count": e.get("view_count").and_then(|v| v.as_u64()).unwrap_or(0),
        "like_count": e.get("like_count").and_then(|v| v.as_u64()).unwrap_or(0),
        "comment_count": e.get("comment_count").and_then(|v| v.as_u64()).unwrap_or(0),
        "duration": e.get("duration").and_then(|v| v.as_u64()).unwrap_or(0),
        "upload_date": e.get("upload_date").and_then(|v| v.as_str()).unwrap_or(""),
        "url": format!("https://www.youtube.com/watch?v={id}"),
    })
}

pub async fn youtube_search(p: &Value) -> Value {
    let q = p.get("q").and_then(|v| v.as_str()).unwrap_or("");
    let count = p.get("count").and_then(|v| v.as_u64()).unwrap_or(10).min(10).max(1);
    let shorts = p.get("shorts").and_then(|v| v.as_bool()).unwrap_or(false);
    let target = format!("ytsearch{count}:{q}{}", if shorts { " shorts" } else { "" });
    let mut args = base_args(&target, p);
    args.extend(["--flat-playlist".into(), "-J".into(), target.clone()]);
    let res = match run_ytdlp(&args).await {
        Ok(v) => v,
        Err(_) => return json!([]),
    };
    let entries = res.get("entries").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut jobs = Vec::new();
    for e in entries.into_iter().take(count as usize) {
        let url = e.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if url.is_empty() {
            continue;
        }
        let pc = p.clone();
        jobs.push(tokio::spawn(async move {
            let mut a = base_args(&url, &pc);
            a.extend(["-J".into(), "--no-playlist".into(), url]);
            run_ytdlp(&a).await.unwrap_or(Value::Null)
        }));
    }
    let mut out = Vec::new();
    for j in jobs {
        if let Ok(v) = j.await {
            if !v.is_null() {
                out.push(norm_video(&v));
            }
        }
    }
    json!(out)
}

pub async fn youtube_video(p: &Value) -> Value {
    let url = p.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let mut args = base_args(url, p);
    args.extend(["-J".into(), "--no-playlist".into(), url.into()]);
    match run_ytdlp(&args).await {
        Ok(v) if !v.is_null() => norm_video(&v),
        _ => json!({"error": "not found"}),
    }
}

pub async fn download_video(p: &Value, prefix: &str, audio_only: bool) -> Value {
    let url = p.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let outtmpl = format!("{prefix}.%(ext)s");
    let mut args = base_args(url, p);
    args.push("-o".into());
    args.push(outtmpl);
    args.push("--no-playlist".into());
    if audio_only {
        args.extend([
            "-f".into(), "bestaudio/best".into(),
            "--extract-audio".into(), "--audio-format".into(), "mp3".into(),
            "--audio-quality".into(), "192K".into(),
        ]);
    } else {
        args.extend([
            "-f".into(), "bestvideo[ext=mp4]+bestaudio[ext=m4a]/best[ext=mp4]/best".into(),
            "--merge-output-format".into(), "mp4".into(),
        ]);
    }
    args.push(url.into());
    let st = Command::new("yt-dlp").args(&args).status().await;
    if st.map(|s| s.success()).unwrap_or(false) {
        if let Ok(rd) = std::fs::read_dir(Path::new(prefix).parent().unwrap_or(Path::new("."))) {
            let base = Path::new(prefix).file_name().and_then(|s| s.to_str()).unwrap_or("");
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.starts_with(base) {
                    let full = e.path().to_string_lossy().into_owned();
                    if e.metadata().map(|m| m.len() > 195 * 1024 * 1024).unwrap_or(true) {
                        let _ = std::fs::remove_file(&full);
                        return json!({"error": "file too large"});
                    }
                    return json!({"path": full});
                }
            }
        }
    }
    json!({"error": "download failed"})
}


pub(crate) fn norm_sc_track(e: &Value) -> Value {
    json!({
        "id": e.get("id").map(|v| v.to_string().trim_matches('"').to_string()).unwrap_or_default(),
        "title": e.get("title").and_then(|v| v.as_str()).unwrap_or("unknown track"),
        "description": e.get("description").and_then(|v| v.as_str()).unwrap_or(""),
        "thumbnail": e.get("thumbnail").and_then(|v| v.as_str()).unwrap_or(""),
        "uploader": e.get("uploader").and_then(|v| v.as_str()).unwrap_or("unknown artist"),
        "uploader_id": e.get("uploader_id").and_then(|v| v.as_str()).unwrap_or(""),
        "uploader_url": e.get("uploader_url").and_then(|v| v.as_str()).unwrap_or(""),
        "view_count": e.get("view_count").and_then(|v| v.as_u64()).unwrap_or(0),
        "like_count": e.get("like_count").and_then(|v| v.as_u64()).unwrap_or(0),
        "comment_count": e.get("comment_count").and_then(|v| v.as_u64()).unwrap_or(0),
        "duration": e.get("duration").and_then(|v| v.as_u64()).unwrap_or(0),
        "upload_date": e.get("upload_date").and_then(|v| v.as_str()).unwrap_or(""),
        "url": e.get("webpage_url").and_then(|v| v.as_str())
             .or_else(|| e.get("url").and_then(|v| v.as_str())).unwrap_or(""),
    })
}

pub async fn sc_extract(url: &str, p: &Value, flat: bool) -> Value {
    let mut args = base_args(url, p);
    args.extend(["-J".into(), url.into()]);
    args.push(if flat { "--flat-playlist".into() } else { "--no-playlist".into() });
    run_ytdlp(&args).await.unwrap_or(Value::Null)
}

pub async fn soundcloud_search(p: &Value) -> Value {
    let q = p.get("q").and_then(|v| v.as_str()).unwrap_or("");
    let count = p.get("count").and_then(|v| v.as_u64()).unwrap_or(10).min(10).max(1);
    let mut urls: Vec<String> = vec![];
    let target = format!("scsearch{count}:{q}");
    let mut args = base_args(&target, p);
    args.extend(["--flat-playlist".into(), "-J".into(), target]);
    if let Ok(v) = run_ytdlp(&args).await {
        for e in v.get("entries").and_then(|x| x.as_array()).cloned().unwrap_or_default() {
            if let Some(u) = e.get("url").and_then(|x| x.as_str()) {
                urls.push(u.to_string());
            }
        }
    }
    let mut jobs = vec![];
    for u in urls.into_iter().take(count as usize) {
        let pc = p.clone();
        jobs.push(tokio::spawn(async move { sc_extract(&u, &pc, false).await }));
    }
    let mut out = vec![];
    for j in jobs {
        if let Ok(v) = j.await {
            if !v.is_null() {
                out.push(norm_sc_track(&v));
            }
        }
    }
    json!(out)
}

pub async fn tiktok_latest(p: &Value) -> Value {    let uid = p.get("user_id").and_then(|v| v.as_str()).unwrap_or("").trim_start_matches('@').to_string();
    let url = format!("https://www.tiktok.com/@{uid}");
    let mut args = base_args(&url, p);
    args.extend([
        "-J".into(), "--flat-playlist".into(), "--playlist-items".into(), "1".into(),
        "--lazy-playlist".into(), url,
    ]);
    match run_ytdlp(&args).await {
        Ok(v) => {
            let entries = v.get("entries").and_then(|x| x.as_array()).cloned().unwrap_or_default();
            match entries.into_iter().next() {
                Some(e) if !e.is_null() => json!({
                    "id": e.get("id").and_then(|x| x.as_str()).unwrap_or(""),
                    "timestamp": e.get("timestamp").and_then(|x| x.as_u64()),
                    "thumbnail": e.get("thumbnail").and_then(|x| x.as_str()),
                    "uploader_pfp": v.get("thumbnail").and_then(|x| x.as_str()),
                }),
                _ => json!({"error": "no video found"}),
            }
        }
        Err(e) => json!({"error": e}),
    }
}

pub async fn reddit_media(p: &Value) -> Value {
    let url = p.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let args: Vec<String> = vec![
        "--no-warnings".into(), "-J".into(), "--no-playlist".into(),
        "-f".into(), "bestvideo+bestaudio/best".into(), url.into(),
    ];
    match run_ytdlp(&args).await {
        Ok(v) => match v.get("url").and_then(|x| x.as_str()) {
            Some(u) => json!({"url": u}),
            None => json!({"error": "no media found"}),
        },
        Err(_) => json!({"error": "no media found"}),
    }
}

pub(crate) fn norm_sc_user(d: &Value, url: &str) -> Value {
    json!({
        "id": d.get("id").map(|v| v.to_string().trim_matches('"').to_string()).unwrap_or_default(),
        "username": d.get("uploader").and_then(|v| v.as_str()).unwrap_or(""),
        "full_name": d.get("title").and_then(|v| v.as_str()).unwrap_or(""),
        "description": d.get("description").and_then(|v| v.as_str()).unwrap_or(""),
        "thumbnail": d.get("thumbnail").and_then(|v| v.as_str()).unwrap_or(""),
        "follower_count": d.get("follower_count").and_then(|v| v.as_u64()).unwrap_or(0),
        "following_count": d.get("following_count").and_then(|v| v.as_u64()).unwrap_or(0),
        "track_count": d.get("track_count").and_then(|v| v.as_u64()).unwrap_or(0),
        "url": d.get("webpage_url").and_then(|v| v.as_str()).unwrap_or(url),
    })
}

pub(crate) fn norm_sc_playlist(d: &Value, url: &str) -> Value {
    json!({
        "id": d.get("id").map(|v| v.to_string().trim_matches('"').to_string()).unwrap_or_default(),
        "title": d.get("title").and_then(|v| v.as_str()).unwrap_or(""),
        "uploader": d.get("uploader").and_then(|v| v.as_str()).unwrap_or(""),
        "track_count": d.get("entries").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0),
        "url": d.get("webpage_url").and_then(|v| v.as_str()).unwrap_or(url),
        "thumbnail": d.get("thumbnail").and_then(|v| v.as_str()).unwrap_or(""),
    })
}

async fn sc_full_each(urls: Vec<String>, p: &Value) -> Value {
    let mut jobs = vec![];
    for u in urls {
        let pc = p.clone();
        jobs.push(tokio::spawn(async move { sc_extract(&u, &pc, false).await }));
    }
    let mut out = vec![];
    for j in jobs {
        if let Ok(v) = j.await {
            if !v.is_null() {
                out.push(norm_sc_track(&v));
            }
        }
    }
    json!(out)
}

pub async fn soundcloud_usertracks(p: &Value) -> Value {
    let url = p.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let count = p.get("count").and_then(|v| v.as_u64()).unwrap_or(5).min(50).max(1);
    let target = if url.contains("/tracks") { url.to_string() } else { format!("{}/tracks", url.trim_end_matches('/')) };
    let mut args = base_args(&target, p);
    args.extend(["--flat-playlist".into(), "--playlist-end".into(), count.to_string(), "-J".into(), target]);
    let urls = match run_ytdlp(&args).await {
        Ok(v) => v.get("entries").and_then(|x| x.as_array()).cloned().unwrap_or_default().iter().filter_map(|e| e.get("url").and_then(|x| x.as_str()).map(|s| s.to_string())).collect(),
        Err(_) => vec![],
    };
    sc_full_each(urls, p).await
}

pub async fn soundcloud_related(p: &Value) -> Value {
    let url = p.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let count = p.get("count").and_then(|v| v.as_u64()).unwrap_or(5).min(50).max(1);
    let target = format!("scsearch{count}:related:{url}");
    let mut args = base_args(&target, p);
    args.extend(["--flat-playlist".into(), "-J".into(), target]);
    let urls = match run_ytdlp(&args).await {
        Ok(v) => v.get("entries").and_then(|x| x.as_array()).cloned().unwrap_or_default().iter().filter_map(|e| e.get("url").and_then(|x| x.as_str()).map(|s| s.to_string())).collect(),
        Err(_) => vec![],
    };
    sc_full_each(urls, p).await
}
