#pragma once

#ifdef __cplusplus
extern "C" {
#endif

char* apicall(const char* route, const char* params_json);
void apifree(char* s);

#ifdef __cplusplus
}
#endif
