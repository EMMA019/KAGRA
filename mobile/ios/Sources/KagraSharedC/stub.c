/* Stub implementations when libkagra_shared.a is not linked yet.
 * Replace by linking the real static library from cargo-lipo / xcframework.
 */
#include "kagra_shared.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

static int64_t g_frame = 0;
static unsigned g_w = 390, g_h = 844;
static int g_paused = 0;

static const char *g_err = "";

const char *kagra_shared_version(void) { return "0.1.0-stub"; }
const char *kagra_shared_last_error(void) { return g_err; }

SharedSession *kagra_shared_create(void) {
    return (SharedSession *)(uintptr_t)1;
}
void kagra_shared_destroy(SharedSession *ptr) { (void)ptr; }

int kagra_shared_create_surface(SharedSession *ptr, unsigned width, unsigned height) {
    (void)ptr; g_w = width; g_h = height; return 0;
}
int kagra_shared_set_asset_root(SharedSession *ptr, const char *root) {
    (void)ptr; (void)root; return 0;
}
int kagra_shared_pause(SharedSession *ptr) { (void)ptr; g_paused = 1; return 0; }
int kagra_shared_resume(SharedSession *ptr) { (void)ptr; g_paused = 0; return 0; }
int kagra_shared_push_pointer(SharedSession *ptr, unsigned id, float x, float y, unsigned phase, float pressure) {
    (void)ptr; (void)id; (void)x; (void)y; (void)phase; (void)pressure; return 0;
}
int kagra_shared_set_pad(SharedSession *ptr, float x, float y) {
    (void)ptr; (void)x; (void)y; return 0;
}
int64_t kagra_shared_request_frame(SharedSession *ptr) {
    (void)ptr;
    if (!g_paused) g_frame++;
    return g_frame;
}
int kagra_shared_stats_json(SharedSession *ptr, char *buf, unsigned buflen) {
    (void)ptr;
    char tmp[128];
    snprintf(tmp, sizeof(tmp),
             "{\"frame\":%lld,\"width\":%u,\"height\":%u,\"paused\":%s,\"pointer_count\":0}",
             (long long)g_frame, g_w, g_h, g_paused ? "true" : "false");
    size_t n = strlen(tmp) + 1;
    if (!buf || buflen < n) return (int)n;
    memcpy(buf, tmp, n);
    return (int)n;
}
/* 描画はスタブでは行えない。UI 側はこの失敗を見てプレースホルダを出す。 */
static int no_renderer(void) {
    g_err = "stub build: link libkagra_shared.a for rendering";
    return -1;
}
int kagra_shared_attach_android_surface(SharedSession *ptr, void *w, unsigned width, unsigned height) {
    (void)ptr; (void)w; (void)width; (void)height; return no_renderer();
}
int kagra_shared_attach_ios_view(SharedSession *ptr, void *view, unsigned width, unsigned height) {
    (void)ptr; (void)view; g_w = width; g_h = height; return no_renderer();
}
int kagra_shared_attach_offscreen(SharedSession *ptr, unsigned width, unsigned height) {
    (void)ptr; g_w = width; g_h = height; return no_renderer();
}
int kagra_shared_detach_surface(SharedSession *ptr) { (void)ptr; return 0; }
int kagra_shared_has_renderer(SharedSession *ptr) { (void)ptr; return 0; }
int kagra_shared_render(SharedSession *ptr) { (void)ptr; return no_renderer(); }

int kagra_shared_resolve_alias(unsigned kind, const char *name, char *buf, unsigned buflen) {
    (void)kind; (void)name;
    const char *s = "assets/Emma.vrm";
    size_t n = strlen(s) + 1;
    if (!buf || buflen < n) return (int)n;
    memcpy(buf, s, n);
    return (int)n;
}
