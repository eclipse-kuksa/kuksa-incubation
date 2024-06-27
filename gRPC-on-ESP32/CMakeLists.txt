# The following lines of boilerplate have to be in your project's CMakeLists
# in this exact order for cmake to work correctly
cmake_minimum_required(VERSION 3.16)

# Add the directory containing Nanopb to the list of extra component directories
# This assumes that `nanopb` is a directory within `components`
set(EXTRA_COMPONENT_DIRS
        $ENV{IDF_PATH}/examples/common_components/protocol_examples_common
        ${CMAKE_CURRENT_LIST_DIR}/components/nanopb
)

include($ENV{IDF_PATH}/tools/cmake/project.cmake)
project(gRPC-on-esp32)
