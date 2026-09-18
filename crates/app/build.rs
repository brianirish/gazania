use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ui_out = out.join("ui");
    std::fs::create_dir_all(&ui_out).unwrap();

    let blp_dir = PathBuf::from("src/ui");
    let mut blps: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&blp_dir).expect("src/ui exists") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) == Some("blp") {
            println!("cargo:rerun-if-changed={}", path.display());
            blps.push(path);
        }
    }

    let status = Command::new("blueprint-compiler")
        .arg("batch-compile")
        .arg(&ui_out)
        .arg(&blp_dir)
        .args(&blps)
        .status()
        .expect("blueprint-compiler is required: pacman -S blueprint-compiler");
    assert!(status.success(), "blueprint-compiler failed");

    println!("cargo:rerun-if-changed=src/ui");
    println!("cargo:rerun-if-changed=resources/gazania.gresource.xml");
    println!("cargo:rerun-if-changed=src/style.css");
    glib_build_tools::compile_resources(
        &[ui_out.to_str().unwrap(), "src", "resources"],
        "resources/gazania.gresource.xml",
        "gazania.gresource",
    );
}
