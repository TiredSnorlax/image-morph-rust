use iced::Length::Fill;
use iced::widget::{button, column, image as iced_image, row, slider, text};
use iced::{Element, Task};
use image::imageops::resize;
use image::{ImageReader, RgbImage};
use image_morph_rust::{create_displacement_map, morph_test};
use rand::Rng;

use std::path::PathBuf;

const MAX_DIMENSION: u32 = 200;

pub fn main() -> iced::Result {
    iced::application(ImageMorph::new, ImageMorph::update, ImageMorph::view)
        .title("Image Morph")
        .run()
}

struct ImageMorph {
    source_path: Option<PathBuf>,
    target_path: Option<PathBuf>,
    status: String,
    is_morphing: bool,
    delta: f32,
    source_image: Option<RgbImage>,
    displacement_map: Option<Vec<Vec<(f64, f64)>>>,
    current_image_handle: Option<iced_image::Handle>,
}

#[derive(Debug, Clone)]
enum Message {
    SelectSource,
    SelectTarget,
    SourceSelected(Option<PathBuf>),
    TargetSelected(Option<PathBuf>),
    StartMorph,
    Morphing(f64),
    MorphFinished((RgbImage, Vec<Vec<u32>>)),
    CreateDisplacementMapFinished,
    DeltaChanged(f32),
}

impl Default for ImageMorph {
    fn default() -> Self {
        Self {
            source_path: None,
            target_path: None,
            status: "Ready".to_string(),
            is_morphing: false,
            delta: 0.0,
            source_image: None,
            displacement_map: None,
            current_image_handle: None,
        }
    }
}

impl ImageMorph {
    fn new() -> (Self, Task<Message>) {
        (Self::default(), Task::none())
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectSource => Task::perform(pick_file(), Message::SourceSelected),
            Message::SelectTarget => Task::perform(pick_file(), Message::TargetSelected),
            Message::SourceSelected(path) => {
                if let Some(p) = path {
                    self.source_path = Some(p);
                } else {
                    self.status = "No source image selected".to_string();
                }
                Task::none()
            }
            Message::TargetSelected(path) => {
                if let Some(p) = path {
                    self.target_path = Some(p);
                } else {
                    self.status = "No target image selected".to_string();
                }
                Task::none()
            }
            Message::StartMorph => {
                if let (Some(s), Some(t)) = (&self.source_path, &self.target_path) {
                    self.is_morphing = true;
                    self.displacement_map = None;
                    self.current_image_handle = None;

                    let s_img = load_image_path(s).map_err(|e| e.to_string()).unwrap();
                    let t_img = load_image_path(t).map_err(|e| e.to_string()).unwrap();

                    self.source_image = Some(s_img.clone());

                    Task::sip(
                        morph_test(s_img, t_img, 0.1, 2_500_000, 30),
                        Message::Morphing,
                        Message::MorphFinished,
                    )

                    // let (output, current_img) = morph(&s_img, &t_img, 0.1, 2_500_000, 30);
                    // Task::perform(morph_logic(s.clone(), t.clone()), Message::MorphFinished)
                } else {
                    self.status = "Please select both images".to_string();
                    Task::none()
                }
            }
            Message::Morphing(progress) => {
                println!("Progress: {}", progress);
                self.status = format!("Morphing... {:.1}%", progress * 100.0);
                Task::none()
            }
            Message::MorphFinished(result) => {
                self.is_morphing = false;
                self.status = "Creating Displacement Map...".to_string();
                let (output_img, current_img) = result;
                let displacement_map = create_displacement_map(&current_img, output_img.width());
                self.displacement_map = Some(displacement_map);
                Task::done(Message::CreateDisplacementMapFinished)
            }
            Message::CreateDisplacementMapFinished => {
                self.delta = 0.0;
                self.update_frame();
                self.status = "Morphing complete! Use the slider to adjust the morph.".to_string();
                Task::none()
            }
            Message::DeltaChanged(value) => {
                self.delta = value;
                self.update_frame();
                Task::none()
            }
        }
    }

    fn update_frame(&mut self) {
        if let (Some(s_img), Some(disp_map)) = (&self.source_image, &self.displacement_map) {
            let width = s_img.width();
            let height = s_img.height();
            // Initialize with black transparent (or opaque black 0,0,0,255)
            let mut pixels = vec![0u8; (width * height * 4) as usize];
            let t = self.delta as f64;

            for y in 0..height {
                for x in 0..width {
                    let (dx, dy) = disp_map[y as usize][x as usize];

                    // Calculate the current position of the pixel based on the displacement and delta
                    let curr_x = x as f64 + dx * t;
                    let curr_y = y as f64 + dy * t;

                    let ix = curr_x.round() as i32;
                    let iy = curr_y.round() as i32;

                    if ix >= 0 && ix < width as i32 && iy >= 0 && iy < height as i32 {
                        let src_pixel = s_img.get_pixel(x, y);
                        let idx = ((iy as u32 * width + ix as u32) * 4) as usize;
                        if idx + 3 < pixels.len() {
                            pixels[idx] = src_pixel[0];
                            pixels[idx + 1] = src_pixel[1];
                            pixels[idx + 2] = src_pixel[2];
                            pixels[idx + 3] = 255;
                        }
                    }
                }
            }

            // Optional: Fill in any blank pixels by looking at neighbors (simple diffusion)
            let mut rng = rand::thread_rng();
            for _ in 0..5 {
                // Repeat a few times to help fill gaps
                let mut new_pixels = pixels.clone();
                for y in 0..height {
                    for x in 1..width {
                        let idx = ((y * width + x) * 4) as usize;
                        if idx + 3 < pixels.len() && pixels[idx + 3] == 0 {
                            // Look for a non-transparent neighbor
                            let mut filled_neighbors: Vec<usize> = Vec::new();
                            for dy in -1..=1 {
                                for dx in -1..=1 {
                                    if dx == 0 && dy == 0 {
                                        continue;
                                    }
                                    let nx = (x as i32 + dx) as u32;
                                    let ny = (y as i32 + dy) as u32;
                                    if nx < width as u32 && ny < height as u32 {
                                        let n_idx = ((ny as u32 * width + nx as u32) * 4) as usize;
                                        if n_idx + 3 < pixels.len() && pixels[n_idx + 3] != 0 {
                                            filled_neighbors.push(n_idx);
                                        }
                                    }
                                }
                            }

                            // If we have filled neighbors, pick one at random and use its color
                            if !filled_neighbors.is_empty() {
                                let random_pixel: usize = rng.gen_range(0..filled_neighbors.len());
                                let random_idx = filled_neighbors[random_pixel];
                                new_pixels[idx] = pixels[random_idx];
                                new_pixels[idx + 1] = pixels[random_idx + 1];
                                new_pixels[idx + 2] = pixels[random_idx + 2];
                                new_pixels[idx + 3] = 255;
                            }
                        }
                    }
                }
                pixels = new_pixels;
            }

            self.current_image_handle = Some(iced_image::Handle::from_rgba(width, height, pixels));
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let source_label = self
            .source_path
            .as_ref()
            .map(|p| {
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
            .unwrap_or("None".to_string());

        let target_label = self
            .target_path
            .as_ref()
            .map(|p| {
                p.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
            .unwrap_or("None".to_string());

        let source_row = row![
            text("Source Image:"),
            text(source_label),
            button("Select Source").on_press(Message::SelectSource)
        ]
        .spacing(20)
        .align_y(iced::Alignment::Center);

        let target_row = row![
            text("Target Image:"),
            text(target_label),
            button("Select Target").on_press(Message::SelectTarget)
        ]
        .spacing(20)
        .align_y(iced::Alignment::Center);

        let start_button = button("Start Morph").on_press_maybe(if !self.is_morphing {
            Some(Message::StartMorph)
        } else {
            None
        });

        let status_text = text(&self.status);

        let mut content = column![source_row, target_row, start_button, status_text]
            .padding(20)
            .spacing(20);

        // Only show slider and image if we have morph data to display
        if self.displacement_map.is_some() {
            let slider_control = slider(0.0..=1.0, self.delta, Message::DeltaChanged).step(0.01);
            content = content.push(slider_control);
        }

        if let Some(handle) = &self.current_image_handle {
            let image_viewer = iced_image::viewer(handle.clone()).width(Fill).height(Fill);
            content = content.push(image_viewer);
        }

        content.into()
    }
}

async fn pick_file() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("Images", &["png", "jpg", "jpeg", "bmp"])
        .pick_file()
}

// async fn morph_logic(
//     source: PathBuf,
//     target: PathBuf,
// ) -> Result<(RgbImage, Vec<Vec<(f64, f64)>>), String> {
//     let s_img = load_image_path(&source).map_err(|e| e.to_string())?;
//     let t_img = load_image_path(&target).map_err(|e| e.to_string())?;

//     let (output, current_img) = morph(&s_img, &t_img, 0.1, 2_500_000, 30);

//     let displacement_map = create_displacement_map(&current_img, output.width());

//     Ok((s_img, displacement_map))
// }

fn load_image_path(path: &PathBuf) -> Result<RgbImage, image::ImageError> {
    let img = ImageReader::open(path)?.decode()?;
    let rgb_img = img.to_rgb8();
    let rgb_img = resize(
        &rgb_img,
        MAX_DIMENSION,
        MAX_DIMENSION,
        image::imageops::FilterType::Gaussian,
    );
    Ok(rgb_img)
}
