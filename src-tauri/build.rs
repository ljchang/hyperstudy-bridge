fn main() {
    tauri_build::build();

    // EyeLink SDK linking (only when eyelink feature is enabled)
    #[cfg(feature = "eyelink")]
    {
        // Platform-specific library search paths for the EyeLink Developers Kit
        #[cfg(target_os = "macos")]
        {
            println!("cargo:rustc-link-search=framework=/Library/Frameworks");
            println!("cargo:rustc-link-lib=framework=eyelink_core");
        }

        #[cfg(target_os = "linux")]
        {
            println!("cargo:rustc-link-search=native=/usr/lib");
            println!("cargo:rustc-link-lib=dylib=eyelink_core");
        }

        #[cfg(target_os = "windows")]
        {
            println!(
                "cargo:rustc-link-search=native=C:/Program Files/SR Research/Eyelink/Libs/x64"
            );
            println!("cargo:rustc-link-lib=dylib=eyelink_core");
        }
    }
}
