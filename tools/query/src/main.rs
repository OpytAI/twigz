use std::fs;
use std::path::PathBuf;
use twigz_query::{compile_query, matches, QueryView};
use twigz_runtime::{javascript_lang, lua_lang, luau_lang, python_lang, twiglet_lang, Parser};

fn usage() -> ! {
    eprintln!("usage: twigz query --lang LANG --view semantic --source FILE QUERY");
    std::process::exit(2)
}

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut lang = None;
    let mut view = QueryView::Semantic;
    let mut source = None;
    let mut query = None;
    let mut index = 0;
    if args.first().map(String::as_str) == Some("query") {
        index = 1;
    }
    while index < args.len() {
        match args[index].as_str() {
            "--lang" => {
                lang = args.get(index + 1).cloned();
                index += 2;
            }
            "--view" => {
                view = match args.get(index + 1).map(String::as_str) {
                    Some("semantic") => QueryView::Semantic,
                    Some("concrete") => QueryView::Concrete,
                    _ => usage(),
                };
                index += 2;
            }
            "--source" => {
                source = args.get(index + 1).map(PathBuf::from);
                index += 2;
            }
            flag if flag.starts_with('-') => usage(),
            other => {
                query = Some(other.to_string());
                index += 1;
            }
        }
    }
    let lang = lang.unwrap_or_else(|| usage());
    let source = source.unwrap_or_else(|| usage());
    let query = query.unwrap_or_else(|| usage());
    let language = match lang.as_str() {
        "lua" => lua_lang(),
        "luau" => luau_lang(),
        "javascript" => javascript_lang(),
        "python" => python_lang(),
        "twiglet" => twiglet_lang(),
        other => {
            eprintln!("unknown language {other}");
            std::process::exit(1);
        }
    };
    let text = fs::read_to_string(&source).unwrap_or_else(|error| {
        eprintln!("{}: {error}", source.display());
        std::process::exit(1);
    });
    let mut parser = Parser::new(language).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(1);
    });
    let tree = parser.parse_str(&text).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(1);
    });
    match compile_query(&tree.language, &query, view) {
        Ok(compiled) => {
            for node in matches(&tree, &compiled, tree.root()) {
                let range = tree.range(node);
                println!(
                    "{}:{}: {}",
                    range.start_point.row + 1,
                    range.start_point.column + 1,
                    tree.text(node)
                );
            }
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
