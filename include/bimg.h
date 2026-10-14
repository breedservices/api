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

int selfcheck(void);

#ifdef __cplusplus
}
#endif
