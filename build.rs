use std::process::Command;

fn main() -> std::io::Result<()> {
    println!("cargo::rerun-if-changed=assets/scenes");
    Command::new("cp")
        .arg("-r")
        .arg("assets/scenes")
        .arg("target/scenes")
        .output()
        .map(|_| ())
}
