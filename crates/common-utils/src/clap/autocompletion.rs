use clap::{App, ArgMatches};
use clap_generate::generate;
use clap_generate::generators::*;

use std::io;

pub fn add_args(app: clap::App) -> clap::App {
    app.subcommand(
        App::new("autocompletion")
            .subcommand(App::new("fish"))
            .subcommand(App::new("bash"))
            .subcommand(App::new("zsh")),
    )
}

fn autodetect_shell() -> Option<&'static str> {
    let shell_var = std::env::var("SHELL").unwrap().to_lowercase();
    if shell_var.contains("fish") {
        Some("fish")
    } else if shell_var.contains("bash") {
        Some("bash")
    } else if shell_var.contains("zsh") {
        Some("zsh")
    } else {
        None
    }
}

pub fn handle_autocompletion(app: &mut clap::App, matches: &ArgMatches, cmd: &str) {
    if let Some(shell) = matches.subcommand_matches("autocompletion") {
        match shell.subcommand() {
            ("fish", _) => {
                generate::<Fish, _>(app, cmd, &mut io::stdout());
            }
            ("bash", _) => {
                generate::<Bash, _>(app, cmd, &mut io::stdout());
            }
            ("zsh", _) => {
                generate::<Zsh, _>(app, cmd, &mut io::stdout());
            }
            _ => {
                if let Some(shell) = autodetect_shell() {
                    eprintln!("Autodetect shell: {}", shell);
                    match shell {
                        "fish" => {
                            generate::<Fish, _>(app, cmd, &mut io::stdout());
                        }
                        "bash" => {
                            generate::<Bash, _>(app, cmd, &mut io::stdout());
                        }
                        "zsh" => {
                            generate::<Zsh, _>(app, cmd, &mut io::stdout());
                        }
                        _ => {
                            eprintln!("Could not autodetect shell");
                        }
                    }
                }
            }
        }

        std::process::exit(0);
    }
}
