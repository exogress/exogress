use clap::{App, Arg, ArgMatches, Shell};

use std::io;

pub fn add_args<'a>(app: clap::App<'a, 'a>) -> clap::App<'a, 'a> {
    app.subcommand(
        App::new("autocompletion").arg(Arg::with_name("shell").possible_values(&[
            "fish",
            "bash",
            "zsh",
            "powershell",
        ])),
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

pub fn handle_autocompletion<'a>(app: &'a mut clap::App<'a, 'a>, matches: &ArgMatches, cmd: &str) {
    if let Some(shell) = matches.subcommand_matches("autocompletion") {
        match shell.value_of("shell") {
            Some("fish") => {
                app.gen_completions_to(cmd, Shell::Fish, &mut io::stdout());
            }
            Some("bash") => {
                app.gen_completions_to(cmd, Shell::Bash, &mut io::stdout());
            }
            Some("zsh") => {
                app.gen_completions_to(cmd, Shell::Zsh, &mut io::stdout());
            }
            Some("powershell") => {
                app.gen_completions_to(cmd, Shell::PowerShell, &mut io::stdout());
            }
            _ => {
                if let Some(shell) = autodetect_shell() {
                    eprintln!("Autodetect shell: {}", shell);
                    match shell {
                        "fish" => {
                            app.gen_completions_to(cmd, Shell::Fish, &mut io::stdout());
                        }
                        "bash" => {
                            app.gen_completions_to(cmd, Shell::Bash, &mut io::stdout());
                        }
                        "zsh" => {
                            app.gen_completions_to(cmd, Shell::Zsh, &mut io::stdout());
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
