#[macro_use]
extern crate tracing;

use include_dir::{include_dir, Dir};
use std::env;
use std::fs;
use std::process::Command;

use clap::{crate_version, App, Arg};
use hex;
use reqwest;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use tokio;
use tracing::Level;

const TEMPLATES_DIR: Dir = include_dir!("templates");

const HOMEBREW_FILE: &str = "exogress.rb";
async fn fetch_archive(url: &str) -> Vec<u8> {
    reqwest::get(url)
        .await
        .unwrap()
        .bytes()
        .await
        .unwrap()
        .to_vec()
}

fn hash_archive(archive: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.input(archive);
    hex::encode(&hasher.result()[..])
}

fn git_add_commit_push(version: &str) {
    Command::new("git").args(&["add", "."]).status().unwrap();
    Command::new("git")
        .args(&["commit", "-m", version])
        .status()
        .unwrap();
    Command::new("git").args(&["push"]).status().unwrap();
}

#[tokio::main]
async fn main() {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("no global subscriber has been set");

    let matches = App::new("Exogress Publisher")
        .version(crate_version!())
        .author("Exogress Team <team@exogress.com>")
        .about("Publish exogress binaries to package repositories")
        .arg(
            Arg::with_name("homebrew_repo_dir")
                .long("homebrew-repo-dir")
                .takes_value(true)
                .required(true)
                .default_value("./homebrew-brew"),
        )
        .arg(
            Arg::with_name("homebrew")
                .long("homebrew")
                .takes_value(false)
                .required(false),
        )
        .arg(
            Arg::with_name("version")
                .about("version")
                .last(true)
                .multiple(false),
        )
        .get_matches();

    let version = matches.value_of("version").expect("version not set");
    let homebrew_repo_dir = matches
        .value_of("homebrew_repo_dir")
        .expect("homebrew_repo_dir not set");

    let macos_url = format!(
        "https://github.com/exogress/exogress/releases/download/v{version}/exogress-v{version}-x86_64-apple-darwin.tar.gz",
        version = version
    );
    let linux_url = format!("https://github.com/exogress/exogress/releases/download/v{version}/exogress-v{version}-x86_64-unknown-linux-gnu.tar.gz", version = version);
    // let repo_url = format!(
    //     "https://github.com/exogress/exogress/archive/{}.tar.gz",
    //     version
    // );

    let macos_archive = fetch_archive(&macos_url).await;
    let linux_archive = fetch_archive(&linux_url).await;

    let macos_hash = hash_archive(&macos_archive);
    let linux_hash = hash_archive(&linux_archive);

    if matches.is_present("homebrew") {
        info!("generate homebrew...");
        let homebrew_tpl = TEMPLATES_DIR
            .get_file("homebrew")
            .unwrap()
            .contents_utf8()
            .unwrap();

        env::set_current_dir(homebrew_repo_dir).unwrap();

        let template = mustache::compile_str(homebrew_tpl).expect("Failed to compile");

        let mut data = HashMap::new();
        data.insert("MACOS_URL", macos_url.as_str());
        data.insert("LINUX_URL", linux_url.as_str());
        data.insert("VERSION", version);
        data.insert("MACOS_SHA256", macos_hash.as_str());
        data.insert("LINUX_SHA256", linux_hash.as_str());

        let mut bytes = vec![];

        template
            .render(&mut bytes, &data)
            .expect("Failed to render");

        let message = std::str::from_utf8(&bytes).unwrap();

        fs::write(HOMEBREW_FILE, message).unwrap();
    }

    git_add_commit_push(version);
}
