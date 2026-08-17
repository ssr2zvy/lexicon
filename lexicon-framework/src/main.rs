fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("test") => {
            println!("test::: installation successful and routed through the cli to the framework");
        }
        Some(other) => {
            eprintln!("lexicon-framework: unknown command \"{other}\"");
            std::process::exit(1);
        }
        None => {
            println!("lexicon-framework: no command given");
        }
    }
}