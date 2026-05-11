use std::path::PathBuf;

const SDK_PATH_ENV_VAR: &str = "ZWO_ASI_SDK";

#[derive(Debug, Clone, Copy)]
enum TargetOs {
    Macos,
    Linux,
}
impl TargetOs {
    pub fn get() -> Self {
        let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
        match &*target_os {
            "macos" => Self::Macos,
            "linux" => Self::Linux,
            _ => panic!("zwo-asi-sys only supports linux and macos"),
        }
    }
}
#[derive(Debug, Clone, Copy)]
enum TargetArch {
    X86,
    X64,
    Arm,
    Arm64,
}
impl TargetArch {
    fn get() -> Self {
        let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
        match &*arch {
            "x86" => Self::X86,
            "x86_64" => Self::X64,
            "arm" => Self::Arm,
            "aarch64" => Self::Arm64,
            _ => panic!("Unsupported architecture {arch}"),
        }
    }
    /// Gets the directory name within the `lib` folder
    fn lib_dir_name(&self) -> &'static str {
        match (self, TargetOs::get()) {
            (Self::X86, TargetOs::Linux) => "x86",
            (Self::X64, TargetOs::Linux) => "x64",
            (Self::Arm, TargetOs::Macos) => unreachable!(),
            (Self::Arm64, TargetOs::Macos) => "mac_arm64",
            // I'm not so sure about the linux ARM ones
            (Self::Arm, TargetOs::Linux) => "armv7",
            (Self::Arm64, TargetOs::Linux) => "armv8",
            (arch, os) => panic!("Unsupported combination of arch and target os: {arch:?} {os:?}"),
        }
    }
}
enum AsiSdkInstallation {
    AtPath(PathBuf),
    System,
}

impl AsiSdkInstallation {
    pub fn detect() -> Self {
        let Ok(sdk_path) = std::env::var(SDK_PATH_ENV_VAR) else {
            return Self::System;
        };
        Self::AtPath(sdk_path.into())
    }

    pub fn link_libs(&self) {
        match self {
            Self::AtPath(path) => {
                let lib_folder = path
                    .join("lib")
                    .join(TargetArch::get().lib_dir_name())
                    .canonicalize()
                    .expect("ZWO_ASI_SDK does not have a valid lib folder");
                println!("cargo:rustc-link-search={}", lib_folder.to_str().unwrap())
            }
            Self::System => {
                // currently do nothing
            }
        }
        match TargetOs::get() {
            TargetOs::Linux => {
                println!("cargo:rustc-link-lib=ASICamera2");
                println!("cargo:rustc-link-lib=stdc++");
            }
            TargetOs::Macos => {
                println!("cargo:rustc-link-lib=static=ASICamera2");
                println!("cargo:rustc-link-lib=c++");
            }
        }

        println!("cargo:rustc-link-lib=pthread");
        println!("cargo:rustc-link-lib=m");
        // todo: maybe use pkg-config
        println!("cargo:rustc-link-lib=usb-1.0");
    }
    pub fn bindgen(&self) {
        let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let wrapper_include_path = manifest_dir.join("include");
        let wrapper_h = wrapper_include_path.join("wrapper.h");

        let mut builder = bindgen::Builder::default()
            // The input header we would like to generate
            // bindings for.
            .header(wrapper_h.to_str().unwrap())
            // Tell cargo to invalidate the built crate whenever any of the
            // included header files changed.
            .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
            // generate doc comments in the system headers
            .generate_comments(true)
            .merge_extern_blocks(true)
            .clang_arg("-fretain-comments-from-system-headers")
            .clang_arg("-fparse-all-comments");

        // If we have a special SDK path, then we want to include from the SDK path
        if let Self::AtPath(path) = self {
            let include_path = path
                .join("include")
                .canonicalize()
                .expect("ZWO_ASI_SDK does not have a valid include folder");
            builder = builder.clang_args(["-I", include_path.to_str().unwrap()])
        }
        let bindings = builder.generate().expect("Failed to generate bindings");

        //  Write the bindings to the $OUT_DIR/bindings.rs file.
        let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap());

        let out_path = out_path.join("bindings.rs");
        bindings
            .write_to_file(out_path)
            .expect("Couldn't write bindings!");
    }
}
fn main() {
    let installation = AsiSdkInstallation::detect();
    installation.link_libs();
    installation.bindgen();
}
