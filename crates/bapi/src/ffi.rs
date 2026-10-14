use std::os::raw::c_char;

#[repr(C)]
pub struct Buf {
    pub data: *mut u8,
    pub len: usize,
}

unsafe extern "C" {
    fn domcolor(rgba: *const u8, w: i32, h: i32) -> u32;
    fn grayinplace(rgba: *mut u8, w: i32, h: i32);
    fn boxblurinplace(rgba: *mut u8, w: i32, h: i32);
    fn gradavatar(
        dst: *mut u8,
        avatar: *const u8,
        w: i32,
        h: i32,
        br: u8,
        bg: u8,
        bb: u8,
        transparency: i32,
        flip: i32,
    );
    fn phash64(rgba: *const u8, w: i32, h: i32) -> u64;
    fn hammingmatch(hash: u64, table: *const u64, n: usize, max_dist: i32) -> i32;
    fn ocrtext(img: *const u8, len: usize, tessdata: *const c_char) -> Buf;
    fn vthumbnail(img: *const u8, len: usize, width: i32) -> Buf;
    fn vid2gif(data: *const u8, len: usize) -> Buf;
    fn c_paste_rgba(
        dst: *mut u8,
        dw: i32,
        dh: i32,
        src: *const u8,
        sw: i32,
        sh: i32,
        x: i32,
        y: i32,
    );
    fn c_fill_rgba(
        p: *mut u8,
        w: i32,
        h: i32,
        x: i32,
        y: i32,
        rw: i32,
        rh: i32,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    );
    fn c_rounded_rgba(
        p: *mut u8,
        w: i32,
        h: i32,
        x: i32,
        y: i32,
        rw: i32,
        rh: i32,
        rad: i32,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    );
    fn c_circle_copy(dst: *mut u8, src: *const u8, size: i32);
    fn buffree(b: Buf);
}

fn take(b: Buf) -> Vec<u8> {
    if b.data.is_null() || b.len == 0 {
        return Vec::new();
    }
    let v = unsafe { std::slice::from_raw_parts(b.data, b.len).to_vec() };
    unsafe { buffree(b) };
    v
}


pub fn dominant(rgba: &[u8], w: u32, h: u32) -> u32 {
    assert!(rgba.len() >= (w as usize) * (h as usize) * 4);
    unsafe { domcolor(rgba.as_ptr(), w as i32, h as i32) }
}

pub fn grayscale(rgba: &mut [u8], w: u32, h: u32) {
    assert!(rgba.len() >= (w as usize) * (h as usize) * 4);
    unsafe { grayinplace(rgba.as_mut_ptr(), w as i32, h as i32) }
}

pub fn box_blur(rgba: &mut [u8], w: u32, h: u32) {
    assert!(rgba.len() >= (w as usize) * (h as usize) * 4);
    unsafe { boxblurinplace(rgba.as_mut_ptr(), w as i32, h as i32) }
}

pub fn gradient_avatar(
    dst: &mut [u8],
    avatar: &[u8],
    w: u32,
    h: u32,
    rgb: (u8, u8, u8),
    transparency: i32,
    flip: bool,
) {
    assert!(dst.len() >= avatar.len());
    unsafe {
        gradavatar(
            dst.as_mut_ptr(),
            avatar.as_ptr(),
            w as i32,
            h as i32,
            rgb.0,
            rgb.1,
            rgb.2,
            transparency,
            flip as i32,
        )
    }
}

pub fn phash(rgba: &[u8], w: u32, h: u32) -> u64 {
    assert!(rgba.len() >= (w as usize) * (h as usize) * 4);
    unsafe { phash64(rgba.as_ptr(), w as i32, h as i32) }
}

pub fn hamming(hash: u64, table: &[u64], max_dist: i32) -> bool {
    unsafe { hammingmatch(hash, table.as_ptr(), table.len(), max_dist) == 1 }
}

pub fn hamming_batch(hash: u64, table: &[u64], max_dist: i32) -> bool {
    hamming(hash, table, max_dist)
}

pub fn ocr(png: &[u8]) -> String {
    let b = unsafe { ocrtext(png.as_ptr(), png.len(), std::ptr::null()) };
    let v = take(b);
    String::from_utf8_lossy(&v).into_owned()
}

pub fn vips_thumbnail(img: &[u8], width: u32) -> Option<Vec<u8>> {
    let b = unsafe { vthumbnail(img.as_ptr(), img.len(), width as i32) };
    let v = take(b);
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

pub fn vid2gif_bytes(data: &[u8]) -> Vec<u8> {
    let b = unsafe { vid2gif(data.as_ptr(), data.len()) };
    take(b)
}

pub fn paste(
    dst: &mut [u8],
    dw: u32,
    dh: u32,
    src: &[u8],
    sw: u32,
    sh: u32,
    x: i64,
    y: i64,
) {
    assert!(dst.len() >= (dw as usize) * (dh as usize) * 4);
    assert!(src.len() >= (sw as usize) * (sh as usize) * 4);
    unsafe {
        c_paste_rgba(
            dst.as_mut_ptr(),
            dw as i32,
            dh as i32,
            src.as_ptr(),
            sw as i32,
            sh as i32,
            x.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            y.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
        )
    }
}

pub fn fill(buf: &mut [u8], w: u32, h: u32, x: i64, y: i64, rw: i64, rh: i64, rgba: [u8; 4]) {
    assert!(buf.len() >= (w as usize) * (h as usize) * 4);
    unsafe {
        c_fill_rgba(
            buf.as_mut_ptr(),
            w as i32,
            h as i32,
            x.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            y.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            rw.clamp(0, i32::MAX as i64) as i32,
            rh.clamp(0, i32::MAX as i64) as i32,
            rgba[0],
            rgba[1],
            rgba[2],
            rgba[3],
        )
    }
}

pub fn rounded(buf: &mut [u8], w: u32, h: u32, rect: [i64; 4], rad: i32, rgba: [u8; 4]) {
    assert!(buf.len() >= (w as usize) * (h as usize) * 4);
    let [x, y, rw, rh] = rect;
    unsafe {
        c_rounded_rgba(
            buf.as_mut_ptr(),
            w as i32,
            h as i32,
            x.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            y.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            rw.clamp(0, i32::MAX as i64) as i32,
            rh.clamp(0, i32::MAX as i64) as i32,
            rad.max(0),
            rgba[0],
            rgba[1],
            rgba[2],
            rgba[3],
        )
    }
}

pub fn circle_copy(dst: &mut [u8], src: &[u8], size: u32) {
    assert!(dst.len() >= (size as usize) * (size as usize) * 4);
    assert!(src.len() >= (size as usize) * (size as usize) * 4);
    unsafe { c_circle_copy(dst.as_mut_ptr(), src.as_ptr(), size as i32) }
}
