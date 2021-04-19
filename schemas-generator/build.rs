use exogress_common::config_core::{
    referenced, referenced::ReferencedConfigValue, ClientConfig, ProjectConfig, CURRENT_VERSION,
};
use std::path::PathBuf;

fn gen_param_schema<T: ReferencedConfigValue>() {
    let schema = schemars::schema_for!(T);
    let txt = serde_json::to_string_pretty(&schema).unwrap();
    let schema_name = T::schema().to_string();
    let mut path = PathBuf::new();
    path.push("..");
    path.push("schemas");
    path.push("parameters");
    path.push(format!("{}.json", schema_name));

    std::fs::write(path, &txt).expect("couldn't write client config schema");
}

fn main() {
    // Generate config version
    let version = (*CURRENT_VERSION).0.to_string();

    let mut base_path = PathBuf::new();
    base_path.push("..");
    base_path.push("schemas");
    base_path.push("config");
    base_path.push(CURRENT_VERSION.major_base_version());
    base_path.push(CURRENT_VERSION.minor_base_version());
    base_path.push(version);

    let mut client_path = base_path.clone();
    let mut project_path = base_path.clone();

    std::fs::create_dir_all(base_path).expect("failed to create version directory");

    client_path.push("client.json");
    project_path.push("project.json");

    println!("save to {}", client_path.to_str().unwrap());
    println!("save to {}", project_path.to_str().unwrap());

    match serde_json::to_string_pretty(&schemars::schema_for!(ClientConfig)) {
        Ok(txt) => {
            std::fs::write(client_path, &txt).expect("couldn't write client config schema");
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

    gen_param_schema::<referenced::aws::credentials::AwsCredentials>();
    gen_param_schema::<referenced::aws::bucket::S3Bucket>();
    gen_param_schema::<referenced::google::credentials::GoogleCredentials>();
    gen_param_schema::<referenced::google::bucket::GcsBucket>();
    gen_param_schema::<referenced::acl::Acl>();
    gen_param_schema::<referenced::mime_types::MimeTypes>();
    gen_param_schema::<referenced::static_response::StaticResponse>();
}
