use std::{env, fs, path::Path};

use uniprops_gen::{generate_categories, generate_digits};

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("generated.rs");

    let code = generate_categories(|_| true).unwrap() + &generate_digits(|_| true).unwrap();

    fs::write(&dest_path, code).unwrap();

    // Важно: скажите Cargo пересобирать, только если изменился build.rs
    println!("cargo:rerun-if-changed=build.rs");
}
