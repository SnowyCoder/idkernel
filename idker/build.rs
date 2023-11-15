use std::env;
use std::path::Path;
use std::process::Command;


fn compile_trampoline(out_dir: &str) {
    println!("cargo:rerun-if-changed=src/asm/x86_64/trampoline.asm");

    let status = Command::new("nasm")
        .arg("-f")
        .arg("bin")
        .arg("-o")
        .arg(format!("{}/trampoline", out_dir))
        .arg("src/asm/x86_64/trampoline.asm")
        .status()
        .expect("Failed to run nasm");
    if !status.success() {
        panic!("nasm failed with exit status {}", status);
    }
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();

    let home_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rerun-if-changed=layout.ld");
    let linker_path = Path::new(&home_dir).join("layout.ld");
    let linker_path = linker_path.display();
    println!("cargo:rustc-link-arg=-T{linker_path}");

    compile_trampoline(&out_dir);
}
