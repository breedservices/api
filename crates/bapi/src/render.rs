use ab_glyph::{Font, FontVec, Glyph, PxScale, ScaleFont};
use base64::Engine;
use image::codecs::gif::{GifDecoder, GifEncoder, Repeat};
use image::{AnimationDecoder, Delay, Frame, ImageDecoder, Rgba, RgbaImage};
use serde_json::{json, Value};
use std::io::Cursor;

use crate::{ffi, net};

pub fn fonts_dir() -> std::path::PathBuf {
    if let Ok(d) = std::env::var("BAPI_FONTS") {
        if !d.is_empty() {
            return d.into();
        }
    }
    let c = crate::config::get("fonts_dir");
    if !c.is_empty() {
        return c.into();
    }
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fonts")
}

pub fn font_bytes(name: &str) -> Vec<u8> {
    let dir = fonts_dir();
    for n in [name, "poppins.semibold.ttf", "Banita.ttf", "Qilka-Bold copy.otf"] {
        if let Ok(b) = std::fs::read(dir.join(n)) {
            if !b.is_empty() {
                return b;
            }
        }
    }
    Vec::new()
}

fn font_vec(name: &str) -> Option<FontVec> {
    FontVec::try_from_vec(font_bytes(name)).ok()
}

fn rgba(px: &[u8]) -> (u8, u8, u8, u8) {
    (px[0], px[1], px[2], px[3])
}

pub fn draw_text(
    canvas: &mut RgbaImage,
    font: &FontVec,
    scale: f32,
    x: u32,
    y: u32,
    color: (u8, u8, u8, u8),
    text: &str,
) -> u32 {
    let sc = PxScale::from(scale);
    let scaled = font.as_scaled(sc);
    let top = y as f32 + scaled.ascent();
    let mut end = x;
    let mut caret = x as f32;
    for c in text.chars() {
        let id = scaled.glyph_id(c);
        let g = Glyph { id, scale: sc, position: ab_glyph::point(caret, top) };
        if let Some(o) = font.outline_glyph(g) {
            let b = o.px_bounds();
            end = end.max(b.max.x as u32);
            o.draw(|gx, gy, v| {
                let (px, py) = (b.min.x as u32 + gx, b.min.y as u32 + gy);
                if px < canvas.width() && py < canvas.height() {
                    let p = canvas.get_pixel_mut(px, py);
                    let a = (v * color.3 as f32) as u8;
                    let ia = 255 - a;
                    p[0] = ((p[0] as u16 * ia as u16 + color.0 as u16 * a as u16) / 255) as u8;
                    p[1] = ((p[1] as u16 * ia as u16 + color.1 as u16 * a as u16) / 255) as u8;
                    p[2] = ((p[2] as u16 * ia as u16 + color.2 as u16 * a as u16) / 255) as u8;
                    p[3] = 255;
                }
            });
        }
        caret += scaled.h_advance(id);
    }
    end
}

pub fn text_width(font: &FontVec, scale: f32, text: &str) -> u32 {
    let scaled = font.as_scaled(PxScale::from(scale));
    let mut w = 0f32;
    for c in text.chars() {
        w += scaled.h_advance(scaled.glyph_id(c));
    }
    w as u32
}

fn line_h(scale: f32) -> u32 {
    (scale + 12.0) as u32
}

pub fn wrap(font: &FontVec, scale: f32, text: &str, max_w: u32) -> Vec<String> {
    let mut lines = vec![];
    let mut cur = String::new();
    for word in text.split(' ') {
        let t = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if text_width(font, scale, &t) > max_w && !cur.is_empty() {
            lines.push(cur);
            cur = word.to_string();
        } else {
            cur = t;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

pub async fn fetch(url: &str) -> Result<(u16, Vec<u8>, String), String> {
    let r = net::http()
        .get(url)
        .header("User-Agent", net::pick_ua())
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let st = r.status().as_u16();
    let ct = r.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let b = r.bytes().await.map_err(|e| e.to_string())?.to_vec();
    Ok((st, b, ct))
}

pub fn decode(bytes: &[u8]) -> Option<RgbaImage> {
    image::load_from_memory(bytes).ok().map(|d| d.to_rgba8())
}

pub fn png_bytes(img: &RgbaImage) -> Vec<u8> {
    let mut b = Vec::new();
    let enc = image::codecs::png::PngEncoder::new(&mut b);
    use image::ImageEncoder;
    enc.write_image(img.as_raw(), img.width(), img.height(), image::ExtendedColorType::Rgba8).ok();
    b
}

fn b64(b: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn is_emoji(c: char) -> bool {
    matches!(c,
        '\u{1F300}'..='\u{1FAFF}' | '\u{2600}'..='\u{27BF}' | '\u{2B00}'..='\u{2BFF}'
        | '\u{2300}'..='\u{23FF}' | '\u{25A0}'..='\u{25FF}' | '\u{2190}'..='\u{21FF}')
}

async fn emoji_png_discord(id: &str, animated: bool) -> Option<RgbaImage> {
    let ext = if animated { "gif" } else { "png" };
    let url = format!("https://cdn.discordapp.com/emojis/{id}.{ext}?size=128");
    let (_, b, _) = fetch(&url).await.ok()?;
    decode(&b)
}

async fn emoji_png_unicode(s: &str) -> Option<RgbaImage> {
    let cp: Vec<String> = s.chars().filter(|c| *c != '\u{FE0F}').map(|c| format!("{:x}", c as u32)).collect();
    if cp.is_empty() {
        return None;
    }
    let url = format!("https://cdnjs.cloudflare.com/ajax/libs/twemoji/14.0.2/72x72/{}.png", cp.join("-"));
    let (_, b, _) = fetch(&url).await.ok()?;
    decode(&b)
}

fn paste(img: &mut RgbaImage, src: &RgbaImage, x: u32, y: u32) {
    let (dw, dh) = (img.width(), img.height());
    let (sw, sh) = (src.width(), src.height());
    ffi::paste(
        img.as_mut(),
        dw,
        dh,
        src.as_raw(),
        sw,
        sh,
        x as i64,
        y as i64,
    );
}

fn circle(img: &RgbaImage, size: u32) -> RgbaImage {
    let r = image::imageops::resize(img, size, size, image::imageops::FilterType::Lanczos3);
    let mut out = RgbaImage::new(size, size);
    ffi::circle_copy(out.as_mut(), r.as_raw(), size);
    out
}

fn fill(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, c: (u8, u8, u8, u8)) {
    let (iw, ih) = (img.width(), img.height());
    ffi::fill(
        img.as_mut(),
        iw,
        ih,
        x as i64,
        y as i64,
        w as i64,
        h as i64,
        [c.0, c.1, c.2, c.3],
    );
}

fn rounded(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, r: u32, c: (u8, u8, u8, u8)) {
    let (iw, ih) = (img.width(), img.height());
    ffi::rounded(
        img.as_mut(),
        iw,
        ih,
        [x as i64, y as i64, w as i64, h as i64],
        r as i32,
        [c.0, c.1, c.2, c.3],
    );
}

enum Tok {
    Text(String),
    Emoji(RgbaImage),
}

async fn tokenize(text: &str, emoji_size: u32) -> Vec<Tok> {
    let mut toks: Vec<(u8, String, bool)> = vec![];
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let rest: String = chars[i..].iter().collect();
        if rest.starts_with("<a:") || rest.starts_with("<:") {
            if let Some(end) = rest.find('>') {
                let inner = &rest[..end + 1];
                let parts: Vec<&str> = inner[1..inner.len() - 1].split(':').collect();
                if parts.len() == 3 && !parts[2].is_empty() {
                    toks.push((1, parts[2].to_string(), parts[0] == "a"));
                    i += inner.chars().count();
                    continue;
                }
            }
        }
        if is_emoji(chars[i]) {
            toks.push((2, chars[i].to_string(), false));
        } else {
            toks.push((0, chars[i].to_string(), false));
        }
        i += 1;
    }
    let mut merged: Vec<(u8, String, bool)> = vec![];
    for t in toks {
        if t.0 == 0 {
            if let Some(l) = merged.last_mut() {
                if l.0 == 0 {
                    l.1.push_str(&t.1);
                    continue;
                }
            }
        }
        merged.push(t);
    }
    let mut out = vec![];
    for (k, v, anim) in merged {
        if k == 0 {
            out.push(Tok::Text(v));
        } else {
            let img = if k == 1 { emoji_png_discord(&v, anim).await } else { emoji_png_unicode(&v).await };
            if let Some(e) = img {
                out.push(Tok::Emoji(image::imageops::resize(&e, emoji_size, emoji_size, image::imageops::FilterType::Lanczos3)));
            }
        }
    }
    out
}

async fn caption_bar(width: u32, text: &str, font_size: u32) -> RgbaImage {
    let font = font_vec("impact.ttf").expect("font");
    let toks = tokenize(text, font_size).await;
    let pad = 10u32;
    let mut lines: Vec<Vec<(u32, Tok2)>> = vec![];
    let mut cur: Vec<(u32, Tok2)> = vec![];
    let mut cx = 0u32;
    for t in toks {
        match t {
            Tok::Text(s) => {
                for (j, word) in s.split(' ').enumerate() {
                    let w = if j + 1 < s.split(' ').count() {
                        text_width(&font, font_size as f32, &format!("{word} "))
                    } else {
                        text_width(&font, font_size as f32, word)
                    };
                    if cx + w > width.saturating_sub(pad * 2) && !cur.is_empty() {
                        lines.push(cur);
                        cur = vec![];
                        cx = 0;
                    }
                    let ww = if j + 1 < s.split(' ').count() { format!("{word} ") } else { word.to_string() };
                    cur.push((w, Tok2::T(ww)));
                    cx += w;
                }
            }
            Tok::Emoji(e) => {
                let w = font_size + 2;
                if cx + w > width.saturating_sub(pad * 2) && !cur.is_empty() {
                    lines.push(cur);
                    cur = vec![];
                    cx = 0;
                }
                cur.push((w, Tok2::E(e)));
                cx += w;
            }
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    let lh = font_size + 10;
    let h = (lines.len() as u32) * lh + pad * 2;
    let mut bar = RgbaImage::from_pixel(width, h.max(10), Rgba([255, 255, 255, 255]));
    let mut y = pad;
    for line in &lines {
        let lw: u32 = line.iter().map(|(w, _)| w).sum();
        let mut x = width.saturating_sub(lw) / 2;
        for (w, t) in line {
            match t {
                Tok2::T(s) => {
                    draw_text(&mut bar, &font, font_size as f32, x, y, (0, 0, 0, 255), s);
                }
                Tok2::E(e) => {
                    paste(&mut bar, e, x, y + (lh - font_size) / 2);
                }
            }
            x += w;
        }
        y += lh;
    }
    bar
}

enum Tok2 {
    T(String),
    E(RgbaImage),
}

fn encode_gif(frames: Vec<(RgbaImage, u32)>) -> Option<Vec<u8>> {
    if frames.is_empty() {
        return None;
    }
    let mut b = Vec::new();
    {
        let mut enc = GifEncoder::new_with_speed(&mut b, 10);
        enc.set_repeat(Repeat::Infinite).ok()?;
        for (img, ms) in frames {
            enc.encode_frame(Frame::from_parts(img, 0, 0, Delay::from_numer_denom_ms(ms.max(20), 1))).ok()?;
        }
    }
    Some(b)
}

fn ffmpeg_bin() -> Option<String> {
    let mut cands = vec![];
    if let Ok(e) = std::env::var("IMAGEIO_FFMPEG_EXE") {
        cands.push(e);
    }
    cands.push("ffmpeg".to_string());
    for c in cands {
        if std::process::Command::new(&c).arg("-version").output().map(|o| o.status.success()).unwrap_or(false) {
            return Some(c);
        }
    }
    None
}

async fn ffmpeg_frames(input: &[u8]) -> Option<Vec<(RgbaImage, u32)>> {
    let dir = std::env::temp_dir().join(format!("cap_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)));
    std::fs::create_dir_all(&dir).ok()?;
    let inp = dir.join("in.mp4");
    std::fs::write(&inp, input).ok()?;
    let ff = ffmpeg_bin()?;
    let st = tokio::process::Command::new(ff)
        .args(["-y", "-i", inp.to_str()?, "-t", "30", "-vf", "fps=10,scale=480:-1", &format!("{}/f%03d.png", dir.display())])
        .output()
        .await
        .ok()?;
    if !st.status.success() {
        return None;
    }
    let mut paths: Vec<_> = std::fs::read_dir(&dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.extension().and_then(|e| e.to_str()) == Some("png")).collect();
    paths.sort();
    let mut out = vec![];
    for p in paths.into_iter().take(300) {
        if let Ok(b) = std::fs::read(&p) {
            if let Some(img) = decode(&b) {
                out.push((img, 100));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

pub async fn imgbb(data: &[u8], filename: &str, key: &str) -> Option<String> {
    let part = reqwest::multipart::Part::bytes(data.to_vec()).file_name(filename.to_string());
    let form = reqwest::multipart::Form::new().part("image", part);
    let url = format!("https://api.imgbb.com/1/upload?key={key}");
    let r = net::http().post(&url).multipart(form).send().await.ok()?;
    if r.status().as_u16() != 200 {
        return None;
    }
    let v: Value = r.json().await.ok()?;
    v.get("data").and_then(|d| d.get("url")).and_then(|u| u.as_str()).map(|s| s.to_string())
}

fn is_gif(b: &[u8]) -> bool {
    b.len() > 3 && b[0] == b'G' && b[1] == b'I' && b[2] == b'F'
}

fn is_mp4(b: &[u8]) -> bool {
    b.len() > 12 && &b[4..8] == b"ftyp"
}

pub async fn fun_caption(p: &Value) -> Value {
    let url = p.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let text = p.get("text").and_then(|v| v.as_str()).unwrap_or("");
    let key = p.get("imgbb_key").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string()).unwrap_or_else(|| crate::config::get("imgbb_key"));
    let (_, data, ct) = match fetch(url).await {
        Ok(t) if t.0 == 200 => t,
        _ => return json!({"error": "download failed"}),
    };
    let animated = is_gif(&data) || is_mp4(&data) || ct.contains("gif") || ct.contains("video") || ct.contains("mp4");
    let up_name = if animated { "captioned.gif" } else { "captioned.png" };
    if !animated {
        let png = match decode(&data) {
            Some(d) => d,
            None => return json!({"error": "decode failed"}),
        };
        let w = png.width().min(480);
        let h = (png.height() as u64 * w as u64 / png.width().max(1) as u64).max(1) as u32;
        let small = image::imageops::resize(&png, w, h, image::imageops::FilterType::Triangle);
        let fs = 20u32.max(80u32.min(small.width() / 12));
        let bar = caption_bar(small.width(), text, fs).await;
        let mut canvas = RgbaImage::from_pixel(small.width(), bar.height() + 20 + small.height(), Rgba([255, 255, 255, 255]));
        paste(&mut canvas, &bar, 0, 10);
        paste(&mut canvas, &small, 0, bar.height() + 20);
        match imgbb(&png_bytes(&canvas), up_name, &key).await {
            Some(u) => json!({"url": u}),
            None => json!({"error": "upload failed"}),
        }
    } else {
        let frames: Vec<(RgbaImage, u32)> = if is_gif(&data) {
            let d = GifDecoder::new(Cursor::new(&data)).ok();
            match d {
                Some(dec) => {
                    let (fw, fh) = dec.dimensions();
                    let w = fw.min(480);
                    let sc = w as f64 / fw.max(1) as f64;
                    let h = ((fh as f64 * sc) as u32).max(1);
                    let mut acc = 0u32;
                    let mut out = vec![];
                    for f in dec.into_frames().flatten() {
                        let (n, dd) = f.delay().numer_denom_ms();
                        let ms = if dd == 0 { 100 } else { (n * 1000 / dd).max(20) };
                        acc += ms;
                        if acc > 30000 {
                            break;
                        }
                        out.push((image::imageops::resize(f.buffer(), w, h, image::imageops::FilterType::Triangle), ms));
                    }
                    out
                }
                None => vec![],
            }
        } else {
            ffmpeg_frames(&data).await.unwrap_or_default()
        };
        if frames.is_empty() {
            return json!({"error": "no frames"});
        }
        let fs = (20u32).max((80u32).min(frames[0].0.width() / 12));
        let bar = caption_bar(frames[0].0.width(), text, fs).await;
        let bh = bar.height() + 20;
        let capped: Vec<(RgbaImage, u32)> = frames
            .into_iter()
            .map(|(f, ms)| {
                let mut c = RgbaImage::from_pixel(f.width(), f.height() + bh, Rgba([255, 255, 255, 255]));
                paste(&mut c, &bar, 0, 10);
                paste(&mut c, &f, 0, bh);
                (c, ms)
            })
            .collect();
        match encode_gif(capped).map(|g| g) {
            Some(gif) => match imgbb(&gif, up_name, &key).await {
                Some(u) => json!({"url": u}),
                None => json!({"error": "upload failed"}),
            },
            None => json!({"error": "encode failed"}),
        }
    }
}

pub async fn fun_img2gif(p: &Value) -> Value {
    let url = p.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let (_, data, _) = match fetch(url).await {
        Ok(t) if t.0 == 200 => t,
        _ => return json!({"error": "download failed"}),
    };
    let img = match decode(&data) {
        Some(d) => d,
        None => return json!({"error": "decode failed"}),
    };
    match encode_gif(vec![(img, 100)]) {
        Some(g) => json!({"data_b64": b64(&g), "media_type": "image/gif"}),
        None => json!({"error": "encode failed"}),
    }
}

fn looks_like_video(data: &[u8], content_type: &str) -> bool {
    if data.len() >= 12 && &data[4..8] == b"ftyp" {
        return true;
    }
    if data.len() >= 4 && (&data[..4] == b"\x1a\x45\xdf\xa3" || &data[..4] == b"RIFF") {
        return true;
    }
    if data.len() >= 4 && &data[..4] == b"OggS" {
        return true;
    }
    content_type.contains("video/")
        || content_type.contains("application/mp4")
        || content_type.contains("video/mp4")
}

async fn fetch_video_bytes(p: &Value, url: &str) -> Result<(Vec<u8>, String), (&'static str, String)> {
    let (_, first, ct) = match fetch(url).await {
        Ok(t) if t.0 == 200 => t,
        _ => return Err(("download failed", String::new())),
    };

    if looks_like_video(&first, &ct) {
        return Ok((first, ct));
    }

    let prefix = std::env::temp_dir().join(format!(
        "vid2gif_in_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let prefix = prefix.to_string_lossy().into_owned();
    let dl = crate::extract::download_video(p, &prefix, false).await;
    let path = dl.get("path").and_then(|v| v.as_str()).map(|s| s.to_string());
    match path {
        Some(path) => match std::fs::read(&path) {
            Ok(bytes) => {
                let _ = std::fs::remove_file(&path);
                if bytes.is_empty() {
                    Err(("extracted video is empty", String::new()))
                } else {
                    Ok((bytes, "video/mp4".to_string()))
                }
            }
            Err(e) => {
                let _ = std::fs::remove_file(&path);
                Err(("extracted read failed", e.to_string()))
            }
        },
        None => {
            let msg = dl.get("error").and_then(|e| e.as_str()).unwrap_or("video extraction failed");
            Err(("unsupported video source", msg.to_string()))
        }
    }
}

async fn ffmpeg_vid2gif(data: &[u8]) -> Option<Vec<u8>> {
    let ff = ffmpeg_bin()?;
    let data = data.to_vec();
    let mut child = tokio::process::Command::new(ff)
        .args([
            "-i",
            "pipe:0",
            "-t",
            "30",
            "-vf",
            "fps=12,scale=480:-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse",
            "-f",
            "gif",
            "pipe:1",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .ok()?;

    use tokio::io::AsyncWriteExt;

    let stdin = child.stdin.take();
    let writer = if let Some(mut stdin) = stdin {
        Some(tokio::spawn(async move {
            let _ = stdin.write_all(&data).await;
            let _ = stdin.flush().await;
            drop(stdin);
        }))
    } else {
        None
    };

    let out = match child.wait_with_output().await {
        Ok(o) if o.status.success() => o.stdout,
        Ok(o) => {
            let detail = String::from_utf8_lossy(&o.stderr);
            net::log("ffmpeg", &format!("vid2gif fail :: {}", detail.chars().take(300).collect::<String>()));
            if let Some(w) = writer {
                w.abort();
            }
            return None;
        }
        Err(_) => {
            if let Some(w) = writer {
                w.abort();
            }
            return None;
        }
    };

    if let Some(w) = writer {
        let _ = w.await;
    }

    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

pub async fn fun_vid2gif(p: &Value) -> Value {
    let url = p.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let key = p.get("imgbb_key").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string()).unwrap_or_else(|| crate::config::get("imgbb_key"));
    if url.is_empty() {
        return json!({"error": "url is required"});
    }

    let (data, _) = match fetch_video_bytes(p, url).await {
        Ok(v) => v,
        Err((msg, detail)) => {
            let mut e = msg.to_string();
            if !detail.is_empty() {
                e.push_str(": ");
                e.push_str(&detail);
            }
            return json!({"error": e});
        }
    };

    let native = {
        let data = data.clone();
        tokio::task::spawn_blocking(move || ffi::vid2gif_bytes(&data))
            .await
            .unwrap_or_default()
    };

    let out = if !native.is_empty() {
        native
    } else {
        match ffmpeg_vid2gif(&data).await {
            Some(v) if !v.is_empty() => v,
            _ => return json!({"error": "conversion failed"}),
        }
    };

    match imgbb(&out, "converted.gif", &key).await {
        Some(u) => json!({"url": u}),
        None => json!({"error": "upload failed"}),
    }
}

pub async fn fun_quote(p: &Value) -> Value {
    let white = p.get("color").and_then(|v| v.as_str()).unwrap_or("black") == "white";
    let flip = p.get("flip").and_then(|v| v.as_bool()).unwrap_or(false);
    let blur = p.get("blur").and_then(|v| v.as_bool()).unwrap_or(false);
    let gray = p.get("grayscale").and_then(|v| v.as_bool()).unwrap_or(false);
    let font_name = p.get("font").and_then(|v| v.as_str()).unwrap_or("poppins.semibold.ttf");
    let content = p.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let avatar_url = p.get("avatar_url").and_then(|v| v.as_str()).unwrap_or("");
    let (_, adata, _) = match fetch(avatar_url).await {
        Ok(t) if t.0 == 200 => t,
        _ => return json!({"error": "avatar download failed"}),
    };
    let avatar = match decode(&adata) {
        Some(a) => a,
        None => return json!({"error": "avatar decode failed"}),
    };
    let bgc: (u8, u8, u8) = if white { (255, 255, 255) } else { (0, 0, 0) };
    let fillc: (u8, u8, u8, u8) = if white { (0, 0, 0, 255) } else { (255, 255, 255, 255) };
    let font = match font_vec(font_name) {
        Some(f) => f,
        None => return json!({"error": "no font"}),
    };
    let w = 1000u32;
    let asz = 450u32;
    let taw = w - asz;
    let wrap_w = taw - 40;
    let mut size = 48f32;
    loop {
        if text_width(&font, size, content) <= wrap_w || size <= 18.0 {
            break;
        }
        size -= 2.0;
    }
    let lines = wrap(&font, size, content, wrap_w);
    let texth = lines.len() as u32 * line_h(size);
    let h = asz.max(texth + 80 + 70 + 120 + 40);
    let av = image::imageops::resize(&avatar, asz, h, image::imageops::FilterType::Lanczos3);
    let raw = av.into_raw();
    let mut grad = vec![0u8; raw.len()];
    ffi::gradient_avatar(&mut grad, &raw, asz, h, bgc, if white { 280 } else { 255 }, flip);
    if gray {
        // the old API grayscales the gradient layer only, matching its look
        ffi::grayscale(&mut grad, asz, h);
    }
    let mut img = RgbaImage::from_pixel(w, h, Rgba([bgc.0, bgc.1, bgc.2, 255]));
    let gx = if flip { (w as f32 / 2.0 - 20.0) as u32 } else { 0 };
    let gimg = RgbaImage::from_raw(asz, h, grad).unwrap();
    paste(&mut img, &gimg, gx.min(w - asz), 0);
    let tx = if flip { taw as f32 / 2.0 } else { asz as f32 + taw as f32 / 2.0 };
    let sy = h as f32 / 2.0 - texth as f32 / 2.0;
    let qf = font_vec("poppins.semibold.ttf").unwrap_or_else(|| font_vec(font_name).expect("font"));
    draw_text(&mut img, &qf, 180.0, (tx - wrap_w as f32 / 2.0 - 10.0).max(0.0) as u32, (sy - 140.0).max(0.0) as u32, (fillc.0, fillc.1, fillc.2, 40), "\u{201C}");
    let mut y = sy as u32;
    for line in &lines {
        let lw = text_width(&font, size, line);
        draw_text(&mut img, &font, size, (tx - lw as f32 / 2.0).max(0.0) as u32, y, rgba(&[fillc.0, fillc.1, fillc.2, fillc.3]), line);
        y += line_h(size);
    }
    let name = format!("- {}", p.get("display_name").and_then(|v| v.as_str()).unwrap_or(""));
    let nw = text_width(&qf, 32.0, &name);
    draw_text(&mut img, &qf, 32.0, (tx - nw as f32 / 2.0).max(0.0) as u32, y + 10, rgba(&[fillc.0, fillc.1, fillc.2, fillc.3]), &name);
    let uname = format!("@{}", p.get("username").and_then(|v| v.as_str()).unwrap_or(""));
    let uw = text_width(&qf, 22.0, &uname);
    draw_text(&mut img, &qf, 22.0, (tx - uw as f32 / 2.0).max(0.0) as u32, y + 55, (89, 89, 89, 255), &uname);
    let (iw, ih) = (img.width(), img.height());
    let raw = img.into_raw();
    let mut out = RgbaImage::from_raw(iw, ih, raw).unwrap();

    if blur {
        // old API: ImageFilter.GaussianBlur(3)
        out = image::imageops::blur(&out, 3.0);
    }

    json!({"image": b64(&png_bytes(&out))})
}

async fn fetch_avatar(url: &str) -> Option<RgbaImage> {
    let (_, b, _) = fetch(url).await.ok()?;
    if b.is_empty() {
        return None;
    }
    decode(&b)
}

fn lev_font(bold: bool) -> FontVec {
    font_vec(if bold { "Qilka-Bold copy.otf" } else { "poppins.semibold.ttf" }).expect("font")
}

pub async fn lev_card(p: &Value) -> Value {
    let (w, h) = (800u32, 120u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([14, 14, 17, 255]));
    rounded(&mut img, 5, 5, w - 10, h - 10, 25, (255, 255, 255, 255));
    let (nf, lf, xf, pf) = (lev_font(true), lev_font(true), lev_font(false), lev_font(false));
    let username = p.get("username").and_then(|v| v.as_str()).unwrap_or("");
    let uname = if username.len() > 20 { format!("{}...", &username[..17]) } else { username.to_string() };
    let level = p.get("level").and_then(|v| v.as_u64()).unwrap_or(0);
    let xp = p.get("xp").and_then(|v| v.as_u64()).unwrap_or(0);
    let pxp = p.get("progress_xp").and_then(|v| v.as_u64()).unwrap_or(0);
    let nxp = p.get("needed_xp").and_then(|v| v.as_u64()).unwrap_or(1).max(1);
    if let Some(av) = fetch_avatar(p.get("avatar_url").and_then(|v| v.as_str()).unwrap_or("")).await {
        paste(&mut img, &circle(&av, 80), 25, (h - 80) / 2);
    }
    let tx = 25 + 80 + 25;
    draw_text(&mut img, &nf, 26.0, tx, 20, (25, 25, 30, 255), &uname);
    draw_text(&mut img, &lf, 32.0, tx, 50, (25, 25, 30, 255), &format!("Level {level}"));
    draw_text(&mut img, &xf, 18.0, tx, 85, (110, 110, 120, 255), &format!("{xp} XP"));
    let (pw, ph, px, py) = (250u32, 20u32, w - 250 - 30, 30u32);
    rounded(&mut img, px, py, pw, ph, 10, (205, 205, 212, 255));
    let pct = pxp as f64 / nxp as f64;
    let fw = (pw as f64 * pct) as u32;
    if fw > 0 {
        rounded(&mut img, px, py, fw, ph, 10, (25, 25, 30, 255));
    }
    draw_text(&mut img, &pf, 14.0, px, py + ph + 8, (110, 110, 120, 255), &format!("{pxp} / {nxp} XP ({:.1}%)", pct * 100.0));
    json!({"data_b64": b64(&png_bytes(&img)), "media_type": "image/png"})
}

pub async fn lev_levelup(p: &Value) -> Value {
    let (w, h) = (800u32, 150u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([14, 14, 17, 255]));
    rounded(&mut img, 5, 5, w - 10, h - 10, 25, (255, 255, 255, 255));
    let (tf, lf, sf) = (lev_font(true), lev_font(true), lev_font(false));
    let username = p.get("username").and_then(|v| v.as_str()).unwrap_or("");
    let uname = if username.len() > 15 { format!("{}...", &username[..12]) } else { username.to_string() };
    let (ol, nl) = (p.get("old_level").and_then(|v| v.as_u64()).unwrap_or(0), p.get("new_level").and_then(|v| v.as_u64()).unwrap_or(0));
    if let Some(av) = fetch_avatar(p.get("avatar_url").and_then(|v| v.as_str()).unwrap_or("")).await {
        paste(&mut img, &circle(&av, 100), 25, (h - 100) / 2);
    }
    let tx = 25 + 100 + 30;
    let title = p.get("custom_title").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| format!("{uname} LEVELED UP!"));
    draw_text(&mut img, &tf, 28.0, tx, 20, (25, 25, 30, 255), &title);
    let a = format!("Level {ol}");
    let ex = draw_text(&mut img, &lf, 36.0, tx, 65, (25, 25, 30, 255), &a);
    let ex2 = draw_text(&mut img, &lf, 36.0, ex + 15, 65, (25, 25, 30, 255), ">");
    draw_text(&mut img, &lf, 36.0, ex2 + 15, 65, (25, 25, 30, 255), &format!("Level {nl}"));
    draw_text(&mut img, &sf, 18.0, tx, 110, (110, 110, 120, 255), p.get("custom_subtitle").and_then(|v| v.as_str()).unwrap_or("Congratulations on your achievement!"));
    json!({"data_b64": b64(&png_bytes(&img)), "media_type": "image/png"})
}

pub async fn lev_leaderboard(p: &Value) -> Value {
    let entries = p.get("entries").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let (w, eh, pad) = (600u32, 50u32, 20u32);
    let h = (entries.len() as u32) * (eh + 10) + pad * 2 + 10;
    let mut img = RgbaImage::from_pixel(w, h.max(1), Rgba([0, 0, 0, 0]));
    let (nfr, lfr, rfr) = (lev_font(true), lev_font(true), lev_font(true));
    let mut avatars = vec![];
    for e in &entries {
        avatars.push(fetch_avatar(e.get("avatar_url").and_then(|v| v.as_str()).unwrap_or("")).await);
    }
    for (i, e) in entries.iter().enumerate() {
        let y = pad + (i as u32) * (eh + 10);
        let rank = e.get("rank").and_then(|v| v.as_u64()).unwrap_or(0);
        let c: (u8, u8, u8, u8) = match rank {
            1 => (25, 25, 30, 255),
            2 => (90, 90, 100, 255),
            3 => (150, 150, 160, 255),
            _ => (25, 25, 30, 255),
        };
        rounded(&mut img, 10, y, w - 20, eh, 8, (255, 255, 255, 255));
        draw_text(&mut img, &rfr, 20.0, 20 + 35 + 15, y + 5, c, &format!("{rank}"));
        let username = e.get("username").and_then(|v| v.as_str()).unwrap_or("");
        let uname = if username.len() > 20 { format!("{}...", &username[..17]) } else { username.to_string() };
        if let Some(av) = &avatars[i] {
            paste(&mut img, &circle(av, 35), 20, y + (eh - 35) / 2);
        }
        draw_text(&mut img, &nfr, 22.0, 20 + 35 + 15, y + 25, (25, 25, 30, 255), &uname);
        let xp = e.get("xp").and_then(|v| v.as_u64()).unwrap_or(0);
        let lvl = e.get("level").and_then(|v| v.as_u64()).unwrap_or(0);
        let lt = format!("Level {lvl}");
        let xt = if xp < 1000 { format!("{xp} XP") } else { format!("{:.1}K XP", xp as f64 / 1000.0) };
        let lw = text_width(&lfr, 18.0, &lt);
        let xw = text_width(&lfr, 18.0, &xt);
        draw_text(&mut img, &lfr, 18.0, w - lw - 20, y + 5, c, &lt);
        draw_text(&mut img, &lfr, 18.0, w - xw - 20, y + 25, (110, 110, 120, 255), &xt);
    }
    json!({"data_b64": b64(&png_bytes(&img)), "media_type": "image/png"})
}

pub async fn stats_dashboard(p: &Value) -> Value {
    let (w, pad) = (1200u32, 28u32);
    let tables = p.get("tables").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let middle = p.get("middle").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let mut tiles: Vec<Value> = vec![];
    for card in &middle {
        for row in card.get("rows").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
            if tiles.len() < 6 {
                tiles.push(row);
            }
        }
    }
    let mut avatars = vec![];
    for t in &tables {
        for r in t.get("rows").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
            let u = r.get("avatar_url").and_then(|v| v.as_str()).unwrap_or("");
            if u.is_empty() {
                avatars.push(None);
            } else {
                avatars.push(fetch_avatar(u).await.map(|a| circle(&a, 40)));
            }
        }
    }
    let table_hs: Vec<u32> = tables.iter().map(|t| 110 + (t.get("rows").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(1).max(1) as u32) * 82).collect();
    let strip = if !p.get("lookback_value").and_then(|v| v.as_str()).unwrap_or("").is_empty() || !tiles.is_empty() { 170u32 } else { 0 };
    let h = pad + 118 + 10 + (if strip > 0 { strip + pad } else { 0 }) + table_hs.iter().map(|x| x + pad).sum::<u32>() + 56 + pad;
    let mut img = RgbaImage::from_pixel(w, h, Rgba([14, 14, 17, 255]));
    let rf = lev_font(false);
    let title_f = lev_font(true);
    let sub_f = lev_font(false);
    let tag_f = lev_font(true);
    draw_text(&mut img, &tag_f, 18.0, pad + 4 + 14, pad + 6 + 8, (60, 60, 68, 255), "SERVER ACTIVITY");
    draw_text(&mut img, &title_f, 46.0, pad + 4, pad + 6 + 18 + 16 + 8, (205, 205, 214, 255), &p.get("title").and_then(|v| v.as_str()).unwrap_or("")[..p.get("title").and_then(|v| v.as_str()).unwrap_or("").len().min(30)]);
    if let Some(sub) = p.get("subtitle").and_then(|v| v.as_str()) {
        if !sub.is_empty() {
            let s = &sub[..sub.len().min(50)];
            let sw = text_width(&sub_f, 22.0, s);
            draw_text(&mut img, &sub_f, 22.0, w - pad - 4 - sw, pad + 6 + 18 + 16 + 16, (175, 175, 185, 255), s);
        }
    }
    let mut y = pad + 118 + 22;
    fill(&mut img, pad, pad + 118, w - pad * 2, 5, (90, 90, 100, 255));
    if strip > 0 {
        rounded(&mut img, pad, y, 300, strip - 12, 18, (255, 255, 255, 255));
        draw_text(&mut img, &lev_font(true), 19.0, pad + 22, y + 14, (175, 175, 185, 255), &p.get("lookback_label").and_then(|v| v.as_str()).unwrap_or("")[..p.get("lookback_label").and_then(|v| v.as_str()).unwrap_or("").len().min(22)]);
        draw_text(&mut img, &lev_font(true), 64.0, pad + 22, y + 44, (25, 25, 30, 255), p.get("lookback_value").and_then(|v| v.as_str()).unwrap_or(""));
        if !tiles.is_empty() {
            let gap = 16u32;
            let rest = w - pad * 2 - 300 - gap;
            let tw = rest / tiles.len().max(1) as u32;
            for (i, row) in tiles.iter().enumerate() {
                let tx = pad + 300 + gap + i as u32 * tw;
                rounded(&mut img, tx, y, tw.saturating_sub(gap), strip - 12, 18, (255, 255, 255, 255));
                draw_text(&mut img, &lev_font(true), 19.0, tx + 18, y + 14, (175, 175, 185, 255), &row.get("pill").and_then(|v| v.as_str()).unwrap_or("")[..row.get("pill").and_then(|v| v.as_str()).unwrap_or("").len().min(14)]);
                draw_text(&mut img, &lev_font(true), 34.0, tx + 18, y + 42, (25, 25, 30, 255), &row.get("value").and_then(|v| v.as_str()).unwrap_or("")[..row.get("value").and_then(|v| v.as_str()).unwrap_or("").len().min(10)]);
                if let Some(u) = row.get("unit").and_then(|v| v.as_str()) {
                    if !u.is_empty() {
                        draw_text(&mut img, &rf, 20.0, tx + 18, y + 84, (145, 145, 155, 255), &u[..u.len().min(12)]);
                    }
                }
            }
        }
        y += strip;
    }
    let mut ai = 0usize;
    for (ti, table) in tables.iter().enumerate() {
        let th = table_hs[ti];
        rounded(&mut img, pad, y, w - pad * 2, th, 18, (255, 255, 255, 255));
        draw_text(&mut img, &lev_font(true), 26.0, pad + 22, y + 14, (25, 25, 30, 255), &table.get("title").and_then(|v| v.as_str()).unwrap_or("")[..table.get("title").and_then(|v| v.as_str()).unwrap_or("").len().min(36)]);
        let rows = table.get("rows").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let tmax: f64 = rows.iter().map(|r| r.get("raw").and_then(|v| v.as_f64()).unwrap_or(0.0)).fold(0.0, f64::max).max(1.0);
        let mut ry = y + 62;
        for (ri, row) in rows.iter().enumerate() {
            rounded(&mut img, pad + 20, ry + 6, 42, 42, 10, (30, 30, 34, 255));
            let rn = format!("{}", ri + 1);
            let nw = text_width(&lev_font(true), 22.0, &rn);
            draw_text(&mut img, &lev_font(true), 22.0, pad + 41 - nw / 2, ry + 12, (222, 222, 230, 255), &rn);
            let mut lx = pad + 74;
            if ai < avatars.len() {
                if let Some(av) = &avatars[ai] {
                    paste(&mut img, av, lx, ry + 8);
                    lx += 50;
                }
                ai += 1;
            }
            let label = row.get("label").and_then(|v| v.as_str()).unwrap_or("");
            draw_text(&mut img, &lev_font(true), 24.0, lx + 6, ry + 6, (25, 25, 30, 255), &label[..label.len().min(24)]);
            if let Some(avg) = row.get("avg").and_then(|v| v.as_str()) {
                if !avg.is_empty() {
                    draw_text(&mut img, &rf, 20.0, lx + 6, ry + 34, (145, 145, 155, 255), &format!("avg {avg}/day"));
                }
            }
            let frac = (row.get("raw").and_then(|v| v.as_f64()).unwrap_or(0.0) / tmax).clamp(0.0, 1.0);
            let (bx, by) = (lx + 6, ry + 62);
            let bw = (w - pad * 2).saturating_sub(bx - pad + 220);
            rounded(&mut img, bx, by, bw, 10, 5, (200, 200, 208, 255));
            if frac > 0.0 {
                rounded(&mut img, bx, by, ((bw as f64 * frac) as u32).max(12), 10, 5, (30, 30, 34, 255));
            }
            let val = row.get("value").and_then(|v| v.as_str()).unwrap_or("");
            let vw = text_width(&lev_font(true), 28.0, val);
            let unit = row.get("unit").and_then(|v| v.as_str()).unwrap_or("");
            let uw = text_width(&rf, 20.0, unit);
            let vx = w - pad - 24 - vw - (if unit.is_empty() { 0 } else { uw + 8 });
            draw_text(&mut img, &lev_font(true), 28.0, vx, ry + 10, (25, 25, 30, 255), val);
            if !unit.is_empty() {
                draw_text(&mut img, &rf, 20.0, vx + vw + 8, ry + 20, (145, 145, 155, 255), unit);
            }
            ry += 82;
        }
        y += th + pad;
    }
    draw_text(&mut img, &rf, 20.0, pad + 4, y + 16, (168, 168, 180, 255), &p.get("footer_left").and_then(|v| v.as_str()).unwrap_or("")[..p.get("footer_left").and_then(|v| v.as_str()).unwrap_or("").len().min(80)]);
    json!({"data_b64": b64(&png_bytes(&img)), "media_type": "image/png"})
}

pub async fn util_zip(p: &Value) -> Value {
    let assets = p.get("assets").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let name = p.get("zip_filename").and_then(|v| v.as_str()).unwrap_or("assets.zip").to_string();
    let mut jobs = vec![];
    for a in assets.into_iter().take(100) {
        let url = a.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let fname = a.get("filename").and_then(|v| v.as_str()).unwrap_or("file").to_string();
        jobs.push(tokio::spawn(async move {
            let r: Option<(String, Vec<u8>)> = async {
                let (_, b, _) = fetch(&url).await.ok()?;
                Some((fname, b))
            }
            .await;
            r
        }));
    }
    let mut buf = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for j in jobs {
            if let Ok(Some((fname, data))) = j.await {
                if zip.start_file(fname, opts).is_ok() {
                    use std::io::Write;
                    let _ = zip.write_all(&data);
                }
            }
        }
        let _ = zip.finish();
    }
    json!({"data_b64": b64(buf.get_ref()), "filename": name, "media_type": "application/zip"})
}

pub async fn util_dominant(p: &Value) -> Value {
    let url = p.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let color = match fetch(url).await {
        Ok((200, b, _)) => {
            let small = if b.len() > 1_000_000 { ffi::vips_thumbnail(&b, 512).unwrap_or(b) } else { b };
            match decode(&small) {
                Some(img) => ffi::dominant(img.as_raw(), img.width(), img.height()),
                None => 0x2B2D31,
            }
        }
        _ => 0x2B2D31,
    };
    json!({"color": color, "hex": format!("#{:06x}", color)})
}

pub fn util_svg(p: &Value) -> Value {
    let code = p.get("code").and_then(|v| v.as_str()).unwrap_or("");
    let tree = match resvg::usvg::Tree::from_str(code, &resvg::usvg::Options::default()) {
        Ok(t) => t,
        Err(_) => return json!({"error": "bad svg"}),
    };
    let size = tree.size();
    let (w, h) = (size.width().max(1.0) as u32, size.height().max(1.0) as u32);
    let mut pix = match resvg::tiny_skia::Pixmap::new(w, h) {
        Some(px) => px,
        None => return json!({"error": "pixmap failed"}),
    };
    resvg::render(&tree, resvg::usvg::Transform::default(), &mut pix.as_mut());
    match pix.encode_png() {
        Ok(b) => json!({"data_b64": b64(&b), "media_type": "image/png"}),
        Err(_) => json!({"error": "encode failed"}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wrap_basic() {
        let f = font_vec("impact.ttf").unwrap();
        let l = wrap(&f, 40.0, "hello world foo bar", 50);
        assert!(l.len() > 1);
    }
    #[test]
    fn text_width_sane() {
        let f = font_vec("impact.ttf").unwrap();
        assert!(text_width(&f, 40.0, "hello") > text_width(&f, 40.0, "hi"));
    }
}
