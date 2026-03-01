use iced::task::{Sipper, sipper};
use image::imageops::resize;
use image::{ImageReader, RgbImage};
use rand::prelude::*;
use std::path::Path;

pub fn load_image(file_name: &str) -> Result<RgbImage, image::ImageError> {
    let path = Path::new("images").join(file_name);

    let img = ImageReader::open(path)?.decode()?;
    let rgb_img = img.to_rgb8();
    let rgb_img = resize(&rgb_img, 200, 200, image::imageops::FilterType::Gaussian);

    println!(
        "Image loaded, {}. Dimensions: {:?}",
        file_name,
        rgb_img.dimensions()
    );

    Ok(rgb_img)
}

pub fn euclidean_color_distance(c1: [u8; 3], c2: [u8; 3]) -> f64 {
    let r_diff = c1[0] as f64 - c2[0] as f64;
    let g_diff = c1[1] as f64 - c2[1] as f64;
    let b_diff = c1[2] as f64 - c2[2] as f64;
    let distance = (r_diff * r_diff + g_diff * g_diff + b_diff * b_diff).sqrt();
    distance / 441.67295593006372
}

pub fn displacement_cost(p1: [u32; 2], p2: [u32; 2], normalization: f64) -> f64 {
    let x_diff = (p1[0] as f64 - p2[0] as f64).abs();
    let y_diff = (p1[1] as f64 - p2[1] as f64).abs();
    if normalization == 0.0 {
        return 0.0;
    }
    (x_diff * x_diff + y_diff * y_diff).sqrt() / normalization
}

pub fn morph(
    s_img: &RgbImage,
    t_img: &RgbImage,
    proximity_weight: f64,
    num_iterations: u32,
    search_radius: u32,
) -> (RgbImage, Vec<Vec<u32>>) {
    let mut rng = rand::thread_rng();

    let width = s_img.width().min(t_img.width());
    let height = s_img.height().min(t_img.height());
    let normalization = ((width * width + height * height) as f64).sqrt();

    let mut output_img = RgbImage::new(width, height);

    let mut current_img: Vec<Vec<u32>> = Vec::new();

    // Current image stores the pixel coordinates for corresponding pixels in the output image.
    // Eg: The current_image[0][0] = 1 means that the pixel at (0, 0) in the output image
    // corresponds to the pixel at (1, 0) in the input images.
    // This can be used to get the pixel data to display the output
    for i in 0..height {
        let mut row: Vec<u32> = Vec::new();
        for j in 0..width {
            row.push(j + i * width);
        }
        current_img.push(row);
    }

    let mut swaps = 0;
    let start_temp = 0.01;
    let end_temp = 0.00001;
    let log_temp_decay = ((end_temp / start_temp) as f64).ln();

    for i in 0..num_iterations {
        if i % 100_000 == 0 {
            println!("Iteration: {}, Swaps: {}", i, swaps);
        }

        let progress = i as f64 / num_iterations as f64;

        // Decay search radius
        let current_radius = (search_radius as f64 * (1.0 - progress)).max(1.0) as u32;

        // Decay temperature
        let current_temp = start_temp * (log_temp_decay * progress).exp();

        let x: u32 = rng.gen_range(0..width);
        let y: u32 = rng.gen_range(0..height);

        // Potential Swap
        let x1 =
            x + (((rng.gen_range(0.0..=1.0) as f64) * 2.0 - 1.0) * current_radius as f64) as u32;
        let x1 = x1.clamp(0, width - 1);

        let y1 = y + ((rng.gen_range(0.0..=1.0) * 2.0 - 1.0) * current_radius as f64) as u32;
        let y1 = y1.clamp(0, height - 1);

        // Calculate the cost of keeping the current pixel (just color distance)
        let s_current_pixel_coords = current_img[y as usize][x as usize];
        let s_current_pixel = s_img.get_pixel(
            s_current_pixel_coords % width,
            s_current_pixel_coords / width,
        );

        let s_swap_pixel_coords = current_img[y1 as usize][x1 as usize];
        let s_swap_pixel =
            s_img.get_pixel(s_swap_pixel_coords % width, s_swap_pixel_coords / width);

        let t_current_pixel = t_img.get_pixel(x, y);
        let t_swap_pixel = t_img.get_pixel(x1, y1);

        // Calculate costs for the current state
        // This includes the color distance of the current source pixel and the current target pixel
        // and the displacement cost of the current source pixel from its original position
        let current_color_cost = euclidean_color_distance(s_current_pixel.0, t_current_pixel.0)
            + euclidean_color_distance(s_swap_pixel.0, t_swap_pixel.0);

        // These are the original coordinates of the pixels currently in (x, y)
        let s_curr_orig_x = s_current_pixel_coords % width;
        let s_curr_orig_y = s_current_pixel_coords / width;
        let s_swap_orig_x = s_swap_pixel_coords % width;
        let s_swap_orig_y = s_swap_pixel_coords / width;

        let current_displacement =
            displacement_cost([x, y], [s_curr_orig_x, s_curr_orig_y], normalization)
                + displacement_cost([x1, y1], [s_swap_orig_x, s_swap_orig_y], normalization);

        let current_total_cost = current_color_cost + proximity_weight * current_displacement;

        // Calculate costs for the swapped state
        let swap_color_cost = euclidean_color_distance(s_current_pixel.0, t_swap_pixel.0)
            + euclidean_color_distance(s_swap_pixel.0, t_current_pixel.0);

        // If we swap:
        // Pixel at (x,y) will be s_swap_pixel (origin s_swap_orig)
        // Pixel at (x1,y1) will be s_current_pixel (origin s_curr_orig)
        let swap_displacement =
            displacement_cost([x, y], [s_swap_orig_x, s_swap_orig_y], normalization)
                + displacement_cost([x1, y1], [s_curr_orig_x, s_curr_orig_y], normalization);

        let swap_total_cost = swap_color_cost + proximity_weight * swap_displacement;

        let delta = swap_total_cost - current_total_cost;

        if delta < 0.0 || rng.gen_range(0.0..=1.0) < (-delta / current_temp).exp() {
            swaps += 1;
            current_img[y as usize][x as usize] = s_swap_pixel_coords;
            current_img[y1 as usize][x1 as usize] = s_current_pixel_coords;
        }
    }

    println!("Swaps: {}", swaps);
    // For the output image
    for y in 0..height {
        for x in 0..width {
            let s_pixel_coords = current_img[y as usize][x as usize];
            let s_pixel = s_img.get_pixel(s_pixel_coords % width, s_pixel_coords / width);
            output_img.put_pixel(x, y, *s_pixel);
        }
    }

    (output_img, current_img)
}

pub fn morph_test(
    s_img: RgbImage,
    t_img: RgbImage,
    proximity_weight: f64,
    num_iterations: u32,
    search_radius: u32,
) -> impl Sipper<(RgbImage, Vec<Vec<u32>>), f64> {
    sipper(async move |mut sender| {
        // let mut rng = rand::thread_rng();
        // Using StdRng with from_entropy as thread_rng is not Send and cannot be used in async tasks
        let mut rng = rand::rngs::StdRng::from_entropy();

        let width = s_img.width().min(t_img.width());
        let height = s_img.height().min(t_img.height());
        let normalization = ((width * width + height * height) as f64).sqrt();

        let mut output_img = RgbImage::new(width, height);

        let mut current_img: Vec<Vec<u32>> = Vec::new();

        // Current image stores the pixel coordinates for corresponding pixels in the output image.
        // Eg: The current_image[0][0] = 1 means that the pixel at (0, 0) in the output image
        // corresponds to the pixel at (1, 0) in the input images.
        // This can be used to get the pixel data to display the output
        for i in 0..height {
            let mut row: Vec<u32> = Vec::new();
            for j in 0..width {
                row.push(j + i * width);
            }
            current_img.push(row);
        }

        let mut swaps = 0;
        let start_temp = 0.01;
        let end_temp = 0.00001;
        let log_temp_decay = ((end_temp / start_temp) as f64).ln();

        for i in 0..num_iterations {
            if i % 100_000 == 0 {
                println!("Iteration: {}, Swaps: {}", i, swaps);
                let _ = sender.send(i as f64 / num_iterations as f64).await;
            }

            let progress = i as f64 / num_iterations as f64;

            // Decay search radius
            let current_radius = (search_radius as f64 * (1.0 - progress)).max(1.0) as u32;

            // Decay temperature
            let current_temp = start_temp * (log_temp_decay * progress).exp();

            let x: u32 = rng.gen_range(0..width);
            let y: u32 = rng.gen_range(0..height);

            // Potential Swap
            let x1 = x + ((rng.gen_range(0.0..=1.0) * 2.0 - 1.0) * current_radius as f64) as u32;
            let x1 = x1.clamp(0, width - 1);

            let y1 = y + ((rng.gen_range(0.0..=1.0) * 2.0 - 1.0) * current_radius as f64) as u32;
            let y1 = y1.clamp(0, height - 1);

            // Calculate the cost of keeping the current pixel (just color distance)
            let s_current_pixel_coords = current_img[y as usize][x as usize];
            let s_current_pixel = s_img.get_pixel(
                s_current_pixel_coords % width,
                s_current_pixel_coords / width,
            );

            let s_swap_pixel_coords = current_img[y1 as usize][x1 as usize];
            let s_swap_pixel =
                s_img.get_pixel(s_swap_pixel_coords % width, s_swap_pixel_coords / width);

            let t_current_pixel = t_img.get_pixel(x, y);
            let t_swap_pixel = t_img.get_pixel(x1, y1);

            // Calculate costs for the current state
            // This includes the color distance of the current source pixel and the current target pixel
            // and the displacement cost of the current source pixel from its original position
            let current_color_cost = euclidean_color_distance(s_current_pixel.0, t_current_pixel.0)
                + euclidean_color_distance(s_swap_pixel.0, t_swap_pixel.0);

            // These are the original coordinates of the pixels currently in (x, y)
            let s_curr_orig_x = s_current_pixel_coords % width;
            let s_curr_orig_y = s_current_pixel_coords / width;
            let s_swap_orig_x = s_swap_pixel_coords % width;
            let s_swap_orig_y = s_swap_pixel_coords / width;

            let current_displacement =
                displacement_cost([x, y], [s_curr_orig_x, s_curr_orig_y], normalization)
                    + displacement_cost([x1, y1], [s_swap_orig_x, s_swap_orig_y], normalization);

            let current_total_cost = current_color_cost + proximity_weight * current_displacement;

            // Calculate costs for the swapped state
            let swap_color_cost = euclidean_color_distance(s_current_pixel.0, t_swap_pixel.0)
                + euclidean_color_distance(s_swap_pixel.0, t_current_pixel.0);

            // If we swap:
            // Pixel at (x,y) will be s_swap_pixel (origin s_swap_orig)
            // Pixel at (x1,y1) will be s_current_pixel (origin s_curr_orig)
            let swap_displacement =
                displacement_cost([x, y], [s_swap_orig_x, s_swap_orig_y], normalization)
                    + displacement_cost([x1, y1], [s_curr_orig_x, s_curr_orig_y], normalization);

            let swap_total_cost = swap_color_cost + proximity_weight * swap_displacement;

            let delta = swap_total_cost - current_total_cost;

            if delta < 0.0 || rng.gen_range(0.0..=1.0) < (-delta / current_temp).exp() {
                swaps += 1;
                current_img[y as usize][x as usize] = s_swap_pixel_coords;
                current_img[y1 as usize][x1 as usize] = s_current_pixel_coords;
            }
        }

        println!("Swaps: {}", swaps);
        // For the output image
        for y in 0..height {
            for x in 0..width {
                let s_pixel_coords = current_img[y as usize][x as usize];
                let s_pixel = s_img.get_pixel(s_pixel_coords % width, s_pixel_coords / width);
                output_img.put_pixel(x, y, *s_pixel);
            }
        }

        (output_img, current_img)
    })
}

pub fn create_displacement_map(current_img: &Vec<Vec<u32>>, width: u32) -> Vec<Vec<(f64, f64)>> {
    let height = current_img.len() as u32;
    let mut displacement_map = vec![vec![(0f64, 0f64); width as usize]; height as usize];

    for y in 0..height {
        for x in 0..width {
            let src_idx = current_img[y as usize][x as usize];
            let src_x = src_idx % width;
            let src_y = src_idx / width;

            let dx = x as f64 - src_x as f64;
            let dy = y as f64 - src_y as f64;

            displacement_map[src_y as usize][src_x as usize] = (dx, dy);
        }
    }

    displacement_map
}

// pub fn create_morph_frames(
//     s_img: &RgbImage,
//     current_img: &Vec<Vec<u32>>,
//     num_frames: u32,
//     output_dir: &Path,
// ) -> Result<(), image::ImageError> {
//     if current_img.is_empty() {
//         return Ok(());
//     }

//     let height = current_img.len() as u32;
//     let width = current_img[0].len() as u32;

//     // Calculate displacement map: source_pos -> target_pos
//     // dest_map[y][x] = (target_x, target_y) for the pixel originally at (x, y)
//     let mut displacement_map = vec![vec![(0f64, 0f64); width as usize]; height as usize];

//     for y in 0..height {
//         for x in 0..width {
//             // The pixel at target (x, y) came from source index `src_idx`
//             let src_idx = current_img[y as usize][x as usize];
//             let src_x = src_idx % width;
//             let src_y = src_idx / width;

//             let dx = x as f64 - src_x as f64;
//             let dy = y as f64 - src_y as f64;

//             displacement_map[src_y as usize][src_x as usize] = (dx, dy);
//         }
//     }

//     if !output_dir.exists() {
//         std::fs::create_dir_all(output_dir)?;
//     }

//     for f in 0..num_frames {
//         let t = f as f64 / (num_frames - 1) as f64;
//         let mut frame = RgbImage::new(width, height);

//         for y in 0..height {
//             for x in 0..width {
//                 let (dx, dy) = displacement_map[y as usize][x as usize];

//                 let curr_x = x as f64 + dx * t;
//                 let curr_y = y as f64 + dy * t;

//                 let ix = curr_x.round() as i32;
//                 let iy = curr_y.round() as i32;

//                 if ix >= 0 && ix < width as i32 && iy >= 0 && iy < height as i32 {
//                     frame.put_pixel(ix as u32, iy as u32, *s_img.get_pixel(x, y));
//                 }
//             }
//         }

//         let file_name = format!("frame_{:05}.png", f);
//         let path = output_dir.join(file_name);
//         frame.save(path)?;

//         if f % 10 == 0 {
//             println!("Saved frame {}/{}", f, num_frames);
//         }
//     }

//     Ok(())
// }
