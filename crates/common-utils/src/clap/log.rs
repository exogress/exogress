use clap::{Arg, ArgMatches};
use tracing::Level;

//
// lazy_static! {
//     static ref DEFAULT_HOSTNAME: String =
//         hostname::get_hostname().unwrap_or_else(|| "unknown".into());
// }

pub fn add_args<'a>(app: clap::App<'a, 'a>) -> clap::App<'a, 'a> {
    app
        // .arg(
        //     Arg::with_name("gelf_own_hostname")
        //         .long("gelf-individual-hostname")
        //         .value_name("HOSTNAME")
        //         .about("Use provided hostname as a gelf hostname")
        //         .takes_value(true)
        //         .required(false)
        //         .default_value(&DEFAULT_HOSTNAME),
        // )
        // .arg(
        //     Arg::with_name("gelf_server")
        //         .long("gelf-server")
        //         .value_name("SOCKET_ADDR")
        //         .about("Log to GELF server")
        //         .takes_value(true)
        //         .required(false),
        // )
        .arg(
            Arg::with_name("log_level")
                .long("log-level")
                .env("LOG_LEVEL")
                .value_name("LOG_LEVEL")
                .help("Log level")
                .default_value("INFO")
                .case_insensitive(true)
                .possible_values(&["trace", "debug", "info", "warn", "error"])
                .required(true)
                .takes_value(true),
        )
}

pub fn handle(matches: &ArgMatches, _service_name: &'static str) {
    let log_level = match &matches
        .value_of("log_level")
        .expect("Please provide --log-level")
        .to_lowercase()[..]
    {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => panic!("Bad log level"),
    };

    let subscriber = tracing_subscriber::fmt().with_max_level(log_level).finish();

    tracing::subscriber::set_global_default(subscriber).expect("no global subscriber has been set");
}
