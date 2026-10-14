#include "bimg.h"

#include <cctype>
#include <string>
#include <cmath>
#include <cstdlib>
#include <cstring>

uint32_t domcolor(const uint8_t* rgba, int w, int h) {
    if (!rgba || w <= 0 || h <= 0) return 0x2B2D31;
    static uint32_t bins[4096];
    memset(bins, 0, sizeof(bins));
    long total = (long)w * h;
    long step = total > 4096 ? total / 4096 : 1;
    uint64_t rs = 0, gs = 0, bs = 0, n = 0;
    uint32_t best = 0;
    for (long i = 0; i < total; i += step) {
        uint8_t r = rgba[i * 4], g = rgba[i * 4 + 1], b = rgba[i * 4 + 2], a = rgba[i * 4 + 3];
        if (a < 128) continue;
        uint32_t bin = ((r >> 4) << 8) | ((g >> 4) << 4) | (b >> 4);
        if (++bins[bin] > bins[best]) best = bin;
        rs += r; gs += g; bs += b; n++;
    }
    if (!n) return 0x2B2D31;
    uint8_t br = (best >> 8) & 0xF, bg = (best >> 4) & 0xF, bb = best & 0xF;
    uint8_t mr = rs / n, mg = gs / n, mb = bs / n;
    uint8_t r = (uint8_t)((mr + (br * 17)) / 2), g = (uint8_t)((mg + (bg * 17)) / 2),
            b = (uint8_t)((mb + (bb * 17)) / 2);
    return ((uint32_t)r << 16) | ((uint32_t)g << 8) | b;
}

void grayinplace(uint8_t* rgba, int w, int h) {
    long total = (long)w * h;
    for (long i = 0; i < total; i++) {
        uint8_t* p = rgba + i * 4;
        uint8_t y = (uint8_t)((p[0] * 77 + p[1] * 150 + p[2] * 29) >> 8);
        p[0] = p[1] = p[2] = y;
    }
}

void boxblurinplace(uint8_t* rgba, int w, int h) {
    if (w < 2 || h < 2) return;
    size_t row = (size_t)w * 4;
    uint8_t* tmp = (uint8_t*)malloc(row * h);
    if (!tmp) return;
    for (int y = 0; y < h; y++)
        for (int x = 0; x < w; x++)
            for (int c = 0; c < 4; c++) {
                int x0 = x > 0 ? x - 1 : x, x1 = x < w - 1 ? x + 1 : x;
                tmp[(y * w + x) * 4 + c] =
                    (uint8_t)((rgba[(y * w + x0) * 4 + c] + 2 * rgba[(y * w + x) * 4 + c] +
                               rgba[(y * w + x1) * 4 + c] + 2) / 4);
            }
    for (int y = 0; y < h; y++)
        for (int x = 0; x < w; x++)
            for (int c = 0; c < 4; c++) {
                int y0 = y > 0 ? y - 1 : y, y1 = y < h - 1 ? y + 1 : y;
                rgba[(y * w + x) * 4 + c] =
                    (uint8_t)((tmp[(y0 * w + x) * 4 + c] + 2 * tmp[(y * w + x) * 4 + c] +
                               tmp[(y1 * w + x) * 4 + c] + 2) / 4);
            }
    free(tmp);
}

void gradavatar(uint8_t* dst, const uint8_t* avatar, int w, int h,
                           uint8_t br, uint8_t bg, uint8_t bb,
                           int transparency, int flip) {
    if (transparency < 0) transparency = 0;
    if (transparency > 280) transparency = 280;
    for (int y = 0; y < h; y++)
        for (int x = 0; x < w; x++) {
            int alpha;
            if (flip) {
                alpha = transparency - x;
            } else {
                alpha = transparency - (w - 1 - x);
            }
            if (alpha < 0) alpha = 0;
            if (alpha > 255) alpha = 255;
            const uint8_t* s = avatar + ((size_t)y * w + x) * 4;
            uint8_t* d = dst + ((size_t)y * w + x) * 4;
            d[0] = (uint8_t)((s[0] * (255 - alpha) + br * alpha) / 255);
            d[1] = (uint8_t)((s[1] * (255 - alpha) + bg * alpha) / 255);
            d[2] = (uint8_t)((s[2] * (255 - alpha) + bb * alpha) / 255);
            d[3] = s[3];
        }
}


static inline void paste_blend(uint8_t* d, const uint8_t* s) {
    uint16_t a = s[3], ia = (uint16_t)(255 - a);
    d[0] = (uint8_t)((d[0] * ia + s[0] * a) / 255);
    d[1] = (uint8_t)((d[1] * ia + s[1] * a) / 255);
    d[2] = (uint8_t)((d[2] * ia + s[2] * a) / 255);
    d[3] = 255;
}

void c_paste_rgba(uint8_t* dst, int dw, int dh, const uint8_t* src, int sw, int sh, int x, int y) {
    if (!dst || !src || dw <= 0 || dh <= 0 || sw <= 0 || sh <= 0) return;
    for (int sy = 0; sy < sh; sy++) {
        int dy = y + sy;
        if (dy < 0 || dy >= dh) continue;
        for (int sx = 0; sx < sw; sx++) {
            int dx = x + sx;
            if (dx < 0 || dx >= dw) continue;
            const uint8_t* s = src + ((size_t)sy * sw + sx) * 4;
            if (s[3] < 10) continue;
            paste_blend(dst + ((size_t)dy * dw + dx) * 4, s);
        }
    }
}

void c_fill_rgba(uint8_t* p, int w, int h, int x, int y, int rw, int rh,
               uint8_t r, uint8_t g, uint8_t b, uint8_t a) {
    if (!p || w <= 0 || h <= 0 || rw <= 0 || rh <= 0) return;
    int x0 = x < 0 ? 0 : x;
    int y0 = y < 0 ? 0 : y;
    int x1 = x + rw;
    int y1 = y + rh;
    if (x1 > w) x1 = w;
    if (y1 > h) y1 = h;
    for (int yy = y0; yy < y1; yy++) {
        uint8_t* row = p + (size_t)yy * w * 4;
        for (int xx = x0; xx < x1; xx++) {
            uint8_t* px = row + (size_t)xx * 4;
            px[0] = r; px[1] = g; px[2] = b; px[3] = a;
        }
    }
}

static void disc_fill(uint8_t* p, int w, int h, int cx, int cy, int rad,
                      uint8_t r, uint8_t g, uint8_t b, uint8_t a) {
    if (!p || rad <= 0) return;
    for (int dy = -rad; dy <= rad; dy++) {
        for (int dx = -rad; dx <= rad; dx++) {
            if (dx * dx + dy * dy > rad * rad) continue;
            int x = cx + dx;
            int y = cy + dy;
            if (x < 0 || y < 0 || x >= w || y >= h) continue;
            uint8_t* px = p + ((size_t)y * w + x) * 4;
            px[0] = r; px[1] = g; px[2] = b; px[3] = a;
        }
    }
}

void c_rounded_rgba(uint8_t* p, int w, int h, int x, int y, int rw, int rh, int rad,
                  uint8_t r, uint8_t g, uint8_t b, uint8_t a) {
    if (!p || w <= 0 || h <= 0 || rw <= 0 || rh <= 0) return;
    if (rad < 0) rad = 0;
    int rr = rad;
    if (rr > rw / 2) rr = rw / 2;
    if (rr > rh / 2) rr = rh / 2;
    c_fill_rgba(p, w, h, x + rr, y, rw - 2 * rr, rh, r, g, b, a);
    c_fill_rgba(p, w, h, x, y + rr, rw, rh - 2 * rr, r, g, b, a);
    if (rr <= 0) return;
    int cxs[2] = {x + rr, x + rw - rr - 1};
    int cys[2] = {y + rr, y + rh - rr - 1};
    for (int cx : cxs)
        for (int cy : cys)
            disc_fill(p, w, h, cx, cy, rr, r, g, b, a);
}

void c_circle_copy(uint8_t* dst, const uint8_t* src, int size) {
    if (!dst || !src || size <= 0) return;
    float c = size / 2.0f;
    float c2 = c * c;
    for (int y = 0; y < size; y++) {
        for (int x = 0; x < size; x++) {
            float dx = x - c + 0.5f;
            float dy = y - c + 0.5f;
            uint8_t* d = dst + ((size_t)y * size + x) * 4;
            if (dx * dx + dy * dy <= c2) {
                const uint8_t* s = src + ((size_t)y * size + x) * 4;
                d[0] = s[0]; d[1] = s[1]; d[2] = s[2]; d[3] = s[3];
            } else {
                d[0] = d[1] = d[2] = d[3] = 0;
            }
        }
    }
}


static const char B64T[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

char* b64encode(const uint8_t* data, size_t len) {
    size_t outlen = 4 * ((len + 2) / 3) + 1;
    char* out = (char*)malloc(outlen);
    if (!out) return nullptr;
    size_t j = 0;
    for (size_t i = 0; i < len; i += 3) {
        uint32_t v = (uint32_t)data[i] << 16;
        if (i + 1 < len) v |= (uint32_t)data[i + 1] << 8;
        if (i + 2 < len) v |= (uint32_t)data[i + 2];
        out[j++] = B64T[(v >> 18) & 63];
        out[j++] = B64T[(v >> 12) & 63];
        out[j++] = (i + 1 < len) ? B64T[(v >> 6) & 63] : '=';
        out[j++] = (i + 2 < len) ? B64T[v & 63] : '=';
    }
    out[j] = '\0';
    return out;
}

void imgfree(void* p) {
    free(p);
}

uint64_t phash64(const uint8_t* rgba, int w, int h) {
    if (!rgba || w <= 0 || h <= 0) return 0;
    uint64_t result = 0xcbf29ce484222325ULL;
    long total = (long)w * h;
    long step = total > 64 ? total / 64 : 1;
    for (long i = 0; i < total; i += step) {
        uint8_t r = rgba[i * 4];
        uint8_t g = rgba[i * 4 + 1];
        uint8_t b = rgba[i * 4 + 2];
        result ^= (uint64_t)r + (uint64_t)g * 31 + (uint64_t)b * 17;
        result *= 0x100000001b3ULL;
    }
    if (result == 0) result = 1;
    return result;
}

int hammingmatch(uint64_t hash, const uint64_t* table, size_t n, int max_dist) {
    if (!table || n == 0) return 0;
    for (size_t i = 0; i < n; i++) {
        unsigned d = __builtin_popcountll(hash ^ table[i]);
        if ((int)d <= max_dist) return 1;
    }
    return 0;
}

#include <cctype>

static std::string lower_ascii(const char* s) {
    std::string out;
    if (!s) return out;
    while (*s) {
        unsigned char c = (unsigned char)*s++;
        out += (char)std::tolower(c);
    }
    return out;
}

int keywordscan(const char* text, const char* keywords) {
    if (!text || !keywords) return 0;
    std::string hay = lower_ascii(text);
    int count = 0;
    size_t i = 0;
    while (true) {
        size_t j = i;
        while (keywords[j] && keywords[j] != '\x1f') {
            j++;
        }
        if (j > i) {
            std::string kw(keywords + i, j - i);
            std::string lkw = lower_ascii(kw.c_str());
            if (!lkw.empty() && hay.find(lkw) != std::string::npos) {
                count++;
            }
        }
        if (!keywords[j]) break;
        i = j + 1;
    }
    return count;
}
