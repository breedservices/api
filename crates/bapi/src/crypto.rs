
use serde_json::{json, Value};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::{ffi, render};

const KEYWORDS: [&str; 15] = [
    "claim your reward",
    "crypto casino",
    "post will be deleted",
    "usdt",
    "promo code",
    "withdrawal success",
    "your withdrawal of",
    "the money will be transferred",
    "select a withdraw method",
    "wallet address",
    "usdc",
    "block explorer",
    "view on block",
    "known scam image hash",
    "0x",
];

static HASHES: Mutex<Vec<u64>> = Mutex::new(Vec::new());
static LOADED_FROM: Mutex<Option<PathBuf>> = Mutex::new(None);

fn default_store() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("api")
        .join("routes")
        .join("known_hashes.msgpack")
}

fn decode_hashes(b: &[u8]) -> Vec<u64> {
    let mut out = vec![];
    if b.is_empty() {
        return out;
    }
    let (mut n, mut i) = match b[0] {
        0x90..=0x9f => ((b[0] & 0x0f) as usize, 1),
        0xdc if b.len() >= 3 => ((u16::from_be_bytes([b[1], b[2]]) as usize), 3),
        _ => return out,
    };
    while n > 0 && i < b.len() {
        let (v, w): (u64, usize) = match b[i] {
            0x00..=0x7f => (b[i] as u64, 1),
            0xcc if i + 2 <= b.len() => (b[i + 1] as u64, 2),
            0xcd if i + 3 <= b.len() => (u16::from_be_bytes([b[i + 1], b[i + 2]]) as u64, 3),
            0xce if i + 5 <= b.len() => (u32::from_be_bytes(b[i + 1..i + 5].try_into().unwrap()) as u64, 5),
            0xcf if i + 9 <= b.len() => (u64::from_be_bytes(b[i + 1..i + 9].try_into().unwrap()), 9),
            _ => break,
        };
        out.push(v);
        i += w;
        n -= 1;
    }
    out
}

fn encode_hashes(h: &[u64]) -> Vec<u8> {
    let mut b = vec![];
    if h.len() < 16 {
        b.push(0x90 | h.len() as u8);
    } else {
        b.push(0xdc);
        b.extend_from_slice(&(h.len() as u16).to_be_bytes());
    }
    for v in h {
        b.push(0xcf);
        b.extend_from_slice(&v.to_be_bytes());
    }
    b
}

fn load(store: &PathBuf) {
    let mut guard = HASHES.lock().unwrap();
    let same = LOADED_FROM.lock().unwrap().as_ref() == Some(store);
    if same && !guard.is_empty() {
        return;
    }
    *guard = fs::read(store).map(|b| decode_hashes(&b)).unwrap_or_default();
    *LOADED_FROM.lock().unwrap() = Some(store.clone());
}

fn hash_exists(h: u64, similarity: f64) -> bool {
    let threshold = ((1.0 - similarity) * 64.0) as u32;
    let guard = HASHES.lock().unwrap();
    if guard.is_empty() {
        return false;
    }
    ffi::hamming_batch(h, &guard, threshold as i32)
}

fn store_hash(h: u64, store: &PathBuf) {
    let mut guard = HASHES.lock().unwrap();
    if !guard.contains(&h) {
        guard.push(h);
        if guard.len() > 50_000 {
            guard.remove(0);
        }
        let _ = fs::write(store, encode_hashes(&guard));
    }
}

fn is_hex_addr(tok: &str) -> bool {
    let t = tok.trim_matches(|c: char| !c.is_alphanumeric());
    (t.len() == 42 && t.starts_with("0x") && t[2..].chars().all(|c| c.is_ascii_hexdigit()))
        || ((25..=44).contains(&t.len())
            && t.chars().all(|c| c.is_alphanumeric() && !"0OIl".contains(c)))
}

pub fn analyze_text(text: &str) -> (bool, &'static str, Vec<String>) {
    let lower = text.to_lowercase();
    let mut matches: Vec<String> = vec![];
    for k in KEYWORDS {
        if k == "0x" {
            continue;
        }
        if lower.contains(k) && !matches.iter().any(|m| m == k) {
            matches.push(k.to_string());
        }
    }
    for tok in lower.split(|c: char| !(c.is_alphanumeric() || c == 'x')) {
        if tok.len() >= 25 && is_hex_addr(tok) {
            matches.push(tok.to_string());
            break;
        }
    }
    if lower.contains("0x") && !matches.iter().any(|m| m.starts_with("0x")) {
        matches.push("0x".to_string());
    }
    let conf = match matches.len() {
        0 => "none",
        1 => "low",
        2..=3 => "medium",
        _ => "high",
    };
    (!matches.is_empty(), conf, matches)
}

pub async fn check_remote(p: &Value) -> Value {
    let store = store_path(p);
    load(&store);
    let items = p.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    if items.is_empty() {
        return json!({"error": "no image urls provided"});
    }
    let mut jobs = vec![];
    for it in items {
        let url = it.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let text = it.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let hash = it.get("hash").and_then(|v| v.as_u64()).unwrap_or(0);
        jobs.push(tokio::spawn(async move {
            if !text.is_empty() || hash != 0 {
                return (url, text, hash);
            }
            match render::fetch(&url).await {
                Ok((200, b, _)) if !b.is_empty() => {
                    let h = render::decode(&b).map(|img| ffi::phash(img.as_raw(), img.width(), img.height())).unwrap_or(0);
                    (url, ffi::ocr(&b), h)
                }
                _ => (url, String::new(), 0),
            }
        }));
    }
    let mut triples = vec![];
    for j in jobs {
        if let Ok(t) = j.await {
            triples.push(t);
        }
    }
    score(triples, p)
}

fn store_path(p: &Value) -> PathBuf {
    p.get("store")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_else(default_store)
}

fn score(triples: Vec<(String, String, u64)>, p: &Value) -> Value {
    let store = store_path(p);
    load(&store);
    let mut results = vec![];
    for (url, text, hash) in &triples {
        let (url, text, hash) = (url.clone(), text.clone(), *hash);
        if hash != 0 && hash_exists(hash, 0.8) {
            results.push(json!({"url": url, "is_scam": true, "confidence": "high",
                                "matches": ["known scam image hash"], "hash": hash}));
            continue;
        }
        if text.is_empty() && hash == 0 {
            results.push(json!({"url": url, "is_scam": false, "confidence": "none",
                                "matches": [], "hash": 0}));
            continue;
        }
        let (scam, conf, m) = analyze_text(&text);
        if scam && hash != 0 {
            store_hash(hash, &store);
        }
        results.push(json!({"url": url, "is_scam": scam, "confidence": conf,
                            "matches": m, "hash": hash}));
    }
    let overall = results.iter().any(|r| r.get("is_scam").and_then(|v| v.as_bool()).unwrap_or(false));
    let mut om: Vec<String> = vec![];
    for r in &results {
        for m in r.get("matches").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
            if let Some(s) = m.as_str() {
                if !om.iter().any(|x| x == s) {
                    om.push(s.to_string());
                }
            }
        }
    }
    let oconf = if !overall {
        "none"
    } else if results.iter().any(|r| r.get("confidence").and_then(|v| v.as_str()) == Some("high")) {
        "high"
    } else if results.iter().any(|r| r.get("confidence").and_then(|v| v.as_str()) == Some("medium")) {
        "medium"
    } else {
        "low"
    };
    let ohash = results
        .iter()
        .find(|r| r.get("is_scam").and_then(|v| v.as_bool()).unwrap_or(false))
        .or(results.first())
        .and_then(|r| r.get("hash").and_then(|v| v.as_u64()))
        .unwrap_or(0);
    json!({"is_scam": overall, "confidence": oconf, "matches": om,
           "hash": ohash, "results": results})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scoring() {
        let (s, c, m) = analyze_text("Claim your reward! Send USDT to wallet address 0xAbC123...promo code WITHDRAWAL SUCCESS your withdrawal of");
        assert!(s);
        assert!(m.len() >= 4);
        assert_eq!(c, "high");
        let (s2, c2, _) = analyze_text("hello world");
        assert!(!s2);
        assert_eq!(c2, "none");
    }
    #[test]
    fn msgpack_roundtrip() {
        let h = vec![1u64, u64::MAX, 123456789];
        assert_eq!(decode_hashes(&encode_hashes(&h)), h);
        assert!(decode_hashes(&[]).is_empty());
    }
}
