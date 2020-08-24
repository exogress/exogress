#[macro_use]
extern crate tracing;

mod termination;

use std::collections::VecDeque;
use std::process::Stdio;

use crate::termination::StopReason;
use clap::{crate_version, App, Arg};
use exogress_client_core::Client;
use exogress_common_utils::termination::stop_signal_listener;
use exogress_entities::{InstanceId, Ulid};
use futures::future::Either;
use futures::{future, select_biased, FutureExt};
use stop_handle::stop_handle;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::runtime::{Builder, Handle};

use exogress_config_core::DEFAULT_CONFIG_FILE;
use trust_dns_resolver::TokioAsyncResolver;

pub fn main() {
    let spawn_args = App::new("spawn")
        .about("spawn exogress client")
        .arg(
            Arg::with_name("command")
                .about("Run this command")
                .last(true)
                .multiple(true),
        )
        .arg(
            Arg::with_name("no_watch_config")
                .long("no-watch")
                .about("Don't watch for config changes")
                .takes_value(false)
                .required(false),
        )
        .arg(
            Arg::with_name("client_id")
                .long("client-id")
                .value_name("ULID")
                .about("CLIENT_ID")
                .env("EXG_CLIENT_ID")
                .hide_env_values(true)
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("client_secret")
                .long("client-secret")
                .value_name("STRING")
                .about("CLIENT_SECRET")
                .env("EXG_CLIENT_SECRET")
                .hide_env_values(true)
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("account")
                .long("account")
                .value_name("STRING")
                .about("Account")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("project")
                .long("project")
                .value_name("STRING")
                .about("Project")
                .required(true)
                .takes_value(true),
        );

    let args = App::new("Exogress CLI")
        .version(crate_version!())
        .author("Exogress Team <team@exogress.com>")
        .about("Expose your app to Exogress cloud load balancer")
        .arg(
            Arg::with_name("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .about("Sets a custom config file")
                .env("EXG_CONFIG_FILE")
                .default_value(DEFAULT_CONFIG_FILE)
                .takes_value(true),
        )
        .subcommand(exogress_common_utils::clap::threads::add_args(
            exogress_common_utils::clap::log::add_args(spawn_args),
        ));

    let mut args = exogress_common_utils::clap::autocompletion::add_args(args);

    let matches = args.clone().get_matches();
    exogress_common_utils::clap::autocompletion::handle_autocompletion(
        &mut args, &matches, "exogress",
    );

    let spawn_matches = matches
        .subcommand_matches("spawn")
        .expect("unknown subcommand");

    exogress_common_utils::clap::log::handle(&spawn_matches, "exogress");
    let num_threads = exogress_common_utils::clap::threads::extract_matches(&spawn_matches);

    let mut rt = Builder::new()
        .threaded_scheduler()
        .enable_all()
        .core_threads(num_threads)
        .thread_name("exogress-reactor")
        .build()
        .unwrap();

    let config_path = matches
        .value_of("config")
        .expect("--config is not set")
        .to_string();
    let should_watch_config = !spawn_matches.is_present("no_watch_config");

    let client_id: Ulid = spawn_matches
        .value_of("client_id")
        .expect("client_id is not set")
        .parse()
        .expect("client_id is not ULID");

    let client_secret = spawn_matches
        .value_of("client_secret")
        .expect("client_secret is not set")
        .to_string();

    let account = spawn_matches
        .value_of("account")
        .expect("account is not set")
        .to_string();

    let project = spawn_matches
        .value_of("project")
        .expect("project is not set")
        .to_string();

    let (app_stop_handle, app_stop_wait) = stop_handle::<StopReason>();

    let instance_id = InstanceId::new();

    info!("Using instance_id {}", instance_id);

    rt.block_on(async move {
        tokio::spawn(stop_signal_listener(app_stop_handle.clone()));

        let resolver = TokioAsyncResolver::from_system_conf(Handle::current())
            .await
            .unwrap();

        let process = match spawn_matches.values_of("command") {
            Some(cmd_and_args) if cmd_and_args.len() > 0 => {
                let mut c: VecDeque<_> = cmd_and_args.collect();

                let mut command = Command::new(c.pop_front().expect("FIXME"));

                command
                    .args(c)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());

                let mut child = command.spawn().expect("failed to spawn command");

                let stderr = child.stderr.take().unwrap();
                let stdout = child.stdout.take().unwrap();

                let mut stdout_reader = BufReader::new(stdout).lines();
                let mut stderr_reader = BufReader::new(stderr).lines();

                let stdout_forward = {
                    async move {
                        while let Ok(Some(line)) = stdout_reader.next_line().await {
                            info!("O {}", line);
                        }
                    }
                }
                .fuse();

                let stderr_forward = {
                    async move {
                        while let Ok(Some(line)) = stderr_reader.next_line().await {
                            info!("E {}", line);
                        }
                    }
                }
                .fuse();

                Either::Left(async move {
                    futures::pin_mut!(stdout_forward);
                    futures::pin_mut!(stderr_forward);

                    select_biased! {
                        _ = child.fuse() => {}
                        _ = stdout_forward => {}
                        _ = stderr_forward => {}
                    }
                })
            }
            _ => {
                info!("running in standalone mode");
                Either::Right(future::pending())
            }
        }
        .fuse();

        let client = Client::builder()
            .config_path(config_path)
            .client_id(client_id)
            .client_secret(client_secret)
            .account(account)
            .project(project)
            .instance_id(instance_id)
            .watch_config(should_watch_config)
            .build()
            .unwrap()
            .spawn(resolver)
            .fuse();

        tokio::select! {
            r = client => {
                if let Err(e) = r {
                    error!("client stopped with error: {:?}", e);
                }
            },
            _ = process => {},
            r = app_stop_wait => {
                info!("stop request received: {}", r);
            },
        }
    });

    info!("we are done");
}
