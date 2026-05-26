use std::path::PathBuf;

const SDK_PATH_ENV_VAR: &str = "PLAYER_ONE_CAMERA_SDK";

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
            _ => {
                panic!("player-one-camera-sys only supports linux and macos (sorry windows users)")
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum TargetArch {
    X86,
    X64,
    Arm32,
    Arm64,
}
impl TargetArch {
    fn get() -> Self {
        let arch = std::env::var("CARGO_CFG_TARGET_ARCH")
            .unwrap()
            .to_ascii_lowercase();
        match &*arch {
            "x86" => Self::X86,
            "x86_64" => Self::X64,
            // I'm not quite sure if this is right,
            // needs more testing. I don't have a linux arm
            // device
            "armv7" | "arm" => Self::Arm32,
            "aarch64" => Self::Arm64,
            _ => panic!("Unsupported architecture {arch}"),
        }
    }
    /// Gets the directory name within the `lib` folder
    fn lib_dir_name(&self) -> &'static str {
        match (self, TargetOs::get()) {
            (Self::X86, TargetOs::Linux) => "x86",
            (Self::X64, TargetOs::Linux) => "x64",
            // The mac people have it easy. They just have everything in
            // a single folder
            (Self::Arm64, TargetOs::Macos) => ".",
            // I'm not so sure about the linux ARM ones
            (Self::Arm32, TargetOs::Linux) => "arm32",
            (Self::Arm64, TargetOs::Linux) => "arm64",
            (arch, os) => panic!("Unsupported combination of arch and target os: {arch:?} {os:?}"),
        }
    }
}
enum PlayerOneSdkInstallation {
    AtPath(PathBuf),
    System,
}

impl PlayerOneSdkInstallation {
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
                    .expect("PLAYER_ONE_CAMERA_SDK does not have a valid lib folder");
                println!("cargo:rustc-link-search={}", lib_folder.to_str().unwrap())
            }
            Self::System => {
                // currently do nothing
            }
        }
        match TargetOs::get() {
            TargetOs::Linux => {
                println!("cargo:rustc-link-lib=PlayerOneCamera");
                println!("cargo:rustc-link-lib=stdc++");
            }
            TargetOs::Macos => {
                println!("cargo:rustc-link-lib=static=PlayerOneCamera");
                println!("cargo:rustc-link-lib=c++");
            }
        }

        println!("cargo:rustc-link-lib=pthread");
        println!("cargo:rustc-link-lib=m");
        // todo: maybe use pkg-config
        println!("cargo:rustc-link-lib=usb-1.0");
    }
}

fn main() {
    PlayerOneSdkInstallation::detect().link_libs();
}
