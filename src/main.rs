use iced::Length::Fill;
use iced::time::{self, Duration};
use iced::widget::canvas::{Geometry, Program};
use iced::widget::{button, canvas, column, progress_bar, radio, row, slider, text};
use iced::{Color, Element, Font, Point, Rectangle, Renderer, Size, Task, Theme};
use image::RgbImage;
use image::imageops::resize;
use image_morph_rust::{create_displacement_map, morph_test};

use rand::Rng;

pub const FONT: Font = Font::with_name("DepartureMono Nerd Font");

pub fn main() -> iced::Result {
    iced::application(ImageMorph::new, ImageMorph::update, ImageMorph::view)
        .font(include_bytes!("../fonts/DepartureMonoNerdFont-Regular.otf"))
        .default_font(FONT)
        .title("Image Morph")
        .subscription(ImageMorph::subscription)
        .run()
}

#[derive(Default, Clone, Copy)]
struct Pixel {
    x: f32,
    y: f32,
    r: u8,
    g: u8,
    b: u8,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum MorphType {
    #[default]
    Linear,
    Diffuse,
}

struct MorphCanvas {
    options_open: bool,
    max_dimension: u32,
    pixels: Option<Vec<Pixel>>,
    displacement_map: Option<Vec<Vec<(f64, f64)>>>,
    delta: f32,
    canvas_cache: canvas::Cache,
    morph_type: MorphType,
}

impl Default for MorphCanvas {
    fn default() -> Self {
        Self {
            options_open: false,
            max_dimension: 200,
            pixels: None,
            displacement_map: None,
            delta: 0.0,
            canvas_cache: canvas::Cache::default(),
            morph_type: MorphType::default(),
        }
    }
}

struct ImageMorph {
    source_file: Option<(String, Vec<u8>)>,
    target_file: Option<(String, Vec<u8>)>,
    status: String,
    is_morphing: bool,
    morph_progress: Option<f64>,
    source_image: Option<RgbImage>,
    is_playing: bool,
    morph_canvas: MorphCanvas,
}

#[derive(Debug, Clone)]
enum Message {
    SelectSource,
    SelectTarget,
    SourceSelected(Option<(String, Vec<u8>)>),
    TargetSelected(Option<(String, Vec<u8>)>),
    StartMorph,
    Morphing(f64),
    MorphFinished((RgbImage, Vec<Vec<u32>>)),
    CreateDisplacementMapFinished,
    DeltaChanged(f32),
    MaxDimensionChanged(u32),
    MorphTypeChanged(MorphType),
    ToggleOptions,
    TogglePlay,
    Tick,
}

impl Default for ImageMorph {
    fn default() -> Self {
        Self {
            source_file: None,
            target_file: None,
            status: "Ready".to_string(),
            is_morphing: false,
            morph_progress: None,
            source_image: None,
            is_playing: false,
            morph_canvas: MorphCanvas::default(),
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
                    self.source_file = Some(p);
                } else {
                    self.status = "No source image selected".to_string();
                }
                Task::none()
            }
            Message::TargetSelected(path) => {
                if let Some(p) = path {
                    self.target_file = Some(p);
                } else {
                    self.status = "No target image selected".to_string();
                }
                Task::none()
            }
            Message::StartMorph => {
                if let (Some((_, s_bytes)), Some((_, t_bytes))) =
                    (&self.source_file, &self.target_file)
                {
                    // This is for the browser since wasm is single-threaded
                    // self.status = "Morphing now... (The browser will hang while this is going on)"
                    //     .to_string();

                    self.is_morphing = true;
                    self.morph_canvas.displacement_map = None;
                    self.morph_canvas.pixels = None;

                    let s_img = load_image_bytes(&s_bytes, self.morph_canvas.max_dimension)
                        .map_err(|e| e.to_string())
                        .unwrap();
                    let t_img = load_image_bytes(&t_bytes, self.morph_canvas.max_dimension)
                        .map_err(|e| e.to_string())
                        .unwrap();

                    self.source_image = Some(s_img.clone());

                    let num_iterations =
                        self.morph_canvas.max_dimension * self.morph_canvas.max_dimension * 100;
                    let search_radius = self.morph_canvas.max_dimension / 5;

                    Task::sip(
                        morph_test(s_img, t_img, 0.3, num_iterations, search_radius),
                        Message::Morphing,
                        Message::MorphFinished,
                    )
                } else {
                    self.status = "Please select both images".to_string();
                    Task::none()
                }
            }
            Message::Morphing(progress) => {
                self.status = format!("Morphing... {:.1}%", progress * 100.0);
                self.morph_progress = Some(progress);
                Task::none()
            }
            Message::MorphFinished(result) => {
                self.is_morphing = false;
                self.morph_progress = None;
                self.status = "Creating Displacement Map...".to_string();

                let (output_img, current_img) = result;
                let displacement_map = create_displacement_map(&current_img, output_img.width());
                self.morph_canvas.displacement_map = Some(displacement_map);
                self.morph_canvas.max_dimension = self.morph_canvas.max_dimension;
                Task::done(Message::CreateDisplacementMapFinished)
            }
            Message::CreateDisplacementMapFinished => {
                self.morph_canvas.delta = 0.0;
                self.update_frame();
                self.status = "Morphing complete! Use the slider to adjust the morph.".to_string();
                Task::none()
            }
            Message::DeltaChanged(value) => {
                self.morph_canvas.delta = value;
                self.update_frame();
                Task::none()
            }
            Message::MaxDimensionChanged(value) => {
                self.morph_canvas.max_dimension = value;
                Task::none()
            }
            Message::MorphTypeChanged(value) => {
                self.morph_canvas.morph_type = value;
                self.update_frame();
                Task::none()
            }
            Message::ToggleOptions => {
                self.morph_canvas.options_open = !self.morph_canvas.options_open;
                Task::none()
            }
            Message::TogglePlay => {
                self.is_playing = !self.is_playing;
                Task::none()
            }
            Message::Tick => {
                if self.is_playing {
                    self.morph_canvas.delta += 0.01;
                    if self.morph_canvas.delta >= 1.0 {
                        self.morph_canvas.delta = 1.0;
                        self.is_playing = false;
                    }
                    self.update_frame();
                }
                Task::none()
            }
        }
    }

    fn update_frame(&mut self) {
        if let (Some(s_img), Some(disp_map)) =
            (&self.source_image, &self.morph_canvas.displacement_map)
        {
            let width = s_img.width();
            let height = s_img.height();
            let t = self.morph_canvas.delta as f64;

            let mut pixels = Vec::with_capacity((width * height) as usize);

            match self.morph_canvas.morph_type {
                MorphType::Linear => {
                    for y in 0..height {
                        for x in 0..width {
                            let (dx, dy) = disp_map[y as usize][x as usize];

                            // Calculate the current position of the pixel based on the displacement and delta
                            let curr_x = x as f64 + dx * t;
                            let curr_y = y as f64 + dy * t;

                            let src_pixel = s_img.get_pixel(x, y);
                            pixels.push(Pixel {
                                x: curr_x as f32,
                                y: curr_y as f32,
                                r: src_pixel[0],
                                g: src_pixel[1],
                                b: src_pixel[2],
                            });
                        }
                    }
                }

                MorphType::Diffuse => {
                    let mut pixel_data = vec![0u8; (width * height * 4) as usize]; // RGBA format
                    for y in 0..height {
                        for x in 0..width {
                            let (dx, dy) = disp_map[y as usize][x as usize];

                            // Cast to integer for diffusion
                            let curr_x = (x as f64 + dx * t).round() as i32;
                            let curr_y = (y as f64 + dy * t).round() as i32;

                            if curr_x < 0
                                || curr_x >= width as i32
                                || curr_y < 0
                                || curr_y >= height as i32
                            {
                                continue; // Skip out-of-bounds pixels
                            }
                            let src_pixel = s_img.get_pixel(x, y);
                            let idx = ((curr_y as u32 * width + curr_x as u32) * 4) as usize;
                            if idx + 3 >= pixel_data.len() {
                                continue; // Skip out-of-bounds indices
                            }
                            pixel_data[idx] = src_pixel[0];
                            pixel_data[idx + 1] = src_pixel[1];
                            pixel_data[idx + 2] = src_pixel[2];
                            pixel_data[idx + 3] = 255;
                        }
                    }
                    // Fill in any blank pixels by looking at neighbors (simple diffusion)
                    let mut rng = rand::thread_rng();
                    for _ in 0..5 {
                        // Repeat a few times to help fill gaps
                        let mut next_pixel_data = pixel_data.clone();
                        for y in 0..height {
                            for x in 0..width {
                                let idx = ((y * width + x) * 4) as usize;
                                if pixel_data[idx + 3] == 0 {
                                    // Look for a non-transparent neighbor
                                    let mut filled_neighbors: Vec<usize> = Vec::new();
                                    for dy in -1..=1 {
                                        for dx in -1..=1 {
                                            if dx == 0 && dy == 0 {
                                                continue;
                                            }
                                            let nx = (x as i32 + dx) as u32;
                                            let ny = (y as i32 + dy) as u32;
                                            if nx < width && ny < height {
                                                let n_idx = ((ny * width + nx) * 4) as usize;
                                                if n_idx + 3 < pixel_data.len()
                                                    && pixel_data[n_idx + 3] != 0
                                                {
                                                    filled_neighbors.push(n_idx);
                                                }
                                            }
                                        }
                                    }

                                    // If we have filled neighbors, pick one at random and use its color
                                    if !filled_neighbors.is_empty() {
                                        let random_pixel: usize =
                                            rng.gen_range(0..filled_neighbors.len());
                                        let random_idx = filled_neighbors[random_pixel];
                                        next_pixel_data[idx] = pixel_data[random_idx];
                                        next_pixel_data[idx + 1] = pixel_data[random_idx + 1];
                                        next_pixel_data[idx + 2] = pixel_data[random_idx + 2];
                                        next_pixel_data[idx + 3] = 255;
                                    }
                                }
                            }
                        }
                        pixel_data = next_pixel_data;
                    }
                    // Convert back to Pixel format for rendering
                    for y in 0..height {
                        for x in 0..width {
                            let idx = ((y * width + x) * 4) as usize;
                            if idx + 3 < pixel_data.len() && pixel_data[idx + 3] != 0 {
                                pixels.push(Pixel {
                                    x: x as f32,
                                    y: y as f32,
                                    r: pixel_data[idx],
                                    g: pixel_data[idx + 1],
                                    b: pixel_data[idx + 2],
                                });
                            }
                        }
                    }
                }
            }

            self.morph_canvas.pixels = Some(pixels);
            self.morph_canvas.canvas_cache.clear();
        }
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        if self.is_playing {
            time::every(Duration::from_millis(33)).map(|_| Message::Tick)
        } else {
            iced::Subscription::none()
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let source_label = if let Some((file_name, _)) = &self.source_file {
            file_name
        } else {
            "None"
        };

        let target_label = if let Some((file_name, _)) = &self.target_file {
            file_name
        } else {
            "None"
        };

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

        let start_button_enabled =
            !self.is_morphing && self.target_file.is_some() && self.source_file.is_some();

        let start_button = button("Start Morph").on_press_maybe(if start_button_enabled {
            Some(Message::StartMorph)
        } else {
            None
        });

        let status_text = text(&self.status);

        let dimension_slider = row![
            text("Max Dimension:"),
            slider(
                50..=500,
                self.morph_canvas.max_dimension,
                Message::MaxDimensionChanged
            ),
            text(format!("{}", self.morph_canvas.max_dimension))
        ]
        .spacing(10)
        .align_y(iced::Alignment::Center);

        let morph_type_options = row![
            text("Morph Type:"),
            radio(
                "Linear",
                MorphType::Linear,
                Some(self.morph_canvas.morph_type),
                Message::MorphTypeChanged
            ),
            radio(
                "Diffuse",
                MorphType::Diffuse,
                Some(self.morph_canvas.morph_type),
                Message::MorphTypeChanged
            ),
        ]
        .spacing(20)
        .align_y(iced::Alignment::Center);

        let options_section = column![
            text("Options").size(20),
            dimension_slider,
            morph_type_options
        ]
        .spacing(10);

        let options_toggle = button(if self.morph_canvas.options_open {
            "Hide Options"
        } else {
            "Show Options"
        })
        .on_press(Message::ToggleOptions);

        let mut content = column![source_row, target_row, options_toggle]
            .padding(20)
            .spacing(20);

        if self.morph_canvas.options_open {
            content = content.push(options_section);
        }

        content = content.push(start_button).push(status_text);

        if self.morph_progress.is_some() {
            let progress = self.morph_progress.unwrap_or(0.0) as f32;
            let bar = progress_bar(0.0..=1.0, progress);
            content = content.push(bar);
        }

        // Only show slider and image if we have morph data to display
        if self.morph_canvas.displacement_map.is_some() {
            let play_button = button(if self.is_playing { "Pause" } else { "Play" })
                .on_press(Message::TogglePlay);
            let slider_control =
                slider(0.0..=1.0, self.morph_canvas.delta, Message::DeltaChanged).step(0.01);
            content = content.push(row![play_button, slider_control].spacing(20));
        }

        if let Some(_) = &self.morph_canvas.pixels {
            let image_canvas = canvas::Canvas::new(&self.morph_canvas)
                .width(Fill)
                .height(Fill);
            content = content.push(image_canvas);
        }

        content.into()
    }
}

impl<Message> Program<Message> for MorphCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let content = self.canvas_cache.draw(renderer, bounds.size(), |frame| {
            if let Some(pixels) = &self.pixels {
                let p_w = bounds.width / self.max_dimension as f32;
                let p_h = bounds.height / self.max_dimension as f32;
                for pixel in pixels {
                    frame.fill_rectangle(
                        Point::new(pixel.x * p_w, pixel.y * p_h),
                        Size::new(p_w, p_h),
                        Color::from_rgb8(pixel.r, pixel.g, pixel.b),
                    );
                }
            }
        });

        vec![content]
    }
}

/// Uses `rfd::AsyncFileDialog` which works on both native desktop targets
async fn pick_file() -> Option<(String, Vec<u8>)> {
    let handle = rfd::AsyncFileDialog::new()
        .add_filter("Images", &["png", "jpg", "jpeg", "bmp"])
        .pick_file()
        .await?;

    let name = handle.file_name();
    let bytes = handle.read().await;
    Some((name, bytes))
}

/// Decode an image from raw in-memory bytes and resize it so neither
/// dimension exceeds `max_dimension`.  Works identically on native and WASM
/// because `image::load_from_memory` never touches the filesystem.
fn load_image_bytes(bytes: &[u8], max_dimension: u32) -> Result<RgbImage, image::ImageError> {
    let img = image::load_from_memory(bytes)?;
    let rgb_img = img.to_rgb8();
    let rgb_img = resize(
        &rgb_img,
        max_dimension,
        max_dimension,
        image::imageops::FilterType::Gaussian,
    );
    Ok(rgb_img)
}
