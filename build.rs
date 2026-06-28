use std::path::Path;

fn main() {
    let icon_path = Path::new("icon.png");
    let ico_path = Path::new("favicon.ico");

    if !ico_path.exists() && icon_path.exists() {
        if let Ok(img) = image::open(icon_path) {
            let img = img.resize(256, 256, image::imageops::FilterType::Lanczos3);
            let rgba = img.to_rgba8();
            let _ = rgba.save_with_format(ico_path, image::ImageFormat::Ico);
        }
    }

    #[cfg(target_os = "windows")]
    {
        embed_resource::compile("resource.rc", embed_resource::NONE);
    }
}
