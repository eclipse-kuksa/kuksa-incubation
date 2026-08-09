cmake_minimum_required(VERSION 3.20)
project(open1722-ffi C)

find_package(Open1722 REQUIRED)

add_library(open1722_ffi SHARED open1722-ffi.c)
target_include_directories(open1722_ffi PRIVATE ${open1722_INCLUDE_DIR})
target_link_libraries(open1722_ffi PRIVATE open1722 open1722custom)

set_target_properties(open1722_ffi PROPERTIES
    VERSION 0.1.0
    SOVERSION 0
)
