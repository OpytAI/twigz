"""Build a cc_binary for wasm32-wasip1 and expose the wasm as a host file."""

def _wasi_transition_impl(_settings, _attr):
    return {"//command_line_option:platforms": str(Label("//platforms:wasm32_wasip1"))}

wasi_transition = transition(
    implementation = _wasi_transition_impl,
    inputs = [],
    outputs = ["//command_line_option:platforms"],
)

def _wasi_binary_impl(ctx):
    src = ctx.executable.bin
    out = ctx.actions.declare_file(ctx.label.name + ".wasm")
    ctx.actions.symlink(output = out, target_file = src)
    return DefaultInfo(
        files = depset([out]),
        runfiles = ctx.runfiles(files = [out]),
    )

wasi_binary = rule(
    implementation = _wasi_binary_impl,
    attrs = {
        "bin": attr.label(
            allow_single_file = True,
            cfg = wasi_transition,
            executable = True,
            mandatory = True,
        ),
        "_allowlist_function_transition": attr.label(
            default = "@bazel_tools//tools/allowlists/function_transition_allowlist",
        ),
    },
)
