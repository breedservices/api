
use std::sync::OnceLock;
use std::time::Duration;

use reqwest::Client;
use tokio::runtime::Runtime;

pub const UAS: [&str; 4] = [
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Linux; Android 10; K) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Mobile Safari/537.36",
];
pub const YT_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/147.0.0.0 Safari/537.36";

static RT: OnceLock<Runtime> = OnceLock::new();
static HTTP: OnceLock<Client> = OnceLock::new();

pub fn rt() -> &'static Runtime {
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

pub fn http() -> &'static Client {
    HTTP.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("reqwest client")
    })
}

pub fn run<T, F>(secs: u64, fut: F) -> Result<T, String>
where
    F: std::future::Future<Output = Result<T, String>>,
{
    rt().block_on(async move {
        tokio::time::timeout(Duration::from_secs(secs), fut)
            .await
            .map_err(|_| "timed out".to_string())?
    })
}

static SEED: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0x9E3779B97F4A7C15);

pub fn pick_ua() -> &'static str {
    let mut x = SEED.fetch_add(0x9E3779B97F4A7C15, std::sync::atomic::Ordering::Relaxed);
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    UAS[(x.wrapping_mul(0x2545F4914F6CDD1D) >> 32) as usize % UAS.len()]
}

pub async fn get_text(url: &str, ua: &str) -> Result<(u16, String), String> {
    let r = http()
        .get(url)
        .header("User-Agent", ua)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let st = r.status().as_u16();
    r.text().await.map(|t| (st, t)).map_err(|e| e.to_string())
}

#[allow(dead_code)]
pub async fn get_bytes(url: &str, ua: &str, secs: u64) -> Result<(u16, Vec<u8>), String> {
    let r = http()
        .get(url)
        .header("User-Agent", ua)
        .timeout(Duration::from_secs(secs))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let st = r.status().as_u16();
    r.bytes()
        .await
        .map(|b| (st, b.to_vec()))
        .map_err(|e| e.to_string())
}

pub fn pct_decode(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(h << 4 | l);
                i += 3;
                continue;
            }
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

pub fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut tag = false;
    for c in s.chars() {
        match c {
            '<' => tag = true,
            '>' => tag = false,
            _ if !tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

pub fn pct_encode(s: &str) -> String {    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => {
                out.push('%');
                out.push(char::from_digit((b >> 4) as u32, 16).unwrap().to_ascii_uppercase());
                out.push(char::from_digit((b & 15) as u32, 16).unwrap().to_ascii_uppercase());
            }
        }
    }
    out
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn log(tag: &str, msg: &str) {
    eprintln!("[{}] {tag}: {msg}", now_ms() / 1000);
}

pub fn summarize(p: &serde_json::Value) -> String {
    match p {
        serde_json::Value::Object(m) => {
            let keys: Vec<String> = m.keys().cloned().collect();
            format!("{{{}}}~{}B", keys.join(","), p.to_string().len())
        }
        serde_json::Value::Array(a) => format!("[{}]~{}B", a.len(), p.to_string().len()),
        _ => format!("~{}B", p.to_string().len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pct_roundtrip() {
        assert_eq!(pct_decode("hello%20world%21+a"), "hello world! a");
        assert_eq!(strip_tags("<b>hi</b> &amp; bye"), "hi & bye");
    }
}

