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
