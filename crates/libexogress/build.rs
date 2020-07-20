extern crate cbindgen;

use cbindgen::Language;
use std::env;
use std::fs::{File, OpenOptions};
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;

fn main() {
    let out_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    // let version = env::var("TRUNK_VERSION")
    //     .ok()
    //     .and_then(|v| {
    //         //Empty string in fact means None
    //         if v.is_empty() {
    //             None
    //         } else {
    //             Some(v)
    //         }
    //     })
    //     .or_else(|| {
    //         env::var("CARGO_PKG_VERSION")
    //             .ok()
    //             .map(|cargo_version| format!("{}-DEV", cargo_version))
    //     })
    //     .expect("Failed to generate version");
    // println!("cargo:rustc-env=TRUNK_VERSION={}", version);

    let path = format!("{}/{}", out_dir, "exogress.h");

    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    match cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_language(Language::C)
        .with_header(&format!(
            "\
             #ifdef __cplusplus\n\
             extern \"C\" {{\n\
             #endif\n\n",
            //             #define TRUNK_HEADER_VERSION \"{}\"",
            //             version
        ))
        .with_trailer(
            "#ifdef __cplusplus\n\
             }\n\
             #endif\n",
        )
        .generate()
    {
        Ok(r) => {
            r.write_to_file(path);
        }
        Err(e) => panic!("Could not generate bindings with cbindgen: {:?}", e),
    }
}
