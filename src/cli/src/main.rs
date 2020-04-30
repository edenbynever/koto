use clap::{App, Arg};
use koto::Koto;
use std::fs;

mod repl;
use repl::Repl;

fn main() {
    let matches = App::new("Koto")
        .version(env!("CARGO_PKG_VERSION"))
        .arg(
            Arg::with_name("show_bytecode")
                .short("b")
                .long("show_bytecode")
                .help("Show the script's compiled bytecode"),
        )
        .arg(
            Arg::with_name("show_annotated")
                .short("B")
                .long("show_annotated")
                .help("Show compiled bytecode annotated with source lines"),
        )
        .arg(
            Arg::with_name("script")
                .help("The koto script to run")
                .index(1),
        )
        .arg(
            Arg::with_name("args")
                .help("Arguments to pass into the script")
                .multiple(true)
                .last(true),
        )
        .get_matches();

    let mut options = koto::Options::default();
    options.show_bytecode = matches.is_present("show_bytecode");
    options.show_annotated = matches.is_present("show_annotated");

    if let Some(path) = matches.value_of("script") {
        let mut koto = Koto::with_options(options);

        let args = match matches.values_of("args") {
            Some(args) => args.map(|s| s.to_string()).collect::<Vec<_>>(),
            None => Vec::new(),
        };

        let script = fs::read_to_string(path).expect("Unable to load path");
        koto.set_script_path(Some(path.to_string()));
        match koto.run_script_with_args(&script, &args) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("{}", e);
            }
        }
    } else {
        let mut repl = Repl::with_options(options);
        repl.run();
    }
}
