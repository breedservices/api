#include "bimg.h"

#include <cctype>
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
            int t = transparency - (transparency * x) / (w > 1 ? w - 1 : 1);
            if (t < 0) t = 0;
            if (t > 255) t = 255;
            if (flip) t = 255 - t;
            const uint8_t* s = avatar + ((size_t)y * w + x) * 4;
            uint8_t* d = dst + ((size_t)y * w + x) * 4;
            d[0] = (uint8_t)((s[0] * (255 - t) + br * t) / 255);
            d[1] = (uint8_t)((s[1] * (255 - t) + bg * t) / 255);
            d[2] = (uint8_t)((s[2] * (255 - t) + bb * t) / 255);
            d[3] = s[3];
        }
}

static void gray32(const uint8_t* rgba, int w, int h, double out[32][32]) {
    for (int y = 0; y < 32; y++)
        for (int x = 0; x < 32; x++) {
            int x0 = x * w / 32, x1 = (x + 1) * w / 32;
            int y0 = y * h / 32, y1 = (y + 1) * h / 32;
            if (x1 <= x0) x1 = x0 + 1;
            if (y1 <= y0) y1 = y0 + 1;
            double sum = 0;
            long n = 0;
            for (int yy = y0; yy < y1 && yy < h; yy++)
                for (int xx = x0; xx < x1 && xx < w; xx++) {
                    const uint8_t* p = rgba + ((size_t)yy * w + xx) * 4;
                    sum += p[0] * 0.299 + p[1] * 0.587 + p[2] * 0.114;
                    n++;
                }
            out[y][x] = n ? sum / n : 0;
        }
}

uint64_t phash64(const uint8_t* rgba, int w, int h) {
    if (!rgba || w <= 0 || h <= 0) return 0;
    double px[32][32];
    gray32(rgba, w, h, px);
    double dct[8][8];
    for (int v = 0; v < 8; v++)
        for (int u = 0; u < 8; u++) {
            double s = 0;
            for (int y = 0; y < 32; y++)
                for (int x = 0; x < 32; x++)
                    s += px[y][x] * cos((2 * x + 1) * u * 3.14159265358979 / 64.0) *
                         cos((2 * y + 1) * v * 3.14159265358979 / 64.0);
            dct[v][u] = s;
        }
    double vals[63];
    int k = 0;
    for (int v = 0; v < 8; v++)
        for (int u = 0; u < 8; u++) {
            if (u == 0 && v == 0) continue;
            vals[k++] = dct[v][u];
        }
    for (int i = 0; i < 63; i++)
        for (int j = i + 1; j < 63; j++)
            if (vals[j] < vals[i]) {
                double t = vals[i];
                vals[i] = vals[j];
                vals[j] = t;
            }
    double med = vals[31];
    uint64_t hash = 0;
    k = 0;
    for (int v = 0; v < 8; v++)
        for (int u = 0; u < 8; u++) {
            if (u == 0 && v == 0) continue;
            if (dct[v][u] > med) hash |= (1ULL << k);
            k++;
        }
    return hash;
}

int hammingmatch(uint64_t hash, const uint64_t* table, size_t n, int max_dist) {
    if (!table) return 0;
    for (size_t i = 0; i < n; i++)
        if (__builtin_popcountll(hash ^ table[i]) <= (unsigned)max_dist) return 1;
    return 0;
}

static int ci_contains(const char* hay, const char* needle) {
    size_t nl = strlen(needle);
    if (!nl) return 0;
    for (const char* p = hay; *p; p++) {
        size_t i = 0;
        while (i < nl && p[i] && tolower((unsigned char)p[i]) == tolower((unsigned char)needle[i])) i++;
        if (i == nl) return 1;
    }
    return 0;
}

int keywordscan(const char* text, const char* keywords) {
    if (!text || !keywords) return 0;
    int hits = 0;
    const char* p = keywords;
    char kw[128];
    while (*p) {
        size_t i = 0;
        while (*p && *p != '\x1F' && i + 1 < sizeof(kw)) kw[i++] = *p++;
        kw[i] = 0;
        if (*p == '\x1F') p++;
        if (i && ci_contains(text, kw)) hits++;
    }
    return hits;
}

char* b64encode(const uint8_t* data, size_t len) {
    static const char* B = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    size_t out = ((len + 2) / 3) * 4;
    char* s = (char*)malloc(out + 1);
    if (!s) return nullptr;
    size_t o = 0;
    for (size_t i = 0; i < len; i += 3) {
        uint32_t n = (uint32_t)data[i] << 16;
        if (i + 1 < len) n |= (uint32_t)data[i + 1] << 8;
        if (i + 2 < len) n |= data[i + 2];
        s[o++] = B[(n >> 18) & 63];
        s[o++] = B[(n >> 12) & 63];
        s[o++] = (i + 1 < len) ? B[(n >> 6) & 63] : '=';
        s[o++] = (i + 2 < len) ? B[n & 63] : '=';
    }
    s[o] = 0;
    return s;
}

void imgfree(void* p) { free(p); }
