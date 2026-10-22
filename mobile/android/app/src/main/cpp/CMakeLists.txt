cmake_minimum_required(VERSION 3.22.1)
project(kagra_jni)

add_library(kagra_jni SHARED jni_bridge.cpp)

find_library(log-lib log)

# 事前ビルド済み libkagra_shared.so を jniLibs/<ABI>/ に置く想定。
# 無い場合でもリンクエラーを避け、スタブで起動できるようにする。
set(SHARED_LIB_DIR ${CMAKE_SOURCE_DIR}/../jniLibs/${ANDROID_ABI})
set(SHARED_LIB ${SHARED_LIB_DIR}/libkagra_shared.so)

if(EXISTS ${SHARED_LIB})
  add_library(kagra_shared SHARED IMPORTED)
  set_target_properties(kagra_shared PROPERTIES IMPORTED_LOCATION ${SHARED_LIB})
  # cpp → main → src → app → android → mobile → repo root
  get_filename_component(KAGRA_REPO "${CMAKE_SOURCE_DIR}/../../../../../.." ABSOLUTE)
  set(KAGRA_INCLUDE "${KAGRA_REPO}/kagra-shared/include")
  if(NOT EXISTS "${KAGRA_INCLUDE}/kagra_shared.h")
    message(FATAL_ERROR "kagra_shared.h not found at ${KAGRA_INCLUDE}")
  endif()
  target_include_directories(kagra_jni PRIVATE "${KAGRA_INCLUDE}")
  target_compile_definitions(kagra_jni PRIVATE KAGRA_HAS_SHARED=1)
  target_link_libraries(kagra_jni kagra_shared ${log-lib} android)
else()
  message(WARNING "libkagra_shared.so not found for ${ANDROID_ABI}; building JNI stub only")
  target_compile_definitions(kagra_jni PRIVATE KAGRA_HAS_SHARED=0)
  target_link_libraries(kagra_jni ${log-lib} android)
endif()
