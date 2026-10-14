#include "bimg.h"

#include <algorithm>
#include <cstdlib>
#include <cstring>
#include <mutex>
#include <string>

#include <vips/vips8>

static void ensure_init() {
    static std::once_flag f;
    std::call_once(f, [] {
        if (vips_init("bimg")) abort();
    });
}

static imgbuf to_png_buf(vips::VImage img) {
    imgbuf nil{nullptr, 0};
    void* ptr = nullptr;
    size_t n = 0;
    try {
        img.write_to_buffer(".png", &ptr, &n);
    } catch (...) {
        return nil;
    }
    if (!ptr) return nil;
    imgbuf out{(uint8_t*)malloc(n), n};
    if (!out.data) {
        g_free(ptr);
        return nil;
    }
    memcpy(out.data, ptr, n);
    g_free(ptr);
    return out;
}

imgbuf staticcaption(const uint8_t* img, size_t len, const char* text, const char* font_path) {
    imgbuf nil{nullptr, 0};
    if (!img || !len || !text) return nil;
    ensure_init();
    try {
        std::string owned((const char*)img, len);
        vips::VImage v = vips::VImage::new_from_buffer(owned, "");
        v = v.colourspace(VIPS_INTERPRETATION_sRGB);
        if (!v.has_alpha()) v = v.bandjoin(255);
        if (v.width() > 480) v = v.thumbnail_image(480);
        int W = v.width();
        int fs = std::max(20, std::min(80, W / 12));
        std::string fontdesc = std::string("Sans Bold ") + std::to_string(fs);
        vips::VImage cap = vips::VImage::text(
            text,
            vips::VImage::option()
                ->set("fontfile", font_path && font_path[0] ? font_path : "")
                ->set("font", fontdesc.c_str())
                ->set("width", W - 20)
                ->set("align", VIPS_ALIGN_CENTRE)
                ->set("rgba", true));
        int bar = cap.height() + 20;
        vips::VImage bg = (vips::VImage::black(
                                   W, bar + v.height(),
                                   vips::VImage::option()->set("bands", 4))
                               + std::vector<double>{255.0, 255.0, 255.0, 255.0})
                                  .cast(VIPS_FORMAT_UCHAR)
                                  .copy(vips::VImage::option()->set("interpretation", VIPS_INTERPRETATION_sRGB));
        vips::VImage res = bg.composite2(cap, VIPS_BLEND_MODE_OVER,
                                         vips::VImage::option()->set("x", (W - cap.width()) / 2)->set("y", 10));
        res = res.composite2(v, VIPS_BLEND_MODE_OVER,
                             vips::VImage::option()->set("x", 0)->set("y", bar));
        return to_png_buf(res);
    } catch (...) {
        return nil;
    }
}

imgbuf vthumbnail(const uint8_t* img, size_t len, int width) {
    imgbuf nil{nullptr, 0};
    if (!img || !len || width <= 0) return nil;
    ensure_init();
    try {
        std::string owned((const char*)img, len);
        vips::VImage v = vips::VImage::new_from_buffer(owned, "");
        v = v.thumbnail_image(width);
        return to_png_buf(v);
    } catch (...) {
        return nil;
    }
}
