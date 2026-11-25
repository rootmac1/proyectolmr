use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/brand/lmr.ico");
    println!("cargo:rerun-if-changed=build.rs");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let icon_path = manifest_dir.join("assets").join("brand").join("lmr.ico");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    let rc_path = out_dir.join("lmr-icon.rc");
    let res_path = out_dir.join("lmr-icon.res");
    let icon_for_rc = icon_path.to_string_lossy().replace('\\', "\\\\");

    if let Err(error) = fs::write(&rc_path, format!("IDI_ICON1 ICON \"{}\"\n", icon_for_rc)) {
        println!("cargo:warning=No se pudo preparar el icono de LMR: {error}");
        return;
    }

    match Command::new("llvm-rc")
        .arg("/FO")
        .arg(&res_path)
        .arg(&rc_path)
        .status()
    {
        Ok(status) if status.success() => {
            println!("cargo:rustc-link-arg-bin=lmr={}", res_path.display());
        }
        Ok(status) => println!(
            "cargo:warning=llvm-rc no pudo compilar el icono de LMR ({}); instala LLVM para incrustarlo",
            status
        ),
        Err(error) => println!(
            "cargo:warning=No se encontró llvm-rc ({error}); instala LLVM para incrustar el icono de LMR"
        ),
    }
}
