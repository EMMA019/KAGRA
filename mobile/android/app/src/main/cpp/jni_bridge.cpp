#include <jni.h>
#include <string>
#include <android/log.h>

#if KAGRA_HAS_SHARED
#include "kagra_shared.h"
static SharedSession *g_session = nullptr;
#endif

#define LOGI(...) __android_log_print(ANDROID_LOG_INFO, "kagra", __VA_ARGS__)

extern "C" JNIEXPORT jstring JNICALL
Java_dev_kagra_shell_KagraNative_version(JNIEnv *env, jobject) {
#if KAGRA_HAS_SHARED
  return env->NewStringUTF(kagra_shared_version());
#else
  return env->NewStringUTF("stub-0.1.0");
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_dev_kagra_shell_KagraNative_lastError(JNIEnv *env, jobject) {
#if KAGRA_HAS_SHARED
  return env->NewStringUTF(kagra_shared_last_error());
#else
  return env->NewStringUTF("libkagra_shared.so missing — run scripts/build_android_native.sh");
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_create(JNIEnv *, jobject) {
#if KAGRA_HAS_SHARED
  if (g_session) kagra_shared_destroy(g_session);
  g_session = kagra_shared_create();
  return g_session != nullptr;
#else
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT void JNICALL
Java_dev_kagra_shell_KagraNative_destroy(JNIEnv *, jobject) {
#if KAGRA_HAS_SHARED
  if (g_session) {
    kagra_shared_destroy(g_session);
    g_session = nullptr;
  }
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_createSurface(JNIEnv *, jobject, jint w, jint h) {
#if KAGRA_HAS_SHARED
  return g_session && kagra_shared_create_surface(g_session, (unsigned)w, (unsigned)h) == 0;
#else
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_setAssetRoot(JNIEnv *env, jobject, jstring root) {
#if KAGRA_HAS_SHARED
  if (!g_session) return JNI_FALSE;
  const char *c = env->GetStringUTFChars(root, nullptr);
  int rc = kagra_shared_set_asset_root(g_session, c);
  env->ReleaseStringUTFChars(root, c);
  return rc == 0;
#else
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_pause(JNIEnv *, jobject) {
#if KAGRA_HAS_SHARED
  return g_session && kagra_shared_pause(g_session) == 0;
#else
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_resume(JNIEnv *, jobject) {
#if KAGRA_HAS_SHARED
  return g_session && kagra_shared_resume(g_session) == 0;
#else
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_pushPointer(
    JNIEnv *, jobject, jint id, jfloat x, jfloat y, jint phase, jfloat pressure) {
#if KAGRA_HAS_SHARED
  return g_session && kagra_shared_push_pointer(
      g_session, (unsigned)id, x, y, (unsigned)phase, pressure) == 0;
#else
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_setPad(JNIEnv *, jobject, jfloat x, jfloat y) {
#if KAGRA_HAS_SHARED
  return g_session && kagra_shared_set_pad(g_session, x, y) == 0;
#else
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jlong JNICALL
Java_dev_kagra_shell_KagraNative_requestFrame(JNIEnv *, jobject) {
#if KAGRA_HAS_SHARED
  if (!g_session) return -1;
  return (jlong)kagra_shared_request_frame(g_session);
#else
  return -1;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_dev_kagra_shell_KagraNative_statsJson(JNIEnv *env, jobject) {
#if KAGRA_HAS_SHARED
  char buf[512];
  if (!g_session) return env->NewStringUTF("{}");
  kagra_shared_stats_json(g_session, buf, sizeof(buf));
  return env->NewStringUTF(buf);
#else
  return env->NewStringUTF("{\"stub\":true}");
#endif
}
