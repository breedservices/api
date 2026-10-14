#include "bimg.h"

#include <cstdlib>
#include <cstring>
#include <mutex>
#include <string>

#include <leptonica/allheaders.h>
#include <tesseract/baseapi.h>

static std::string tessdata_dir(const char* hint) {
    if (hint && hint[0]) return hint;
    if (const char* e = getenv("BAPI_TESSDATA")) return e;
    return "/usr/share/tesseract-ocr/5/tessdata";
}

imgbuf ocrtext(const uint8_t* img, size_t len, const char* tessdata_path) {
    imgbuf nil{nullptr, 0};
    if (!img || !len) return nil;
    Pix* pix = pixReadMem(img, len);
    if (!pix) return nil;
    if (pixGetWidth(pix) < 64 || pixGetHeight(pix) < 64) {
        Pix* scaled = pixScale(pix, 2.0, 2.0);
        pixDestroy(&pix);
        pix = scaled;
        if (!pix) return nil;
    }
    Pix* bordered = pixAddBorder(pix, 10, 10);
    pixDestroy(&pix);
    pix = bordered ? bordered : pix;
    tesseract::TessBaseAPI api;
    std::string dir = tessdata_dir(tessdata_path);
    if (api.Init(dir.c_str(), "eng", tesseract::OEM_LSTM_ONLY)) {
        pixDestroy(&pix);
        return nil;
    }
    api.SetPageSegMode(tesseract::PSM_SPARSE_TEXT);
    api.SetVariable("user_defined_dpi", "70");
    api.SetImage(pix);
    char* raw = api.GetUTF8Text();
    imgbuf out{nullptr, 0};
    if (raw) {
        size_t n = strlen(raw);
        out.data = (uint8_t*)malloc(n + 1);
        if (out.data) {
            memcpy(out.data, raw, n + 1);
            out.len = n;
        }
        delete[] raw;
    }
    pixDestroy(&pix);
    return out;
}

void buffree(imgbuf b) { free(b.data); }
