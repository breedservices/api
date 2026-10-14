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

add_library(kernels STATIC kernels.cpp)
add_library(imgfull STATIC kernels.cpp ocr.cpp vips_ops.cpp)

add_library(bimg SHARED kernels.cpp ocr.cpp vips_ops.cpp shim.cpp)
target_link_libraries(bimg ${SYS_LIBS})
if(TP_RPATH)
  set_target_properties(bimg PROPERTIES BUILD_RPATH "${TP_RPATH}" INSTALL_RPATH "${TP_RPATH}")
endif()

add_executable(bimg_selfcheck selfcheck.cpp kernels.cpp)
