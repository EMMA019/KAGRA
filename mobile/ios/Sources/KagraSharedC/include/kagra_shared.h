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
int64_t kagra_shared_request_frame(SharedSession *ptr);
int kagra_shared_stats_json(SharedSession *ptr, char *buf, unsigned buflen);
int kagra_shared_resolve_alias(unsigned kind, const char *name, char *buf, unsigned buflen);

#ifdef __cplusplus
}
#endif

#endif
