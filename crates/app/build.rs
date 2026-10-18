//! Gives makine.exe its icon on Windows.

fn main() {
    println!("cargo:rerun-if-changed=makine.rc");
    println!("cargo:rerun-if-changed=../../assets/makine.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("makine.rc", embed_resource::NONE)
            .manifest_optional()
            .unwrap();
    }
}
