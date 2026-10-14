#include "bimg.h"

#include <cassert>
#include <cstdio>
#include <cstring>
#include <vector>

int selfcheck(void) {
    std::vector<uint8_t> red(16 * 16 * 4);
    for (size_t i = 0; i < 16 * 16; i++) {
        red[i * 4] = 200;
        red[i * 4 + 1] = 10;
        red[i * 4 + 2] = 10;
        red[i * 4 + 3] = 255;
    }
    uint32_t dc = domcolor(red.data(), 16, 16);
    assert(((dc >> 16) & 0xFF) > 100 && (dc & 0xFF) < 100);

    uint8_t px[4] = {255, 0, 0, 255};
    grayinplace(px, 1, 1);
    assert(px[0] == 76 || px[0] == 77);
    assert(px[3] == 255);

    std::vector<uint8_t> uni(8 * 8 * 4, 128);
    boxblurinplace(uni.data(), 8, 8);
    for (auto v : uni) assert(v == 128);

    uint64_t h1 = phash64(red.data(), 16, 16);
    assert(h1 == phash64(red.data(), 16, 16) && h1 != 0);

    uint64_t table[2] = {h1, 0x1234};
    assert(hammingmatch(h1, table, 2, 12) == 1);
    assert(hammingmatch(h1 ^ 0xFFFFFFFFFFFFFFFFULL, table, 2, 12) == 0);
    assert(hammingmatch(h1, nullptr, 0, 12) == 0);

    assert(keywordscan("Claim your REWARD, send usdt now", "claim your reward\x1Fusdt\x1Fpromo code") == 2);
    assert(keywordscan("hello world", "claim your reward\x1Fusdt") == 0);
    assert(keywordscan(nullptr, "x") == 0);

    std::vector<uint8_t> av(4 * 2 * 4, 100), dst(4 * 2 * 4);
    for (size_t i = 0; i < 8; i++) av[i * 4 + 3] = 255;
    gradavatar(dst.data(), av.data(), 4, 2, 0, 0, 0, 255, 0);
    assert(dst[0] < 20);
    assert(dst[(3 * 4)] > 60);

    const uint8_t man[] = {'M', 'a', 'n'};
    char* b64 = b64encode(man, 3);
    assert(strcmp(b64, "TWFu") == 0);
    imgfree(b64);

    assert(domcolor(nullptr, 4, 4) == 0x2B2D31);
    assert(phash64(nullptr, 4, 4) == 0);
    return 0;
}

#ifndef BAPI_IMG_LIB_ONLY
int main() {
    int rc = selfcheck();
    printf("bimg selfcheck: %s\n", rc == 0 ? "OK" : "FAIL");
    return rc;
}
#endif
