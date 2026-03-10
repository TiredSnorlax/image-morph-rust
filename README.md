# Image Morph Rust

A Rust-based image morphing application using the Iced GUI framework. It performs pixel-level morphing between two images by moving them.

## Showcase
The website demo can be found [here](https://tiredsnorlax.github.io/image-morph-rust/)

https://github.com/user-attachments/assets/22b910d6-8716-494d-b0e1-fe5c44b7e2c9

## Morphing Logic

The core logic implements a swap-based optimization algorithm to map pixels from a source image to a target image while minimizing a combined cost function.

### Cost Function
The algorithm evaluates the "quality" of a pixel mapping using two primary metrics:
1. **Color Distance**: The Euclidean distance between the RGB values of a source pixel and its corresponding target pixel.
2. **Displacement Cost**: The distance between a pixel's current position and its original coordinates in the source image, normalized by the image dimensions.

The total cost is a weighted sum: `Total Cost = Color Cost + (Proximity Weight * Displacement Cost)`.

### Optimization Process
The morphing is performed through an iterative process:
- A random pixel and a potential swap candidate within a decaying search radius are selected.
- The algorithm calculates the change in total cost if the two pixels were to swap their target mapping.
- If the swap reduces the total cost, it is accepted.
- Over thousands of iterations, this results in a source image where pixels have migrated to positions that resemble the target image while maintaining relative proximity to their original neighbors.

## Features

- **Real-time Rendering**: A custom Iced canvas implementation for hardware-accelerated pixel rendering.
- **Morph Modes**: 
  - **Linear**: Direct interpolation of pixel positions.
  - **Diffuse**: A gap-filling algorithm that uses neighbor color sampling to create a smoother transition.
- **Adjustable Parameters**: Control the maximum image dimension and toggle between different morphing behaviors.
- **WASM Support**: Designed to run both natively and in the browser via WebAssembly.
