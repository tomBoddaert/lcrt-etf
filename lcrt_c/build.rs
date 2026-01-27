use std::{env, fs, path::Path};

fn main() {
    #![expect(clippy::unwrap_used)]
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    let c_config_path = Path::new(&crate_dir).join("cbindgen.toml");
    let cpp_config_patch_path = Path::new(&crate_dir).join("cppbindgen.toml.patch");
    let cpp_config_path = Path::new(&crate_dir).join("cppbindgen.toml");

    let c_config_str = fs::read_to_string(&c_config_path).expect("failed to read cbindgen.toml");
    let cpp_config_patch_str =
        fs::read_to_string(cpp_config_patch_path).expect("failed to read cppbindgen.toml.patch");

    let patch = diffy::Patch::from_str(&cpp_config_patch_str)
        .expect("failed to parse cppbindgen.toml.patch");

    let cpp_config_str = diffy::apply(&c_config_str, &patch)
        .expect("failed to apply cppbindgen.toml.patch to cbindgen.toml");

    let mut c_config = toml::from_str::<cbindgen::Config>(&c_config_str)
        .expect("failed to parse cbindgen.toml as toml");
    c_config.config_path = Some(c_config_path);
    let mut cpp_config = toml::from_str::<cbindgen::Config>(&cpp_config_str)
        .expect("failed to parse cppbindgen.toml as toml");
    cpp_config.config_path = Some(cpp_config_path);

    cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(c_config)
        .generate()
        .expect("failed to generate c bindings")
        .write_to_file("liblcrt.h");
    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(cpp_config)
        .generate()
        .expect("failed to generate c bindings")
        .write_to_file("liblcrt.hpp");
}
