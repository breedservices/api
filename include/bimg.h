#pragma once
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct {
    uint8_t* data;
    size_t len;
} imgbuf;

void buffree(imgbuf b);

uint32_t domcolor(const uint8_t* rgba, int w, int h);
void grayinplace(uint8_t* rgba, int w, int h);
void boxblurinplace(uint8_t* rgba, int w, int h);
void gradavatar(uint8_t* dst, const uint8_t* avatar, int w, int h,
                           uint8_t br, uint8_t bg, uint8_t bb,
                           int transparency, int flip);
uint64_t phash64(const uint8_t* rgba, int w, int h);
int hammingmatch(uint64_t hash, const uint64_t* table, size_t n, int max_dist);
int keywordscan(const char* text, const char* keywords);
char* b64encode(const uint8_t* data, size_t len);
void imgfree(void* p);

imgbuf ocrtext(const uint8_t* img, size_t len, const char* tessdata_path);
imgbuf staticcaption(const uint8_t* img, size_t len, const char* text, const char* font_path);
imgbuf vthumbnail(const uint8_t* img, size_t len, int width);
imgbuf vid2gif(const uint8_t* data, size_t len);


void c_paste_rgba(uint8_t* dst, int dw, int dh, const uint8_t* src, int sw, int sh, int x, int y);
void c_fill_rgba(uint8_t* p, int w, int h, int x, int y, int rw, int rh,
               uint8_t r, uint8_t g, uint8_t b, uint8_t a);
void c_rounded_rgba(uint8_t* p, int w, int h, int x, int y, int rw, int rh, int rad,
                  uint8_t r, uint8_t g, uint8_t b, uint8_t a);
void c_circle_copy(uint8_t* dst, const uint8_t* src, int size);

int selfcheck(void);

#ifdef __cplusplus
}
#endif
