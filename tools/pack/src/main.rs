use std::path::PathBuf;
use twigz_pack::{run, LanguageInput, PackOptions};

fn usage() -> ! {
    eprintln!(
        "usage: twigz-pack --language NAME GRAMMAR_JSON SEMANTICS_JSON PARSER_C NODE_TYPES MANIFEST [--language ...] --tables-c FILE --registry-json FILE --report FILE\n"
    );
    std::process::exit(2)
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut index = 0;
    let mut languages = Vec::new();
    let mut tables_c = None;
    let mut registry_json = None;
    let mut report = None;
    while index < args.len() {
        match args[index].as_str() {
            "--language" => {
                if index + 6 >= args.len() {
                    usage();
                }
                languages.push(LanguageInput {
                    name: args[index + 1].clone(),
                    version: String::new(),
                    grammar: PathBuf::from(&args[index + 2]),
                    semantics: PathBuf::from(&args[index + 3]),
                    parser_out: PathBuf::from(&args[index + 4]),
                    node_types_out: PathBuf::from(&args[index + 5]),
                    manifest_out: PathBuf::from(&args[index + 6]),
                });
                index += 7;
            }
            "--tables-c" | "--registry-json" | "--report" => {
                let value = args.get(index + 1).unwrap_or_else(|| usage());
                match args[index].as_str() {
                    "--tables-c" => tables_c = Some(PathBuf::from(value)),
                    "--registry-json" => registry_json = Some(PathBuf::from(value)),
                    "--report" => report = Some(PathBuf::from(value)),
                    _ => unreachable!(),
                }
                index += 2;
            }
            _ => usage(),
        }
    }
    let options = PackOptions {
        languages,
        tables_c: tables_c.unwrap_or_else(|| usage()),
        registry_json: registry_json.unwrap_or_else(|| usage()),
        report: report.unwrap_or_else(|| usage()),
    };
    if let Err(error) = run(options) {
        eprintln!("twigz-pack: {error}");
        std::process::exit(1);
    }
}
