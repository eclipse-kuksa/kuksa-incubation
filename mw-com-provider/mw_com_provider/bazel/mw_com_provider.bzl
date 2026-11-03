def mw_com_provider_cargo_commands(
        check_name = "cargo_check_mw_com_provider",
        test_name = "cargo_test_mw_com_provider",
    codegen_name = "generate_mw_com_provider_artifacts",
    verify_codegen_name = "verify_mw_com_provider_codegen",
        visibility = None):
    native.sh_binary(
        name = check_name,
        srcs = ["bazel/cargo_provider.sh"],
        args = ["check"],
        visibility = visibility,
    )

    native.sh_binary(
        name = test_name,
        srcs = ["bazel/cargo_provider.sh"],
        args = ["test"],
        visibility = visibility,
    )

    native.sh_binary(
        name = codegen_name,
        srcs = ["bazel/cargo_provider.sh"],
        args = ["codegen"],
        data = [
            ":provider_codegen_inputs",
            "//xtask:xtask_sources",
        ],
        visibility = visibility,
    )

    native.sh_binary(
        name = verify_codegen_name,
        srcs = ["bazel/cargo_provider.sh"],
        args = ["verify-codegen"],
        data = [
            ":provider_codegen_inputs",
            ":provider_generated_artifacts",
            "//xtask:xtask_sources",
        ],
        visibility = visibility,
    )

def mw_com_provider_score_lola_runtime_files(name, srcs, visibility = None):
    native.filegroup(
        name = name,
        srcs = srcs,
        visibility = visibility,
    )