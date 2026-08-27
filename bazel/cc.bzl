"""Compile C with the hermetic Zig toolchain. No Bazel CC toolchain."""

load("@bazel_skylib//lib:paths.bzl", "paths")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")

def _cc_headers_impl(ctx):
    includes = [
        paths.normalize(paths.join(ctx.label.workspace_root, ctx.label.package, directory))
        for directory in ctx.attr.includes
    ]
    return [
        DefaultInfo(files = depset(ctx.files.hdrs)),
        CcInfo(compilation_context = cc_common.create_compilation_context(
            headers = depset(ctx.files.hdrs),
            includes = depset(includes),
        )),
    ]

cc_headers = rule(
    implementation = _cc_headers_impl,
    attrs = {
        "hdrs": attr.label_list(allow_files = True),
        "includes": attr.string_list(),
    },
)

def _zig_executable(ctx):
    zigtoolchaininfo = ctx.toolchains["@rules_zig//zig:toolchain_type"].zigtoolchaininfo
    zig = zigtoolchaininfo.zig_exe.file
    if zig == None:
        fail("a hermetic Zig toolchain is required")
    return zig, zigtoolchaininfo

def _zig_tool_inputs(zigtoolchaininfo):
    tool_inputs = [zigtoolchaininfo.validation]
    if zigtoolchaininfo.zig_lib.file != None:
        tool_inputs.append(zigtoolchaininfo.zig_lib.file)
    return tool_inputs

def _zig_env(zigtoolchaininfo):
    return {
        "ZIG_GLOBAL_CACHE_DIR": zigtoolchaininfo.zig_cache,
        "ZIG_LOCAL_CACHE_DIR": zigtoolchaininfo.zig_cache,
    }

def _c_sources(files, attr_name):
    sources = [file for file in files if file.basename.endswith(".c")]
    if not sources:
        fail("%s must contain at least one .c file" % attr_name)
    return sources

def _add_compile_flags(args, ctx, compilation):
    if ctx.attr.target:
        args.add("-target")
        args.add(ctx.attr.target)
    # zig cc enables UBSan by default; rustc/gcc then cannot resolve __ubsan_*.
    args.add("-fno-sanitize=undefined")
    args.add_all(ctx.attr.copts)
    args.add_all(compilation.defines, format_each = "-D%s")
    args.add_all(compilation.includes, format_each = "-I%s")
    args.add_all(compilation.quote_includes, format_each = "-I%s")
    args.add_all(compilation.system_includes, before_each = "-isystem")

def _compile_c_object(ctx, zig, zigtoolchaininfo, compilation, source):
    obj = ctx.actions.declare_file("%s/%s.o" % (
        ctx.label.name,
        source.short_path.replace("..", "_").replace("/", "_").replace("\\", "_").rsplit(".", 1)[0],
    ))
    args = ctx.actions.args()
    _add_compile_flags(args, ctx, compilation)
    args.add("-c")
    args.add(source)
    args.add("-o")
    args.add(obj)
    ctx.actions.run(
        executable = zig,
        arguments = ["cc", args],
        env = _zig_env(zigtoolchaininfo),
        inputs = depset(
            direct = [source] + ctx.files.extra_srcs + _zig_tool_inputs(zigtoolchaininfo),
            transitive = [compilation.headers],
        ),
        outputs = [obj],
        tools = [zig],
        mnemonic = "ZigCcCompile",
        progress_message = "Compiling C %{label}",
    )
    return obj

def _zig_c_archive_impl(ctx):
    sources = _c_sources(ctx.files.srcs, "srcs")
    zig, zigtoolchaininfo = _zig_executable(ctx)
    cc_info = cc_common.merge_cc_infos(cc_infos = [dep[CcInfo] for dep in ctx.attr.deps])
    compilation = cc_info.compilation_context
    objects = [_compile_c_object(ctx, zig, zigtoolchaininfo, compilation, source) for source in sources]

    output = ctx.actions.declare_file("lib%s.a" % ctx.label.name)
    args = ctx.actions.args()
    args.add("rcs")
    args.add(output)
    args.add_all(objects)
    ctx.actions.run(
        executable = zig,
        arguments = ["ar", args],
        inputs = objects,
        outputs = [output],
        tools = [zig],
        mnemonic = "ZigArchiveBundle",
        progress_message = "Bundling C archive %{label}",
    )
    own = CcInfo(linking_context = cc_common.create_linking_context(
        linker_inputs = depset([cc_common.create_linker_input(
            owner = ctx.label,
            user_link_flags = [output.path],
            additional_inputs = depset([output]),
        )]),
    ))
    return [
        DefaultInfo(files = depset([output])),
        cc_common.merge_cc_infos(cc_infos = [own] + [dep[CcInfo] for dep in ctx.attr.deps]),
    ]

zig_c_archive = rule(
    implementation = _zig_c_archive_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True, mandatory = True),
        "copts": attr.string_list(),
        "extra_srcs": attr.label_list(allow_files = True),
        "deps": attr.label_list(providers = [CcInfo]),
        "target": attr.string(default = ""),
    },
    toolchains = ["@rules_zig//zig:toolchain_type"],
)

def _zig_c_link_impl(ctx, shared):
    sources = _c_sources(ctx.files.srcs, "srcs")
    zig, zigtoolchaininfo = _zig_executable(ctx)
    cc_info = cc_common.merge_cc_infos(cc_infos = [dep[CcInfo] for dep in ctx.attr.deps])
    compilation = cc_info.compilation_context
    output = ctx.actions.declare_file(ctx.attr.out if ctx.attr.out else ctx.label.name)
    args = ctx.actions.args()
    _add_compile_flags(args, ctx, compilation)
    if shared:
        args.add("-shared")
    args.add_all(ctx.attr.linkopts)
    args.add_all(sources)
    args.add("-o")
    args.add(output)
    ctx.actions.run(
        executable = zig,
        arguments = ["cc", args],
        env = _zig_env(zigtoolchaininfo),
        inputs = depset(
            direct = sources + ctx.files.extra_srcs + _zig_tool_inputs(zigtoolchaininfo),
            transitive = [compilation.headers],
        ),
        outputs = [output],
        tools = [zig],
        mnemonic = "ZigCcLink",
        progress_message = "Linking C %{label}",
    )
    return [DefaultInfo(
        files = depset([output]),
        runfiles = ctx.runfiles(files = [output]),
    )]

def _zig_c_shared_impl(ctx):
    return _zig_c_link_impl(ctx, True)

def _zig_c_wasm_impl(ctx):
    return _zig_c_link_impl(ctx, False)

_LINK_ATTRS = {
    "srcs": attr.label_list(allow_files = True, mandatory = True),
    "copts": attr.string_list(),
    "linkopts": attr.string_list(),
    "extra_srcs": attr.label_list(allow_files = True),
    "deps": attr.label_list(providers = [CcInfo]),
    "target": attr.string(default = ""),
    "out": attr.string(default = ""),
}

zig_c_shared = rule(
    implementation = _zig_c_shared_impl,
    attrs = _LINK_ATTRS,
    toolchains = ["@rules_zig//zig:toolchain_type"],
)

zig_c_wasm = rule(
    implementation = _zig_c_wasm_impl,
    attrs = _LINK_ATTRS,
    toolchains = ["@rules_zig//zig:toolchain_type"],
)
