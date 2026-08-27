"""Hermetic .grammar -> generated parser artifacts."""

load("//bazel:cc.bzl", "zig_c_shared")

TwigzGrammarInfo = provider(
    fields = {
        "grammar_json": "Normalized Tree-sitter grammar JSON.",
        "semantics": "Concrete-to-semantic projection.",
        "ir": "Normalized Grammar IR JSON.",
        "scanner_c": "Generated external scanner C, or empty file when unused.",
        "language": "Stable runtime language name.",
    },
)

def _twigz_grammar_impl(ctx):
    root = ctx.file.root
    module_inputs = []
    module_args = []
    for target, module_id in sorted(ctx.attr.modules.items(), key = lambda item: item[1]):
        files = target.files.to_list()
        if len(files) != 1:
            fail("twigz_grammar module %s must provide exactly one file" % target.label)
        module_inputs.append(files[0])
        module_args.extend(["--module", "%s=%s" % (module_id, files[0].path)])

    outs = struct(
        ir = ctx.actions.declare_file(ctx.label.name + "/grammar.ir.json"),
        grammar_json = ctx.actions.declare_file(ctx.label.name + "/grammar.json"),
        semantics = ctx.actions.declare_file(ctx.label.name + "/semantics.json"),
        diagnostics = ctx.actions.declare_file(ctx.label.name + "/diagnostics.json"),
        scanner_c = ctx.actions.declare_file(ctx.label.name + "/scanner.c"),
    )
    args = ctx.actions.args()
    args.add_all(["--root", root.path])
    args.add_all(module_args)
    args.add_all(["--ir", outs.ir.path, "--grammar-json", outs.grammar_json.path])
    args.add_all(["--semantics", outs.semantics.path, "--diagnostics", outs.diagnostics.path])
    args.add_all(["--scanner-c", outs.scanner_c.path])
    all_outputs = [outs.ir, outs.grammar_json, outs.semantics, outs.diagnostics, outs.scanner_c]
    ctx.actions.run(
        executable = ctx.executable._generator,
        arguments = [args],
        inputs = depset([root] + module_inputs),
        outputs = all_outputs,
        mnemonic = "TwigzGrammarGen",
        progress_message = "Generating grammar %{label}",
    )
    return [
        DefaultInfo(files = depset(all_outputs)),
        TwigzGrammarInfo(
            grammar_json = outs.grammar_json,
            semantics = outs.semantics,
            ir = outs.ir,
            scanner_c = outs.scanner_c,
            language = ctx.attr.language,
        ),
        OutputGroupInfo(
            semantics = depset([outs.semantics]),
            scanner = depset([outs.scanner_c]),
            inspection = depset([outs.ir, outs.grammar_json, outs.diagnostics]),
        ),
    ]

twigz_grammar = rule(
    implementation = _twigz_grammar_impl,
    attrs = {
        "root": attr.label(allow_single_file = [".grammar"], mandatory = True),
        "modules": attr.label_keyed_string_dict(allow_files = [".grammar"]),
        "language": attr.string(mandatory = True),
        "_generator": attr.label(
            default = Label("//tools/grammar-gen:twigz-grammar-gen"),
            executable = True,
            cfg = "exec",
        ),
    },
)

TwigzPackInfo = provider(
    fields = {
        "csrcs": "Generated per-language parsers plus the shared immutable table pool.",
        "registry_json": "Canonical pack table.",
        "registry_rs": "Generated Rust registry.",
        "report": "Deterministic parser sharing report.",
        "scanners": "Generated scanner C files.",
    },
)

def _twigz_pack_impl(ctx):
    tables_c = ctx.actions.declare_file(ctx.label.name + "/shared_tables.c")
    registry_json = ctx.actions.declare_file(ctx.label.name + "/registry.json")
    registry_rs = ctx.actions.declare_file(ctx.label.name + "/registry.rs")
    report = ctx.actions.declare_file(ctx.label.name + "/report.json")
    parsers = []
    node_types = []
    manifests = []
    scanners = []
    glues = []
    inputs = []
    args = ctx.actions.args()
    grammars = sorted([target[TwigzGrammarInfo] for target in ctx.attr.grammars], key = lambda info: info.language)
    for grammar in grammars:
        parser = ctx.actions.declare_file(ctx.label.name + "/" + grammar.language + "/parser.c")
        nodes = ctx.actions.declare_file(ctx.label.name + "/" + grammar.language + "/node-types.json")
        manifest = ctx.actions.declare_file(ctx.label.name + "/" + grammar.language + "/manifest.json")
        glue = ctx.actions.declare_file(ctx.label.name + "/" + grammar.language + "/glue.c")
        args.add_all([
            "--language",
            grammar.language,
            grammar.grammar_json.path,
            grammar.semantics.path,
            parser.path,
            nodes.path,
            manifest.path,
        ])
        inputs.extend([grammar.grammar_json, grammar.semantics, grammar.scanner_c])
        parsers.append(parser)
        node_types.append(nodes)
        manifests.append(manifest)
        scanners.append(grammar.scanner_c)
        glues.append(glue)
    args.add_all([
        "--tables-c",
        tables_c.path,
        "--registry-json",
        registry_json.path,
        "--report",
        report.path,
    ])
    outputs = [tables_c, registry_json, registry_rs, report] + parsers + node_types + manifests + glues
    ctx.actions.run(
        executable = ctx.executable._packer,
        arguments = [args],
        inputs = depset(inputs),
        outputs = outputs,
        mnemonic = "TwigzPack",
        progress_message = "Packing parsers %{label}",
    )
    return [
        DefaultInfo(files = depset(outputs + scanners)),
        TwigzPackInfo(
            csrcs = depset([tables_c] + parsers),
            registry_json = registry_json,
            registry_rs = registry_rs,
            report = report,
            scanners = depset(scanners),
        ),
        OutputGroupInfo(
            csrcs = depset([tables_c] + parsers),
            registry_json = depset([registry_json]),
            registry_rs = depset([registry_rs]),
            node_types = depset(node_types),
            manifests = depset(manifests),
            report = depset([report]),
            scanners = depset(scanners),
            glues = depset(glues),
        ),
    ]

twigz_pack = rule(
    implementation = _twigz_pack_impl,
    attrs = {
        "grammars": attr.label_list(providers = [TwigzGrammarInfo], mandatory = True),
        "_packer": attr.label(
            default = Label("//tools/pack:twigz-pack"),
            executable = True,
            cfg = "exec",
        ),
    },
)

def twigz_language_cdylib(name, grammar, soname, visibility = None):
    pack_name = name + "_pack"
    twigz_pack(
        name = pack_name,
        grammars = [grammar],
    )
    native.filegroup(
        name = name + "_csrcs",
        srcs = [":" + pack_name],
        output_group = "csrcs",
    )
    native.filegroup(
        name = name + "_scanners",
        srcs = [":" + pack_name],
        output_group = "scanners",
    )
    native.filegroup(
        name = name + "_glues",
        srcs = [":" + pack_name],
        output_group = "glues",
    )
    zig_c_shared(
        name = name,
        srcs = [
            ":" + name + "_csrcs",
            ":" + name + "_scanners",
            ":" + name + "_glues",
            "@tree_sitter//:runtime_c",
        ],
        copts = [
            "-std=c11",
            "-fPIC",
            "-D_POSIX_C_SOURCE=200112L",
            "-D_DEFAULT_SOURCE",
        ],
        extra_srcs = [
            "@tree_sitter//:runtime_support",
            "@tree_sitter//:runtime_header_files",
        ],
        linkopts = ["-Wl,-soname," + soname] if soname else [],
        out = soname if soname else name + ".so",
        deps = ["@tree_sitter//:headers"],
        visibility = visibility,
    )
    native.filegroup(
        name = name + "_so",
        srcs = [":" + name],
        visibility = visibility,
    )
