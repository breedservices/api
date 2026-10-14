
use serde_json::{json, Value};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::net::{self, pct_decode, pct_encode, strip_tags};

static HELPER: Mutex<Option<Option<Vec<String>>>> = Mutex::new(None);

fn helper() -> Option<Vec<String>> {
    let mut g = HELPER.lock().unwrap();
    if let Some(h) = g.clone() {
        return h;
    }
    let found: Option<Vec<String>> = (|| {
        for c in [
            std::env::var("BAPI_DDGS_PY").unwrap_or_default(),
            crate::config::get("ddgs_py"),
        ] {
            if c.is_empty() {
                continue;
            }
            if c == "uv" || std::process::Command::new(&c).arg("--version").output().map(|o| o.status.success()).unwrap_or(false) {
                if c.ends_with("uv") || c == "uv" {
                    return Some(vec!["uv".to_string(), "run".to_string(), "--with".to_string(), "ddgs".to_string(), "python".to_string(), "-c".to_string()]);
                }
                return Some(vec![c]);
            }
        }
        if std::process::Command::new("uv").arg("--version").output().map(|o| o.status.success()).unwrap_or(false) {
            return Some(vec!["uv".to_string(), "run".to_string(), "--with".to_string(), "ddgs".to_string(), "python".to_string(), "-c".to_string()]);
        }
        for py in ["python3", "python"] {
            let probe = "import ddgs; print(1)";
            if std::process::Command::new(py).args(["-c", probe]).output().map(|o| o.status.success()).unwrap_or(false) {
                return Some(vec![py.to_string()]);
            }
        }
        None
    })();
    *g = Some(found.clone());
    found
}

async fn ddgs_run(kind: &str, query: &str, max: usize, safesearch: &str) -> Option<Value> {
    let h = helper()?;
    let script = "import json,os;from ddgs import DDGS;d=DDGS();"
        .to_string()
        + if kind == "images" {
            "print(json.dumps(d.images(query=os.environ['DDGS_Q'],max_results=int(os.environ['DDGS_N']),safesearch=os.environ.get('DDGS_S','moderate'))))"
        } else {
            "print(json.dumps(d.text(query=os.environ['DDGS_Q'],max_results=int(os.environ['DDGS_N']))))"
        };
    let mut cmd = tokio::process::Command::new(&h[0]);
    if h.len() > 1 {
        cmd.args(&h[1..]);
    }
    if h.len() == 1 && h[0] == "ddgs" {
        cmd.arg(kind).arg("-q").arg(query).arg("-n").arg(max.to_string());
    } else {
        if h.len() == 1 {
            cmd.arg("-c");
        }
        cmd.arg(script);
    }
    let out = cmd
        .env("DDGS_Q", query)
        .env("DDGS_N", max.to_string())
        .env("DDGS_S", safesearch)
        .output()
        .await
        .ok()?;
    if !out.status.success() {
        return None;
    }
    serde_json::from_slice::<Value>(&out.stdout).ok()
}


fn unmunge(href: &str) -> String {
    if let Some(p) = href.find("uddg=") {
        let rest = &href[p + 5..];
        let end = rest.find('&').unwrap_or(rest.len());
        return pct_decode(&rest[..end]);
    }
    href.to_string()
}

pub async fn ddg_text(query: &str, max: usize) -> Value {
    if query.is_empty() || query.len() > 500 {
        return json!({"results": [], "error": "query must be 1-500 characters"});
    }
    if let Some(v) = ddgs_run("text", query, max.max(1).min(20), "").await {
        if let Some(arr) = v.as_array() {
            let out: Vec<Value> = arr
                .iter()
                .take(max.max(1).min(20))
                .map(|r| {
                    json!({
                        "title": r.get("title").and_then(|x| x.as_str()).unwrap_or(""),
                        "body": r.get("body").and_then(|x| x.as_str()).unwrap_or(""),
                        "href": r.get("href").and_then(|x| x.as_str()).unwrap_or(""),
                    })
                })
                .collect();
            if !out.is_empty() {
                return json!({ "results": out });
            }
        }
    }
    let url = format!("https://html.duckduckgo.com/html/?q={}", pct_encode(query));
    let body = match net::get_text(&url, net::pick_ua()).await {
        Ok((200, t)) => t,
        Ok((s, _)) => return json!({"results": [], "error": format!("ddg http {s}")}),
        Err(e) => return json!({"results": [], "error": e}),
    };
    let mut out = Vec::new();
    let mut rest = body.as_str();
    while out.len() < max.max(1).min(20) {
        let a = match rest.find("result__a") {
            Some(i) => i,
            None => break,
        };
        rest = &rest[a..];
        let href = rest
            .find("href=\"")
            .and_then(|i| {
                let s = &rest[i + 6..];
                s.find('"').map(|e| s[..e].to_string())
            })
            .unwrap_or_default();
        let title = rest
            .find('>')
            .and_then(|i| rest[i + 1..].find("</a>").map(|e| rest[i + 1..i + 1 + e].to_string()))
            .unwrap_or_default();
        let snippet = rest
            .find("result__snippet")
            .and_then(|i| {
                rest[i..]
                    .find('>')
                    .and_then(|g| rest[i + g + 1..].find('<').map(|e| rest[i + g + 1..i + g + 1 + e].to_string()))
            })
            .unwrap_or_default();
        let link = unmunge(&href);
        if !link.is_empty() && !link.contains("duckduckgo.com/y.js") && !href.contains("ad_domain") {
            out.push(json!({
                "title": strip_tags(&title).trim(),
                "body": strip_tags(&snippet).trim(),
                "href": link,
            }));
        }
        rest = &rest[1.min(rest.len())..];
    }
    json!({ "results": out })
}

pub async fn ddg_images(query: &str, max: usize) -> Value {
    if query.is_empty() || query.len() > 500 {
        return json!({"results": [], "error": "query must be 1-500 characters"});
    }
    if let Some(v) = ddgs_run("images", query, max.max(1).min(100), "moderate").await {
        if let Some(arr) = v.as_array() {
            let mut out = vec![];
            for r in arr.iter().take(max.max(1).min(100)) {
                let img = r.get("image").and_then(|x| x.as_str()).unwrap_or("");
                let thumb = r.get("thumbnail").and_then(|x| x.as_str()).unwrap_or("");
                if (img.starts_with("http") || thumb.starts_with("http"))
                    && img.len() < 512
                    && thumb.len() < 512
                    && (!img.is_empty() || !thumb.is_empty())
                {
                    out.push(json!({
                        "title": r.get("title").and_then(|x| x.as_str()).unwrap_or(""),
                        "image": img, "thumbnail": thumb,
                        "url": r.get("url").and_then(|x| x.as_str()).unwrap_or(""),
                        "source": r.get("source").and_then(|x| x.as_str()).unwrap_or(""),
                    }));
                }
            }
            if !out.is_empty() {
                return json!({ "results": out });
            }
        }
    }
    let html = match net::get_text(
        &format!("https://duckduckgo.com/?q={}&iax=images&ia=images", pct_encode(query)),
        net::pick_ua(),
    )
    .await
    {
        Ok((200, t)) => t,
        Ok((s, _)) => return json!({"results": [], "error": format!("ddg http {s}")}),
        Err(e) => return json!({"results": [], "error": e}),
    };
    let vqd = html
        .find("vqd=\"")
        .and_then(|i| {
            let s = &html[i + 5..];
            s.find('"').map(|e| s[..e].to_string())
        })
        .or_else(|| {
            html.find("vqd='")
                .and_then(|i| {
                    let s = &html[i + 5..];
                    s.find('\'').map(|e| s[..e].to_string())
                })
        })
        .unwrap_or_default();
    if vqd.is_empty() {
        return json!({"results": [], "error": "ddg token unavailable"});
    }
    let url = format!(
        "https://duckduckgo.com/i.js?l=us-en&o=json&q={}&vqd={}&f=,,,,,&p=1&v7exp=a",
        pct_encode(query),
        pct_encode(&vqd)
    );
    let body = match net::get_text(&url, net::pick_ua()).await {
        Ok((200, t)) => t,
        Ok((s, _)) => return json!({"results": [], "error": format!("ddg http {s}")}),
        Err(e) => return json!({"results": [], "error": e}),
    };
    let data: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let mut out = Vec::new();
    if let Some(arr) = data.get("results").and_then(|v| v.as_array()) {
        for r in arr.iter().take(max.max(1).min(100)) {
            let img = r.get("image").and_then(|v| v.as_str()).unwrap_or("");
            let thumb = r.get("thumbnail").and_then(|v| v.as_str()).unwrap_or("");
            if (img.starts_with("http") || thumb.starts_with("http"))
                && img.len() < 512
                && thumb.len() < 512
                && (!img.is_empty() || !thumb.is_empty())
            {
                out.push(json!({
                    "title": r.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                    "image": img, "thumbnail": thumb,
                    "url": r.get("url").and_then(|v| v.as_str()).unwrap_or(""),
                    "source": r.get("source").and_then(|v| v.as_str()).unwrap_or(""),
                }));
            }
        }
    }
    json!({ "results": out })
}


struct Tok {
    token: String,
    exp_ms: u64,
}
static SPOTOK: Mutex<Option<Tok>> = Mutex::new(None);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub async fn spotify_token() -> Value {
    if let Some(t) = SPOTOK.lock().unwrap().as_ref() {
        if t.exp_ms > now_ms() + 60_000 && !t.token.is_empty() {
            return json!({"accessToken": t.token, "accessTokenExpirationTimestampMs": t.exp_ms});
        }
    }
    let url = "https://open.spotify.com/get_access_token?reason=transport&productType=web_player";
    let (st, body) = net::get_text(url, net::UAS[0]).await.unwrap_or((0, String::new()));
    if st != 200 {
        return json!({"error": format!("spotify http {st}")});
    }
    let v: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    let (Some(at), Some(exp)) = (
        v.get("accessToken").and_then(|x| x.as_str()),
        v.get("accessTokenExpirationTimestampMs").and_then(|x| x.as_u64()),
    ) else {
        return json!({"error": "bad spotify payload"});
    };
    *SPOTOK.lock().unwrap() = Some(Tok { token: at.to_string(), exp_ms: exp });
    json!({"accessToken": at, "accessTokenExpirationTimestampMs": exp})
}


pub async fn bible(book: &str, chapter: i64, verse: &str, version: &str) -> Value {
    let url = format!(
        "https://scriptura-api.com/api/verse?book={}&chapter={}&verse={}&version={}",
        pct_encode(book),
        chapter,
        pct_encode(verse),
        pct_encode(version)
    );
    match net::get_text(&url, net::pick_ua()).await {
        Ok((200, t)) => {
            let v: Value = serde_json::from_str(&t).unwrap_or(Value::String(t.clone()));
            let text = v.as_str().unwrap_or("").to_string();
            let text = if text.is_empty() {
                v.get("text").and_then(|x| x.as_str()).unwrap_or(&t).to_string()
            } else {
                text
            };
            json!({"reference": format!("{book} {chapter}:{verse}"),
                   "text": text.trim(), "translation": version.to_uppercase()})
        }
        Ok((s, _)) => json!({"error": format!("verse not found ({s})")}),
        Err(e) => json!({"error": e}),
    }
}

pub async fn bible_random(version: &str) -> Value {
    let url = format!("https://scriptura-api.com/api/random?version={}", pct_encode(version));
    match net::get_text(&url, net::pick_ua()).await {
        Ok((200, t)) => {
            let v: Value = serde_json::from_str(&t).unwrap_or(Value::String(t.clone()));
            let text = v.as_str().unwrap_or(v.get("text").and_then(|x| x.as_str()).unwrap_or(""));
            json!({"reference": "Random Verse", "text": text.trim(),
                   "translation": version.to_uppercase()})
        }
        Ok((s, _)) => json!({"error": format!("verse not found ({s})")}),
        Err(e) => json!({"error": e}),
    }
}

pub async fn quran(surah: i64, ayah: i64) -> Value {
    let url = format!("https://api.alquran.cloud/v1/ayah/{surah}:{ayah}/en.pickthall");
    match net::get_text(&url, net::pick_ua()).await {
        Ok((200, t)) => {
            let v: Value = serde_json::from_str(&t).unwrap_or(Value::Null);
            let d = v.get("data").cloned().unwrap_or(Value::Null);
            let name = d.get("surah").and_then(|s| s.get("englishName")).and_then(|x| x.as_str()).unwrap_or("");
            json!({"reference": format!("Surah {name} ({surah}:{ayah})"),
                   "text": d.get("text").and_then(|x| x.as_str()).unwrap_or(""),
                   "translation": "English (Pickthall)"})
        }
        Ok((s, _)) => json!({"error": format!("ayah not found ({s})")}),
        Err(e) => json!({"error": e}),
    }
}

pub async fn quran_random() -> Value {
    let url = "https://api.alquran.cloud/v1/ayah/random/editions/quran-uthmani,en.asad";
    match net::get_text(&url, net::pick_ua()).await {
        Ok((200, t)) => {
            let v: Value = serde_json::from_str(&t).unwrap_or(Value::Null);
            let arr = v.get("data").and_then(|x| x.as_array()).cloned().unwrap_or_default();
            if arr.len() < 2 {
                return json!({"error": "ayah data incomplete"});
            }
            let (ar, en) = (&arr[0], &arr[1]);
            let sname = ar.get("surah").and_then(|s| s.get("englishName")).and_then(|x| x.as_str()).unwrap_or("Unknown");
            let snum = ar.get("surah").and_then(|s| s.get("number")).and_then(|x| x.as_u64()).unwrap_or(0);
            let anum = ar.get("numberInSurah").and_then(|x| x.as_u64()).unwrap_or(0);
            json!({"reference": format!("Surah {sname} ({snum}:{anum})"),
                   "text": format!("{}\n\n{}", ar.get("text").and_then(|x| x.as_str()).unwrap_or(""),
                                                 en.get("text").and_then(|x| x.as_str()).unwrap_or("")),
                   "translation": "English (Asad)"})
        }
        Ok((s, _)) => json!({"error": format!("ayah not found ({s})")}),
        Err(e) => json!({"error": e}),
    }
}


pub async fn catbox(filename: &str, data_b64: &str, userhash: Option<&str>) -> Value {
    use base64::Engine;
    let data = match base64::engine::general_purpose::STANDARD.decode(data_b64) {
        Ok(d) => d,
        Err(_) => return json!({"error": "bad base64"}),
    };
    let part = reqwest::multipart::Part::bytes(data).file_name(filename.to_string());
    let mut form = reqwest::multipart::Form::new()
        .text("reqtype", "fileupload")
        .part("fileToUpload", part);
    if let Some(h) = userhash {
        form = form.text("userhash", h.to_string());
    }
    let r = net::http()
        .post("https://catbox.moe/user/api.php")
        .header("User-Agent", net::pick_ua())
        .multipart(form)
        .send()
        .await
        .map_err(|e| e.to_string());
    match r {
        Ok(resp) => {
            let t = resp.text().await.unwrap_or_default();
            let t = t.trim().to_string();
            if t.starts_with("http") {
                json!({"url": t})
            } else {
                json!({"error": format!("catbox rejected upload")})
            }
        }
        Err(e) => json!({"error": e}),
    }
}

#[cfg(test)]
mod tests {
    use super::unmunge;
    #[test]
    fn unmunge_ddg() {
        assert_eq!(
            unmunge("//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fx&rut=abc"),
            "https://example.com/x"
        );
        assert_eq!(unmunge("https://plain.example/"), "https://plain.example/");
    }
}
