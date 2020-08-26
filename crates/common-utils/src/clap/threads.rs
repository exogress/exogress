use clap::{Arg, ArgMatches};
use lazy_static::lazy_static;

lazy_static! {
    static ref DEFAULT_NUM_THREADS: std::string::String = num_cpus::get().to_string();
}

pub fn add_args<'a>(app: clap::App<'a, 'a>) -> clap::App<'a, 'a> {
    app.arg(
        Arg::with_name("num_threads")
            .long("num-threads")
            .value_name("NUMBER")
            .default_value(&DEFAULT_NUM_THREADS)
            .required(true)
            .help("Number of threads to use")
            .takes_value(true),
    )
}

pub fn extract_matches(matches: &ArgMatches) -> usize {
    let num_threads: usize = matches
        .value_of("num_threads")
        .expect("no num_threads provided")
        .parse()
        .expect("bad num_threads");

    info!("Using {} threads", num_threads);

    num_threads
}
