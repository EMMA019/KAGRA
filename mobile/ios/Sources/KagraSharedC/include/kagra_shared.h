#ifndef kagra_shared_h
#define kagra_shared_h

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct SharedSession SharedSession;

const char *kagra_shared_version(void);
const char *kagra_shared_last_error(void);
SharedSession *kagra_shared_create(void);
void kagra_shared_destroy(SharedSession *ptr);
int kagra_shared_create_surface(SharedSession *ptr, unsigned width, unsigned height);
int kagra_shared_set_asset_root(SharedSession *ptr, const char *root);
int kagra_shared_pause(SharedSession *ptr);
int kagra_shared_resume(SharedSession *ptr);
int kagra_shared_push_pointer(SharedSession *ptr, unsigned id, float x, float y, unsigned phase, float pressure);
int kagra_shared_set_pad(SharedSession *ptr, float x, float y);
/* 連続値のドライバ入力。steer は -1..1、throttle と brake は 0..1。 */
int kagra_shared_set_drive(SharedSession *ptr, float steer, float throttle, float brake);
/* 0 = 運転（3D）、1 = タッチデモ（2D）、2 = Crest Isle（歩き）。 */
int kagra_shared_set_scene(SharedSession *ptr, unsigned kind);
int kagra_shared_set_walk(SharedSession *ptr, float lx, float lz, int jump);
int kagra_shared_set_jump(SharedSession *ptr, int held);
int kagra_shared_save_json(SharedSession *ptr, char *buf, unsigned buflen);
int kagra_shared_load_json(SharedSession *ptr, const char *json);
int kagra_shared_set_settings(
    SharedSession *ptr,
    float master_volume,
    float steer_sensitivity,
    int muted
);
int kagra_shared_audio_json(SharedSession *ptr, char *buf, unsigned buflen);
int64_t kagra_shared_request_frame(SharedSession *ptr);
int kagra_shared_stats_json(SharedSession *ptr, char *buf, unsigned buflen);
int kagra_shared_resolve_alias(unsigned kind, const char *name, char *buf, unsigned buflen);

/* 描画。stub.c 実装では常に失敗し、本物の libkagra_shared.a をリンクすると
 * CAMetalLayer 経由で wgpu が描く。 */
int kagra_shared_attach_android_surface(SharedSession *ptr, void *a_native_window, unsigned width, unsigned height);
int kagra_shared_attach_ios_view(SharedSession *ptr, void *ui_view, unsigned width, unsigned height);
int kagra_shared_attach_offscreen(SharedSession *ptr, unsigned width, unsigned height);
int kagra_shared_detach_surface(SharedSession *ptr);
int kagra_shared_has_renderer(SharedSession *ptr);
int kagra_shared_render(SharedSession *ptr);

#ifdef __cplusplus
}
#endif

#endif
