use exogress_common::config_core::{ClientConfig, ProjectConfig, CURRENT_VERSION};
use std::path::PathBuf;

fn main() {
    let version = (*CURRENT_VERSION).0.to_string();

    let mut base_path = PathBuf::new();
    base_path.push("schemas");
    base_path.push(version);

    let mut client_path = base_path.clone();
    let mut project_path = base_path.clone();

    std::fs::create_dir_all(base_path).expect("failed to create version directory");

    client_path.push("client.json");
    project_path.push("project.json");

    match serde_json::to_string_pretty(&schemars::schema_for!(ClientConfig)) {
        Ok(txt) => {
            std::fs::write(client_path, txt).expect("couldn't write client config schema");
        }
        Err(e) => {
            println!("Error {}", e);
        }
    }

    match serde_json::to_string_pretty(&schemars::schema_for!(ProjectConfig)) {
        Ok(txt) => {
            std::fs::write(project_path, txt).expect("couldn't write project config schema");
        }
        Err(e) => {
            println!("Error {}", e);
        }
    }
}
