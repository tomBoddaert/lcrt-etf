use std::{env, fs, path::Path};

fn main() {
    let crate_dir_var =
        env::var("CARGO_MANIFEST_DIR").expect("expected build.rs to be run from cargo");
    let crate_dir = Path::new(&crate_dir_var);
    let include_dir = crate_dir.join("include");

    let c_config_path = crate_dir.join("cbindgen.toml");
    let cpp_config_patch_path = crate_dir.join("cppbindgen.toml.patch");
    let cpp_config_path = crate_dir.join("cppbindgen.toml");

    println!(
        "cargo::rerun-if-changed={}",
        c_config_path
            .file_name()
            .expect("failed to get C config file name")
            .display()
    );
    println!(
        "cargo::rerun-if-changed={}",
        cpp_config_patch_path
            .file_name()
            .expect("failed to get C++ config patch file name")
            .display()
    );

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

    let builder = cbindgen::Builder::new().with_crate(crate_dir);
    let write = |config, name| {
        builder.clone().with_config(config).generate().map_or_else(
            |error| match error {
                cbindgen::Error::ParseSyntaxError { .. } => {
                    println!("cargo::warning=cbindgen failed to parse the code");
                }
                error => println!("cargo::error={error}"),
            },
            |bindings| {
                bindings.write_to_file(include_dir.join(name));
            },
        );
    };

    write(c_config, "liblcrt.h");
    write(cpp_config, "liblcrt.hpp");
}
