#include <stdarg.h>
#include <stdint.h>
#include <time.h>

extern "C" int64_t sysprof_clock(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (int64_t)ts.tv_sec * 1000000000LL + ts.tv_nsec;
}

extern "C" void sysprof_collector_mark_vprintf(int64_t, int64_t, const char*, const char*, const char*, va_list) {
}

extern "C" unsigned int sysprof_collector_request_counters(unsigned int n) {
    return n;
}

extern "C" void sysprof_collector_define_counters(const void*, unsigned int) {
}

extern "C" void sysprof_collector_set_counters(const unsigned int*, const void*, unsigned int) {
}
