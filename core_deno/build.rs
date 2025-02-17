use std::env;
use std::path::Path;
use std::process::{
    Command,
    Stdio,
};

fn main() {
    // Watch for changes in the deno api
    println!("cargo:rerun-if-changed=./core_deno/src/graviton.ts");

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let out_file_dir = Path::new(&out_dir).join("graviton.js");

    // Transpile the deno api
    let res = Command::new("deno")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .arg("bundle")
        .arg("--unstable")
        .arg("./src/graviton.ts")
        .arg(out_file_dir.to_str().unwrap())
        .output()
        .expect("Could not compile graviton.ts");

    println!("{}", std::str::from_utf8(&res.stderr).unwrap());

    assert!(res.status.success());
}
