use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Convert PNG image to 2-color format at build time
fn convert_image_to_binary(
    input_path: &str,
    output_path: &str,
    target_width: u32,
    target_height: u32,
    threshold: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed={}", input_path);

    // Check if input file exists
    if !Path::new(input_path).exists() {
        println!(
            "cargo:warning=Image file '{}' not found, skipping conversion",
            input_path
        );
        // Create empty file so build doesn't fail
        let mut file = File::create(output_path)?;
        file.write_all(&[])?;
        return Ok(());
    }

    println!("cargo:warning=Converting image: {}", input_path);

    // Load the image
    let img = image::open(input_path)?;
    println!(
        "cargo:warning=Original image size: {}x{}",
        img.width(),
        img.height()
    );

    // Calculate aspect-ratio-preserving dimensions
    let orig_width = img.width();
    let orig_height = img.height();
    let orig_ratio = orig_width as f32 / orig_height as f32;
    let target_ratio = target_width as f32 / target_height as f32;

    let (new_width, new_height) = if orig_ratio > target_ratio {
        // Image is wider than target - fit to width
        (target_width, (target_width as f32 / orig_ratio) as u32)
    } else {
        // Image is taller than target - fit to height
        ((target_height as f32 * orig_ratio) as u32, target_height)
    };

    println!(
        "cargo:warning=Resizing to: {}x{} (preserving aspect ratio)",
        new_width, new_height
    );

    // Resize the image maintaining aspect ratio
    let resized = img.resize(new_width, new_height, image::imageops::FilterType::Lanczos3);

    // Convert to grayscale first
    let gray_resized = resized.to_luma8();

    // Rotate 90 degrees counter-clockwise to match physical display orientation
    // The display RAM is 128×296 (portrait), but we want landscape (296×128 visual)
    // So we rotate the image before converting to match how it will appear
    println!("cargo:warning=Rotating image 90° counter-clockwise for landscape display");
    let gray = image::imageops::rotate270(&gray_resized);

    // After rotation, dimensions are swapped
    let rotated_width = gray.width();
    let rotated_height = gray.height();
    println!(
        "cargo:warning=After rotation: {}x{}",
        rotated_width, rotated_height
    );

    // Convert to 2-color (black and white) using threshold
    // Pack into bytes where each bit represents a pixel
    let bytes_per_row = target_width.div_ceil(8);
    let total_bytes = (bytes_per_row * target_height) as usize;
    let mut buffer = vec![0u8; total_bytes];

    println!(
        "cargo:warning=Converting to 2-color format (threshold: {})",
        threshold
    );

    // Calculate centering offsets using rotated dimensions
    let offset_x = (target_width - rotated_width) / 2;
    let offset_y = (target_height - rotated_height) / 2;

    println!(
        "cargo:warning=Centering image with offset: ({}, {})",
        offset_x, offset_y
    );

    // Fill buffer with white background, then draw centered image
    for y in 0..target_height {
        for x in 0..target_width {
            // Check if we're within the image bounds
            let img_x = x.checked_sub(offset_x);
            let img_y = y.checked_sub(offset_y);

            let brightness = if let (Some(ix), Some(iy)) = (img_x, img_y) {
                if ix < rotated_width && iy < rotated_height {
                    // Within image bounds - get pixel
                    let pixel = gray.get_pixel(ix, iy);
                    pixel[0]
                } else {
                    // Outside image bounds - white
                    255
                }
            } else {
                // Outside image bounds - white
                255
            };

            // INVERTED: If pixel is LIGHTER than threshold, set bit to 1 (black)
            // This inverts the image: white background becomes black, black content becomes white
            // For e-paper: bit 1 = black, bit 0 = white
            if brightness >= threshold {
                let byte_index = (y * bytes_per_row + x / 8) as usize;
                let bit_index = 7 - (x % 8);
                buffer[byte_index] |= 1 << bit_index;
            }
        }
    }

    println!(
        "cargo:warning=Image conversion complete. Buffer size: {} bytes",
        buffer.len()
    );

    // Write binary data to file
    let mut file = File::create(output_path)?;
    file.write_all(&buffer)?;

    println!("cargo:warning=Binary image saved to: {}", output_path);
    Ok(())
}

fn main() {
    embuild::espidf::sysenv::output();

    // Get output directory
    let out_dir = env::var("OUT_DIR").unwrap();

    // Convert logo.png to binary format at build time
    // IMPORTANT: Display physical RAM is 128 pixels wide × 296 pixels tall
    // For landscape mode (296×128 visual), we need to rotate the image 90° clockwise
    // So we convert as if the display is in portrait mode (128×296)
    let logo_output = format!("{}/logo.bin", out_dir);

    if let Err(e) = convert_image_to_binary(
        "logo.png",
        &logo_output,
        128, // physical width (pixels horizontally in RAM)
        296, // physical height (pixels vertically in RAM)
        128, // threshold (0-255, 128 = middle gray)
    ) {
        println!("cargo:warning=Failed to convert logo.png: {}", e);
    }

    println!("cargo:rerun-if-changed=logo.png");
}
