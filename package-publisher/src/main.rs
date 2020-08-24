#[macro_use]
extern crate tracing;

pub mod git;

use include_dir::{include_dir, Dir};
use std::env;
use std::fs;
use std::process::Command;

use crate::git::commit_file;
use clap::{crate_version, App, Arg};
use hex;
use reqwest;
use semver::Version;
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
            Arg::with_name("additional_message")
                .long("message")
                .about("message")
                .takes_value(true)
                .required(true),
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
                .long("version")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("github_token")
                .about("github-token")
                .long("github-token")
                .env("GITHUB_TOKEN")
                .takes_value(true)
                .required(true),
        )
        .get_matches();

    let github_token = matches
        .value_of("github_token")
        .expect("github token not set");

    let version: Version = matches
        .value_of("version")
        .expect("version not set")
        .parse()
        .expect("bad version");

    let version_string = version.to_string();

    let additional_message = matches
        .value_of("additional_message")
        .expect("additional_message not set");

    let macos_url = format!(
        "https://github.com/exogress/exogress/releases/download/{version}/exogress-{version}-x86_64-apple-darwin.tar.gz",
        version = version_string
    );
    let linux_url = format!("https://github.com/exogress/exogress/releases/download/{version}/exogress-{version}-x86_64-unknown-linux-gnu.tar.gz", version = version_string);
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

        let template = mustache::compile_str(homebrew_tpl).expect("Failed to compile");

        let mut data = HashMap::new();
        data.insert("MACOS_URL", macos_url.as_str());
        data.insert("LINUX_URL", linux_url.as_str());
        data.insert("VERSION", version_string.as_str());
        data.insert("MACOS_SHA256", macos_hash.as_str());
        data.insert("LINUX_SHA256", linux_hash.as_str());

        let mut bytes = vec![];

        template
            .render(&mut bytes, &data)
            .expect("Failed to render");

        let message = std::str::from_utf8(&bytes).unwrap();

        commit_file(
            HOMEBREW_FILE,
            message,
            &version,
            additional_message,
            "https://github.com/exogress/homebrew-brew.git"
                .parse()
                .unwrap(),
            github_token,
        )
        .unwrap()
    }
}
