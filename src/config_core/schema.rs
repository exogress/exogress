use crate::config_core::{ConfigVersion, CONFIG_SCHEMAS};
use anyhow::{anyhow, bail};
use std::str::FromStr;
use valico::json_schema;

pub fn validate_schema(yaml_data: impl AsRef<[u8]>, schema_file_name: &str) -> anyhow::Result<()> {
    let cfg: serde_yaml::Value = serde_yaml::from_slice(yaml_data.as_ref())?;
    let version = cfg
        .get("version")
        .ok_or_else(|| anyhow!("no version supplied"))
        .and_then(|v| {
            Ok(ConfigVersion::from_str(
                v.as_str().ok_or_else(|| anyhow!("bad version format"))?,
            )?)
        })?;
    let json_cfg_value: serde_json::Value = serde_json::from_slice(&serde_json::to_vec(&cfg)?)?;

    let res = CONFIG_SCHEMAS.get_file(&format!(
        "{}/{}/{}/{}",
        version.major_base_version(),
        version.minor_base_version(),
        version.0.to_string(),
        schema_file_name
    ));

    match res {
        Some(schema_file) => {
            let schema = serde_json::from_slice(schema_file.contents()).unwrap();
            let mut scope = json_schema::Scope::new();
            let schema = scope.compile_and_return(schema, true).unwrap();
            let validation = schema.validate(&json_cfg_value);
            if validation.is_strictly_valid() {
                Ok(())
            } else {
                let err = serde_json::to_string(&validation.errors).unwrap();
                bail!("validation error: `{}`", err)
            }
        }
        None => bail!("version is not supported"),
    }
}
