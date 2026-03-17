fn main() {
    #[cfg(target_os = "windows")]
    {
        let png_path = "assets/logo.png";
        let ico_path = "assets/icon.ico";

        // Generate .ico from .png if needed
        let needs_rebuild = !std::path::Path::new(ico_path).exists()
            || std::fs::metadata(png_path)
                .and_then(|png_meta| {
                    std::fs::metadata(ico_path).map(|ico_meta| {
                        png_meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                            > ico_meta.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                    })
                })
                .unwrap_or(true);

        if needs_rebuild {
            if let Err(e) = generate_ico(png_path, ico_path) {
                eprintln!("Warning: Failed to generate icon: {e}");
            }
        }

        // Write the .rc file next to the ico
        let rc_path = "assets/app.rc";
        let abs_ico = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join(ico_path);
        let rc_content = format!(
            "1 ICON \"{}\"",
            abs_ico.to_str().unwrap().replace('\\', "\\\\")
        );
        std::fs::write(rc_path, &rc_content).expect("Failed to write .rc file");

        // Compile and link the resource into the binary (not the lib)
        let _ = embed_resource::compile_for(
            rc_path,
            ["polterdesk"],
            embed_resource::NONE,
        );

        println!("cargo::rerun-if-changed={png_path}");
        println!("cargo::rerun-if-changed={rc_path}");
    }
}

#[cfg(target_os = "windows")]
fn generate_ico(png_path: &str, ico_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    use image::imageops::FilterType;
    use std::io::BufWriter;

    let img = image::open(png_path)?;
    let sizes: &[u32] = &[16, 32, 48, 256];

    let file = std::fs::File::create(ico_path)?;
    let mut writer = BufWriter::new(file);

    let mut encoder = ico::IconDir::new(ico::ResourceType::Icon);
    for &size in sizes {
        let resized = img.resize_exact(size, size, FilterType::Lanczos3);
        let rgba = resized.to_rgba8();
        let icon_image = ico::IconImage::from_rgba_data(size, size, rgba.as_raw().to_vec());
        encoder.add_entry(ico::IconDirEntry::encode(&icon_image)?);
    }
    encoder.write(&mut writer)?;

    Ok(())
}
