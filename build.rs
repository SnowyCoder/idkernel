use std::env;

fn compile_trampoline(out_dir: &str) {
    use std::process::Command;

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
    compile_trampoline(&out_dir);
}
