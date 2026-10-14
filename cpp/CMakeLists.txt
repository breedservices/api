cmake_minimum_required(VERSION 3.16)
project(bimg CXX)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

set(TP "${CMAKE_CURRENT_SOURCE_DIR}/../thirdparty")

find_package(PkgConfig QUIET)
if(PkgConfig_FOUND)
  pkg_check_modules(VIPS_CPP QUIET vips-cpp)
  pkg_check_modules(TESS QUIET tesseract)
endif()

if(PkgConfig_FOUND)
  pkg_check_modules(FFMPEG QUIET libavformat libavcodec libavutil libswscale libswresample)
endif()

set(FFTP_INC "${TP}/usr/include/x86_64-linux-gnu")
set(FFTP_LIB "${TP}/usr/lib/x86_64-linux-gnu")
if(EXISTS "${FFTP_INC}/libavformat/avformat.h")
  set(FFMPEG_FOUND 1)
  set(FFMPEG_INCLUDE_DIRS "${FFTP_INC}")
  set(FFMPEG_LIBRARY_DIRS "${FFTP_LIB}")
  set(FFMPEG_LIBRARIES
      "${FFTP_LIB}/libavformat.so"
      "${FFTP_LIB}/libavcodec.so"
      "${FFTP_LIB}/libavutil.so"
      "${FFTP_LIB}/libswscale.so"
      "${FFTP_LIB}/libswresample.so"
  )
elseif(FFMPEG_FOUND)
  # use pkg-config locations as-is
endif()

if(VIPS_CPP_FOUND AND TESS_FOUND)
  include_directories(${CMAKE_CURRENT_SOURCE_DIR}/../include ${VIPS_CPP_INCLUDE_DIRS} ${TESS_INCLUDE_DIRS})
  link_directories(${VIPS_CPP_LIBRARY_DIRS} ${TESS_LIBRARY_DIRS})
  set(SYS_LIBS ${VIPS_CPP_LIBRARIES} ${TESS_LIBRARIES} leptonica glib-2.0 gobject-2.0)
else()
  set(TP_INC "${TP}/usr/include")
  set(TP_LIB "${TP}/usr/lib/x86_64-linux-gnu")
  include_directories(
    ${CMAKE_CURRENT_SOURCE_DIR}/../include
    ${TP_INC}
    ${TP_INC}/glib-2.0
    ${TP_INC}/x86_64-linux-gnu
    ${TP_LIB}/glib-2.0/include
  )
  link_directories(${TP_LIB})
  set(SYS_LIBS tesseract leptonica vips vips-cpp glib-2.0 gobject-2.0)
  set(TP_RPATH "${TP_LIB}")
endif()

if(FFMPEG_FOUND)
    include_directories(${FFMPEG_INCLUDE_DIRS})
    link_directories(${FFMPEG_LIBRARY_DIRS})
endif()
add_library(kernels STATIC kernels.cpp)
add_library(imgfull STATIC kernels.cpp ocr.cpp vips_ops.cpp)

add_library(bimg SHARED kernels.cpp ocr.cpp vips_ops.cpp shim.cpp)
if(FFMPEG_FOUND)
    target_sources(bimg PRIVATE ffmpeg_ops.cpp)
endif()
target_link_libraries(bimg ${SYS_LIBS})
if(FFMPEG_FOUND)
    target_link_libraries(bimg ${FFMPEG_LIBRARIES})
    target_compile_definitions(bimg PRIVATE BAPI_HAVE_FFMPEG)
endif()
if(TP_RPATH)
  set_target_properties(bimg PROPERTIES BUILD_RPATH "${TP_RPATH}" INSTALL_RPATH "${TP_RPATH}")
endif()

add_executable(bimg_selfcheck selfcheck.cpp kernels.cpp)
