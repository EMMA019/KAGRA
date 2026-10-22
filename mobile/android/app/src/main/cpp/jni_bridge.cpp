#include <jni.h>
#include <string>
#include <vector>
#include <android/log.h>
#include <android/native_window.h>
#include <android/native_window_jni.h>

#if KAGRA_HAS_SHARED
#include "kagra_shared.h"
static SharedSession *g_session = nullptr;
#endif

// Surface から取り出した ANativeWindow。wgpu のサーフェスより長生きさせる。
static ANativeWindow *g_window = nullptr;

static void release_window() {
  if (g_window) {
    ANativeWindow_release(g_window);
    g_window = nullptr;
  }
}

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
    kagra_shared_detach_surface(g_session);
    kagra_shared_destroy(g_session);
    g_session = nullptr;
  }
#endif
  release_window();
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

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_setDrive(
    JNIEnv *, jobject, jfloat steer, jfloat throttle, jfloat brake) {
#if KAGRA_HAS_SHARED
  return g_session && kagra_shared_set_drive(g_session, steer, throttle, brake) == 0;
#else
  (void)steer; (void)throttle; (void)brake;
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_setScene(JNIEnv *, jobject, jint kind) {
#if KAGRA_HAS_SHARED
  return g_session && kagra_shared_set_scene(g_session, (unsigned)kind) == 0;
#else
  (void)kind;
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_setWalk(
    JNIEnv *, jobject, jfloat lx, jfloat lz, jboolean jump) {
#if KAGRA_HAS_SHARED
  return g_session && kagra_shared_set_walk(g_session, lx, lz, jump ? 1 : 0) == 0;
#else
  (void)lx; (void)lz; (void)jump;
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_setPlayInput(
    JNIEnv *, jobject, jfloat lx, jfloat lz, jboolean jump, jboolean attack, jboolean dodge) {
#if KAGRA_HAS_SHARED
  return g_session && kagra_shared_set_play_input(
      g_session, lx, lz, jump ? 1 : 0, attack ? 1 : 0, dodge ? 1 : 0) == 0;
#else
  (void)lx; (void)lz; (void)jump; (void)attack; (void)dodge;
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_loadGameFile(JNIEnv *env, jobject, jstring path) {
#if KAGRA_HAS_SHARED
  if (!g_session || !path) return JNI_FALSE;
  const char *c = env->GetStringUTFChars(path, nullptr);
  if (!c) return JNI_FALSE;
  int rc = kagra_shared_load_game_file(g_session, c);
  env->ReleaseStringUTFChars(path, c);
  return rc == 0 ? JNI_TRUE : JNI_FALSE;
#else
  (void)env; (void)path;
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_setJump(JNIEnv *, jobject, jboolean held) {
#if KAGRA_HAS_SHARED
  return g_session && kagra_shared_set_jump(g_session, held ? 1 : 0) == 0;
#else
  (void)held;
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

extern "C" JNIEXPORT jstring JNICALL
Java_dev_kagra_shell_KagraNative_saveJson(JNIEnv *env, jobject) {
#if KAGRA_HAS_SHARED
  if (!g_session) return env->NewStringUTF("{}");
  int need = kagra_shared_save_json(g_session, nullptr, 0);
  if (need <= 0) return env->NewStringUTF("{}");
  std::vector<char> buf((size_t)need);
  kagra_shared_save_json(g_session, buf.data(), (unsigned)buf.size());
  return env->NewStringUTF(buf.data());
#else
  return env->NewStringUTF("{}");
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_loadJson(JNIEnv *env, jobject, jstring json) {
#if KAGRA_HAS_SHARED
  if (!g_session || !json) return JNI_FALSE;
  const char *c = env->GetStringUTFChars(json, nullptr);
  if (!c) return JNI_FALSE;
  int rc = kagra_shared_load_json(g_session, c);
  env->ReleaseStringUTFChars(json, c);
  return rc == 0 ? JNI_TRUE : JNI_FALSE;
#else
  (void)env; (void)json;
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_setSettings(
    JNIEnv *, jobject, jfloat master, jfloat steer, jboolean muted) {
#if KAGRA_HAS_SHARED
  return g_session &&
         kagra_shared_set_settings(g_session, master, steer, muted ? 1 : 0) == 0;
#else
  (void)master; (void)steer; (void)muted;
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_setSaveRoot(JNIEnv *env, jobject, jstring path) {
#if KAGRA_HAS_SHARED
  if (!path) return JNI_FALSE;
  const char *c = env->GetStringUTFChars(path, nullptr);
  if (!c) return JNI_FALSE;
  int rc = kagra_shared_set_save_root(c);
  env->ReleaseStringUTFChars(path, c);
  return rc == 0 ? JNI_TRUE : JNI_FALSE;
#else
  (void)env;
  (void)path;
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_dev_kagra_shell_KagraNative_takeAudioCues(JNIEnv *env, jobject) {
#if KAGRA_HAS_SHARED
  int need = kagra_shared_take_audio_cues(nullptr, 0);
  if (need <= 1) return env->NewStringUTF("{}");
  std::vector<char> buf((size_t)need);
  kagra_shared_take_audio_cues(buf.data(), (unsigned)buf.size());
  return env->NewStringUTF(buf.data());
#else
  return env->NewStringUTF("{}");
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_dev_kagra_shell_KagraNative_audioJson(JNIEnv *env, jobject) {
#if KAGRA_HAS_SHARED
  char buf[256];
  if (!g_session) return env->NewStringUTF("{}");
  kagra_shared_audio_json(g_session, buf, sizeof(buf));
  return env->NewStringUTF(buf);
#else
  return env->NewStringUTF("{\"engine\":0,\"wind\":0,\"brake\":0}");
#endif
}

// ── 描画 ────────────────────────────────────────────────────

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_attachSurface(
    JNIEnv *env, jobject, jobject surface, jint w, jint h) {
#if KAGRA_HAS_SHARED
  if (!g_session || !surface) return JNI_FALSE;
  release_window();
  g_window = ANativeWindow_fromSurface(env, surface);
  if (!g_window) {
    LOGI("ANativeWindow_fromSurface returned null");
    return JNI_FALSE;
  }
  int rc = kagra_shared_attach_android_surface(
      g_session, (void *)g_window, (unsigned)w, (unsigned)h);
  if (rc != 0) {
    LOGI("attach failed: %s", kagra_shared_last_error());
    release_window();
    return JNI_FALSE;
  }
  return JNI_TRUE;
#else
  (void)env; (void)surface; (void)w; (void)h;
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT void JNICALL
Java_dev_kagra_shell_KagraNative_detachSurface(JNIEnv *, jobject) {
#if KAGRA_HAS_SHARED
  if (g_session) kagra_shared_detach_surface(g_session);
#endif
  // サーフェスを捨ててから ANativeWindow を解放する順序を守る。
  release_window();
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_hasRenderer(JNIEnv *, jobject) {
#if KAGRA_HAS_SHARED
  return g_session && kagra_shared_has_renderer(g_session) == 1;
#else
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_render(JNIEnv *, jobject) {
#if KAGRA_HAS_SHARED
  return g_session && kagra_shared_render(g_session) == 0;
#else
  return JNI_FALSE;
#endif
}

// ── 「直してここ」 ──────────────────────────────────────────

#if KAGRA_HAS_SHARED
static jstring playtest_json(JNIEnv *env, int (*fn)(SharedSession *, char *, unsigned)) {
  if (!g_session) return env->NewStringUTF("{}");
  int need = fn(g_session, nullptr, 0);
  if (need <= 1) return env->NewStringUTF("{}");
  std::vector<char> buf((size_t)need);
  fn(g_session, buf.data(), (unsigned)buf.size());
  return env->NewStringUTF(buf.data());
}
#endif

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_setPlaytestEnabled(JNIEnv *, jobject, jboolean on) {
#if KAGRA_HAS_SHARED
  return g_session && kagra_shared_set_playtest_enabled(g_session, on ? 1 : 0) == 0;
#else
  (void)on;
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_setPlaytestExternalChrome(JNIEnv *, jobject, jboolean on) {
#if KAGRA_HAS_SHARED
  return g_session && kagra_shared_set_playtest_external_chrome(g_session, on ? 1 : 0) == 0;
#else
  (void)on;
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_setPlaytestDevice(
    JNIEnv *env, jobject, jstring label, jstring lang) {
#if KAGRA_HAS_SHARED
  if (!g_session) return JNI_FALSE;
  const char *c_label = label ? env->GetStringUTFChars(label, nullptr) : nullptr;
  const char *c_lang = lang ? env->GetStringUTFChars(lang, nullptr) : nullptr;
  int rc = kagra_shared_set_playtest_device(g_session, c_label, c_lang);
  if (c_label) env->ReleaseStringUTFChars(label, c_label);
  if (c_lang) env->ReleaseStringUTFChars(lang, c_lang);
  return rc == 0 ? JNI_TRUE : JNI_FALSE;
#else
  (void)env;
  (void)label;
  (void)lang;
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_playtestPointer(
    JNIEnv *, jobject, jint id, jfloat x, jfloat y, jint phase, jfloat pressure) {
#if KAGRA_HAS_SHARED
  if (!g_session) return JNI_FALSE;
  return kagra_shared_playtest_pointer(
             g_session, (unsigned)id, x, y, (unsigned)phase, pressure) == 1;
#else
  (void)id;
  (void)x;
  (void)y;
  (void)phase;
  (void)pressure;
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_pushKey(JNIEnv *env, jobject, jstring name, jboolean down) {
#if KAGRA_HAS_SHARED
  if (!g_session || !name) return JNI_FALSE;
  const char *c = env->GetStringUTFChars(name, nullptr);
  if (!c) return JNI_FALSE;
  int rc = kagra_shared_push_key(g_session, c, down ? 1 : 0);
  env->ReleaseStringUTFChars(name, c);
  return rc == 1 ? JNI_TRUE : JNI_FALSE;
#else
  (void)env;
  (void)name;
  (void)down;
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_playtestSetNote(JNIEnv *env, jobject, jstring note) {
#if KAGRA_HAS_SHARED
  if (!g_session) return JNI_FALSE;
  const char *c = note ? env->GetStringUTFChars(note, nullptr) : nullptr;
  int rc = kagra_shared_playtest_set_note(g_session, c);
  if (c) env->ReleaseStringUTFChars(note, c);
  return rc == 0 ? JNI_TRUE : JNI_FALSE;
#else
  (void)env;
  (void)note;
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_playtestSetShotPath(JNIEnv *env, jobject, jstring path) {
#if KAGRA_HAS_SHARED
  if (!g_session) return JNI_FALSE;
  const char *c = path ? env->GetStringUTFChars(path, nullptr) : nullptr;
  int rc = kagra_shared_playtest_set_shot_path(g_session, c);
  if (c) env->ReleaseStringUTFChars(path, c);
  return rc == 0 ? JNI_TRUE : JNI_FALSE;
#else
  (void)env;
  (void)path;
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jboolean JNICALL
Java_dev_kagra_shell_KagraNative_playtestAction(JNIEnv *env, jobject, jstring action) {
#if KAGRA_HAS_SHARED
  if (!g_session || !action) return JNI_FALSE;
  const char *c = env->GetStringUTFChars(action, nullptr);
  if (!c) return JNI_FALSE;
  int rc = kagra_shared_playtest_action(g_session, c);
  env->ReleaseStringUTFChars(action, c);
  return rc == 0 ? JNI_TRUE : JNI_FALSE;
#else
  (void)env;
  (void)action;
  return JNI_FALSE;
#endif
}

extern "C" JNIEXPORT jint JNICALL
Java_dev_kagra_shell_KagraNative_playtestBlocksInput(JNIEnv *, jobject) {
#if KAGRA_HAS_SHARED
  if (!g_session) return 0;
  int rc = kagra_shared_playtest_blocks_input(g_session);
  return rc < 0 ? 0 : rc;
#else
  return 0;
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_dev_kagra_shell_KagraNative_playtestStateJson(JNIEnv *env, jobject) {
#if KAGRA_HAS_SHARED
  return playtest_json(env, kagra_shared_playtest_state_json);
#else
  return env->NewStringUTF("{}");
#endif
}

extern "C" JNIEXPORT jstring JNICALL
Java_dev_kagra_shell_KagraNative_playtestPacketJson(JNIEnv *env, jobject) {
#if KAGRA_HAS_SHARED
  return playtest_json(env, kagra_shared_playtest_packet_json);
#else
  return env->NewStringUTF("{}");
#endif
}
