use std::{env, fs, io, path::Path};

const EMBEDDED_WEB_CFG: &str = "lightbws_embedded_web";

fn main() {
    println!("cargo:rustc-check-cfg=cfg({EMBEDDED_WEB_CFG})");
    for path in [
        "web/index.html",
        "web/package.json",
        "web/package-lock.json",
        "web/tsconfig.json",
        "web/vite.config.ts",
        "web/public",
        "web/src",
        "web/tokens.css",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    if env::var("PROFILE").as_deref() != Ok("debug") {
        let index = Path::new("web/dist/index.html");
        if !index.is_file() {
            panic!("release Web bundle is missing; run `npm --prefix web run build` first");
        }
        println!("cargo:rustc-cfg={EMBEDDED_WEB_CFG}");
        let count = track_assets(Path::new("web/dist"))
            .unwrap_or_else(|error| panic!("failed to inspect Web bundle: {error}"));
        if count == 0 {
            panic!("release Web bundle is empty");
        }
    }
}

fn track_assets(directory: &Path) -> io::Result<usize> {
    println!("cargo:rerun-if-changed={}", directory.display());
    let mut files = 0;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            files += track_assets(&path)?;
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
            files += 1;
        }
    }
    Ok(files)
}
