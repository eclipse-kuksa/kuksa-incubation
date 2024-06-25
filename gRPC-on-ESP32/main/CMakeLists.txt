set (
        srcs
        "main.c"
        "grpc.c"
        "encoder.c"
        "decoder.c"
        "generated/val.pb.c"
        "generated/types.pb.c"
)

set(
        include_dirs
            "."
            "generated/"
)

idf_component_register(
        SRCS            "${srcs}"
        INCLUDE_DIRS    "${include_dirs}"#EMBED_TXTFILES ../certs/server_cert.pem
)

target_compile_options(${COMPONENT_LIB} PRIVATE "-Wno-format")
