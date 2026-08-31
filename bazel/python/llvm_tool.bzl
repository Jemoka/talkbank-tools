"""Selects one host executable from an LLVM distribution."""


def _llvm_tool_file_impl(ctx):
    for executable_name in ctx.attr.executable_names:
        matches = [file for file in ctx.files.srcs if file.basename == executable_name]
        if len(matches) > 1:
            fail("LLVM distribution contains multiple {} executables".format(executable_name))
        if matches:
            return DefaultInfo(files = depset(matches))

    fail("LLVM distribution contains none of: {}".format(
        ", ".join(ctx.attr.executable_names),
    ))


llvm_tool_file = rule(
    implementation = _llvm_tool_file_impl,
    attrs = {
        "executable_names": attr.string_list(mandatory = True),
        "srcs": attr.label_list(allow_files = True, mandatory = True),
    },
)
