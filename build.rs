use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=TONO_RELEASE_MANIFEST");
    println!(
        "cargo:rustc-env=TONO_TARGET={}",
        env::var("TARGET").unwrap()
    );
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let manifest = env::var_os("TONO_RELEASE_MANIFEST").map(|path| {
        println!("cargo:rerun-if-changed={}", PathBuf::from(&path).display());
        fs::read_to_string(path).expect("read release runtime manifest")
    });
    fs::write(
        out.join("runtime-manifest.json"),
        manifest.unwrap_or_default(),
    )
    .unwrap();
    let mut files = Vec::new();
    for dir in ["instruments", "ml", "ml/tono_ml"] {
        println!("cargo:rerun-if-changed={dir}");
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "json" || e == "py") {
                files.push(path);
            }
        }
    }
    files.sort();
    let entries = files
        .iter()
        .map(|path| {
            let name = path.to_str().unwrap().replace('\\', "/");
            let absolute = fs::canonicalize(path).unwrap();
            format!(
                "({name:?}, include_bytes!({:?})),",
                absolute.to_str().unwrap()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(
        out.join("assets.rs"),
        format!("pub const ASSETS: &[(&str, &[u8])] = &[{entries}];"),
    )
    .unwrap();
}
