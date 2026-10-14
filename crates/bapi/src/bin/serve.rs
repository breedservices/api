use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Query};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use base64::Engine;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use bapi::dispatch;

fn j(v: Value) -> axum::response::Response {
    ([("content-type", "application/json")], v.to_string()).into_response()
}

fn err(code: StatusCode, msg: String) -> axum::response::Response {
    (code, [("content-type", "application/json")], json!({"error": msg}).to_string()).into_response()
}

async fn run(route: &str, p: Value, secs: u64) -> Value {
    let t = std::time::SystemTime::now();
    let v = match tokio::time::timeout(Duration::from_secs(secs), dispatch(route, &p)).await {
        Ok(v) => v,
        Err(_) => json!({"error": "timed out"}),
    };
    let ms = t.elapsed().map(|d| d.as_millis()).unwrap_or(0);
    let outcome = if v.get("error").is_some() {
        format!("err {}", v.get("error").and_then(|e| e.as_str()).unwrap_or("?"))
    } else if let Some(a) = v.as_array() {
        format!("ok:{}", a.len())
    } else {
        "ok".to_string()
    };
    eprintln!("[{}] api: {route} {ms}ms {outcome}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0));
    v
}

async fn docs() -> impl IntoResponse {
    ([("content-type", "text/html; charset=utf-8")], include_str!("../../../../ui/index.html"))
}

async fn docs_css() -> impl IntoResponse {
    ([("content-type", "text/css; charset=utf-8")], include_str!("../../../../ui/style.css"))
}

async fn docs_js() -> impl IntoResponse {
    ([("content-type", "application/javascript; charset=utf-8")], include_str!("../../../../ui/app.js"))
}

fn qval(s: &str) -> Value {
    match s {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => s.parse::<i64>().map(Value::from).unwrap_or_else(|_| s.parse::<f64>().map(Value::from).unwrap_or_else(|_| Value::String(s.to_string()))),
    }
}

fn qmap(q: &HashMap<String, String>) -> Value {
    q.iter().map(|(k, v)| (k.clone(), qval(v))).collect::<serde_json::Map<String, Value>>().into()
}

async fn g(route: &'static str, secs: u64, Query(q): Query<HashMap<String, String>>) -> axum::response::Response {
    let p = qmap(&q);
    let v = run(route, p, secs).await;
    if v.get("error").and_then(|e| e.as_str()) == Some("not found")
        || v.get("error").and_then(|e| e.as_str()) == Some("track not found")
        || v.get("error").and_then(|e| e.as_str()) == Some("user not found")
        || v.get("error").and_then(|e| e.as_str()) == Some("playlist not found")
        || v.get("error").and_then(|e| e.as_str()) == Some("no media found")
        || v.get("error").and_then(|e| e.as_str()) == Some("no video found")
    {
        return err(StatusCode::NOT_FOUND, v["error"].as_str().unwrap_or("not found").to_string());
    }
    j(v)
}

async fn gfile(route: &'static str, Query(q): Query<HashMap<String, String>>) -> axum::response::Response {
    let p = qmap(&q);
    let catbox = q.get("catbox").map(|s| s == "true").unwrap_or(false);
    let v = run(route, p, 320).await;
    if catbox {
        return j(v);
    }
    match v.get("path").and_then(|x| x.as_str()) {
        Some(path) => match std::fs::read(path) {
            Ok(b) => {
                let _ = std::fs::remove_file(path);
                let ct = if route.contains("audio") { "audio/mpeg" } else { "video/mp4" };
                ([("content-type", ct)], b).into_response()
            }
            Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        },
        None => err(
            StatusCode::INTERNAL_SERVER_ERROR,
            v.get("error").and_then(|e| e.as_str()).unwrap_or("download failed").to_string(),
        ),
    }
}

async fn p(route: &'static str, secs: u64, body: Bytes) -> axum::response::Response {
    let p: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    j(run(route, p, secs).await)
}

async fn pimg(route: &'static str, body: Bytes) -> axum::response::Response {
    let p: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let v = run(route, p, 120).await;
    match v.get("data_b64").and_then(|x| x.as_str()) {
        Some(b) => match base64::engine::general_purpose::STANDARD.decode(b) {
            Ok(raw) => ([("content-type", v.get("media_type").and_then(|x| x.as_str()).unwrap_or("image/png"))], raw).into_response(),
            Err(_) => err(StatusCode::INTERNAL_SERVER_ERROR, "bad image".to_string()),
        },
        None => j(v),
    }
}

async fn crypto(headers: HeaderMap, body: Bytes) -> axum::response::Response {
    let want = std::env::var("BAPI_CRYPTO_TOKEN").ok().filter(|s| !s.is_empty()).or_else(|| {
        let c = bapi::config::get("crypto_token");
        if c.is_empty() { None } else { Some(c) }
    }).unwrap_or_else(|| "tokens".to_string());
    let got = headers.get("authorization").and_then(|v| v.to_str().ok()).unwrap_or("");
    let token = got.strip_prefix("Bearer ").or_else(|| got.strip_prefix("bearer ")).unwrap_or("");
    if token != want {
        return err(StatusCode::UNAUTHORIZED, "unauthorized".to_string());
    }
    let p: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    j(run("crypto/check", p, 120).await)
}

async fn health() -> axum::response::Response {
    j(json!({"status": "ok"}))
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/health", get(health))
        .route("/docs", get(docs))
        .route("/ui/style.css", get(docs_css))
        .route("/ui/app.js", get(docs_js))
        .route("/search/web", post(|b| p("search/web", 30, b)))
        .route("/search/images", post(|b| p("search/images", 30, b)))
        .route("/spotify/token", get(|q| g("spotify/token", 30, q)))
        .route("/religion/bible", get(|q| g("religion/bible", 30, q)))
        .route("/religion/bible/random", get(|q| g("religion/bible/random", 30, q)))
        .route("/religion/quran", get(|q| g("religion/quran", 30, q)))
        .route("/religion/quran/random", get(|q| g("religion/quran/random", 30, q)))
        .route("/youtube/search", get(|q| g("youtube/search", 60, q)))
        .route("/youtube/video", get(|q| g("youtube/video", 60, q)))
        .route("/youtube/download", get(|q| gfile("youtube/download", q)))
        .route("/soundcloud/search", get(|q| g("soundcloud/search", 60, q)))
        .route("/soundcloud/track", get(|q| g("soundcloud/track", 60, q)))
        .route("/soundcloud/user", get(|q| g("soundcloud/user", 60, q)))
        .route("/soundcloud/playlist", get(|q| g("soundcloud/playlist", 60, q)))
        .route("/soundcloud/usertracks", get(|q| g("soundcloud/usertracks", 60, q)))
        .route("/soundcloud/related", get(|q| g("soundcloud/related", 60, q)))
        .route("/soundcloud/download/audio", get(|q| gfile("soundcloud/download/audio", q)))
        .route("/tiktok/latest", get(|q| g("tiktok/latest", 25, q)))
        .route("/reddit/media", get(|q| g("reddit/media", 60, q)))
        .route("/reddit/download", get(|q| gfile("reddit/download", q)))
        .route("/crypto/check", post(crypto))
        .route("/utility/dominant-color", get(|q| g("util/dominant-color", 30, q)))
        .route("/utility/svg", post(|b| pimg("util/svg", b)))
        .route("/utility/zip", post(|b| pimg("util/zip", b)))
        .route("/fun/caption", post(|b| p("fun/caption", 120, b)))
        .route("/fun/img2gif", post(|b| pimg("fun/img2gif", b)))
        .route("/fun/vid2gif", post(|b| p("fun/vid2gif", 120, b)))
        .route("/fun/quote", post(|b| p("fun/quote", 60, b)))
        .route("/fun/quote/fonts", get(|| async { j(dispatch("fun/quote/fonts", &Value::Null).await) }))
        .route("/levelling/card", post(|b| pimg("levelling/card", b)))
        .route("/levelling/levelup", post(|b| pimg("levelling/levelup", b)))
        .route("/levelling/leaderboard", post(|b| pimg("levelling/leaderboard", b)))
        .route("/stats/dashboard", post(|b| pimg("stats/dashboard", b)))
        .layer(DefaultBodyLimit::max(200 * 1024 * 1024));

    let port: u16 = std::env::var("BAPI_PORT").ok().and_then(|s| s.parse().ok()).or_else(|| {
        bapi::config::all().get("port").and_then(|v| v.as_u64()).map(|n| n as u16)
    }).unwrap_or(5461);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("listening on {addr}");
    axum::Server::bind(&addr).serve(app.into_make_service()).await.unwrap();
}
