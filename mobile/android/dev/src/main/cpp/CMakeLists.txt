cmake_minimum_required(VERSION 3.22.1)
project(kagra_jni)

# Same JNI as the game shell (package dev.kagra.shell) so libkagra_jni matches
# KagraNative. applicationId is separate (dev.kagra.devshell).
set(JNI_SRC ${CMAKE_SOURCE_DIR}/../../../../app/src/main/cpp/jni_bridge.cpp)
add_library(kagra_jni SHARED ${JNI_SRC})

# 16 KB ELF alignment, same flags as the game shell. NDK r28+ does this by
# default; the flags also cover an older NDK so libkagra_jni.so LOAD p_align
# stays >= 16384. libkagra_shared.so is aligned by scripts/build_android_native.sh.
target_link_options(kagra_jni PRIVATE
  "-Wl,-z,max-page-size=16384"
  "-Wl,-z,common-page-size=16384")

find_library(log-lib log)

set(SHARED_LIB_DIR ${CMAKE_SOURCE_DIR}/../../../../app/src/main/jniLibs/${ANDROID_ABI})
set(SHARED_LIB ${SHARED_LIB_DIR}/libkagra_shared.so)

if(EXISTS ${SHARED_LIB})
  # Do not target_link_libraries() the imported .so. NDK CMake passes
  # IMPORTED_LOCATION to lld, which writes that absolute path into DT_NEEDED.
  add_library(kagra_shared SHARED IMPORTED)
  set_target_properties(kagra_shared PROPERTIES
    IMPORTED_LOCATION "${SHARED_LIB}"
    IMPORTED_NO_SONAME FALSE
    IMPORTED_SONAME "libkagra_shared.so")
  get_filename_component(KAGRA_REPO "${CMAKE_SOURCE_DIR}/../../../../../.." ABSOLUTE)
  set(KAGRA_INCLUDE "${KAGRA_REPO}/kagra-shared/include")
  if(NOT EXISTS "${KAGRA_INCLUDE}/kagra_shared.h")
    message(FATAL_ERROR "kagra_shared.h not found at ${KAGRA_INCLUDE}")
  endif()
  target_include_directories(kagra_jni PRIVATE "${KAGRA_INCLUDE}")
  target_compile_definitions(kagra_jni PRIVATE KAGRA_HAS_SHARED=1)
  target_link_libraries(kagra_jni
    "-Wl,-L${SHARED_LIB_DIR}"
    "-Wl,-l:libkagra_shared.so"
    ${log-lib}
    android)
else()
  message(WARNING "libkagra_shared.so not found for ${ANDROID_ABI}; building JNI stub only")
  target_compile_definitions(kagra_jni PRIVATE KAGRA_HAS_SHARED=0)
  target_link_libraries(kagra_jni ${log-lib} android)
endif()
