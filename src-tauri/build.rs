use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

// TDLib loads with its own directory on the DLL search path. Bundle Microsoft's
// app-local redistributable files so selecting a proxy works on a fresh PC.
fn stage_tdlib_runtime() {
    const FILES: [&str; 3] = ["msvcp140.dll", "vcruntime140.dll", "vcruntime140_1.dll"];
    let crt_in = |root: &Path| root.join("x64/Microsoft.VC143.CRT");
    let from_prompt = env::var_os("VCToolsRedistDir").map(|dir| crt_in(Path::new(&dir)));
    let from_visual_studio = || {
        let vswhere = PathBuf::from(env::var_os("ProgramFiles(x86)")?)
            .join("Microsoft Visual Studio/Installer/vswhere.exe");
        let result = Command::new(vswhere)
            .args([
                "-latest",
                "-products",
                "*",
                "-requires",
                "Microsoft.VisualStudio.Component.VC.Redist.14.Latest",
                "-property",
                "installationPath",
            ])
            .output()
            .ok()?;
        let installation = String::from_utf8(result.stdout).ok()?;
        let root = PathBuf::from(installation.trim()).join("VC/Redist/MSVC");
        fs::read_dir(root)
            .ok()?
            .filter_map(Result::ok)
            .map(|entry| crt_in(&entry.path()))
            .filter(|dir| FILES.iter().all(|name| dir.join(name).is_file()))
            .max()
    };
    let usable = |dir: &PathBuf| FILES.iter().all(|name| dir.join(name).is_file());
    let crt = from_prompt
        .filter(usable)
        .or_else(|| from_visual_studio().filter(usable))
        .expect("Build requires the Visual Studio 2022 x64 C++ Redistributable component (VC143).");
    let staging = Path::new("installer/vc-runtime");
    fs::create_dir_all(staging).expect("create app-local runtime staging directory");
    for name in FILES {
        let original = crt.join(name);
        let target = staging.join(name);
        println!("cargo:rerun-if-changed={}", original.display());
        if fs::read(&original).expect("read Microsoft C++ runtime")
            != fs::read(&target).unwrap_or_default()
        {
            fs::copy(original, target).expect("stage Microsoft C++ runtime");
        }
    }
}

fn main() {
    println!("cargo:rerun-if-env-changed=VCToolsRedistDir");
    if env::var("TARGET").is_ok_and(|target| target == "x86_64-pc-windows-msvc") {
        stage_tdlib_runtime();
    }
    tauri_build::build()
}
