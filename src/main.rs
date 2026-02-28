use std::error::Error;
use std::path::Path;

use image_morph_rust::{create_morph_frames, load_image, morph};

fn main() -> Result<(), Box<dyn Error>> {
    let s_img = load_image("cat.jpg")?;
    let t_img = load_image("obama.jpg")?;

    let (output, current_img) = morph(&s_img, &t_img, 0.1, 2_500_000, 30);

    output.save("./output/morphed.png")?;

    create_morph_frames(&s_img, &current_img, 30, Path::new("output/frames"))?;

    Ok(())
}
