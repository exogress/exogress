use git2::build::RepoBuilder;
use git2::{Cred, IndexAddOption, PushOptions, RemoteCallbacks, Signature};
use semver::Version;
use std::io::{Read, Write};
use std::path::Path;
use std::{fs, io};
use tempfile::tempdir;
use url::Url;

fn pause() {
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();

    // We want the cursor to stay at the end of the line, so we print without a newline and flush manually.
    write!(stdout, "Press any key to continue...").unwrap();
    stdout.flush().unwrap();

    // Read a single byte and discard
    let _ = stdin.read(&mut [0u8]).unwrap();
}

pub fn commit_file(
    file_path: impl AsRef<Path>,
    content: &str,
    version: &Version,
    additional_message: &str,
    repo_url: Url,
    token: &str,
) -> Result<(), anyhow::Error> {
    let clone_to = tempdir()?;
    info!("clone to {:?}", clone_to);
    let repo = RepoBuilder::new().clone(repo_url.as_str(), clone_to.path())?;
    let absolute_path = clone_to.path().join(file_path.as_ref());
    fs::write(absolute_path.clone(), content)?;
    info!("{:?} saved", absolute_path.to_str());
    let mut index = repo.index()?;
    info!("add {:?}", file_path.as_ref().to_str());
    index.add_all(
        [file_path.as_ref().to_str().unwrap()].iter(),
        IndexAddOption::DEFAULT,
        None,
    )?;
    info!("write index");
    index.write()?;
    let tree_id = index.write_tree()?;

    let signature = Signature::now("Package Publisher", "team@exogress.com")?;
    let parent = repo
        .head()
        .ok()
        .and_then(|h| h.target())
        .ok_or(anyhow::Error::msg("no head"))?;
    let parent = repo.find_commit(parent)?;
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        format!("{}: {}", version, additional_message).as_str(),
        &repo.find_tree(tree_id).unwrap(),
        &[&parent],
    )?;

    info!("find origin");
    let mut origin = repo.find_remote("origin")?;
    info!("push");
    let mut push_options = PushOptions::new();
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|url, username_from_url, allowed_types| {
        info!(
            "credentials callback: {:?}, {:?}, {:?}",
            url, username_from_url, allowed_types
        );
        Cred::userpass_plaintext(token, "")
    });

    callbacks.sideband_progress(|t| unsafe {
        let s = std::str::from_utf8(t).expect("bad string");
        info!("GIT PROGRESS: {}", s);
        true
    });

    callbacks.push_update_reference(|s1, s2| {
        info!("GIT UPDATE: {:?}, {:?}", s1, s2);
        Ok(())
    });

    push_options.remote_callbacks(callbacks);
    origin.push::<String>(
        &["refs/heads/master:refs/heads/master".to_string()],
        Some(&mut push_options),
    )?;

    pause();

    Ok(())
}
