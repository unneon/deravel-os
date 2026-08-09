#![no_std]
#![no_main]
extern crate alloc;

use deravel_kernel_api::*;

#[derive(Debug)]
enum Image<'a> {
    PpmRaw {
        width: usize,
        height: usize,
        maxval: usize,
        sample_size: usize,
        raster: &'a [u8],
    },
}

const PPM_RAW_MAGIC: [u8; 2] = *b"P6";

impl Image<'_> {
    fn width(&self) -> usize {
        let Image::PpmRaw { width, .. } = self;
        *width
    }

    fn height(&self) -> usize {
        let Image::PpmRaw { height, .. } = self;
        *height
    }

    fn rgb(&self, x: usize, y: usize) -> (u8, u8, u8) {
        match self {
            Image::PpmRaw {
                width,
                maxval,
                sample_size,
                raster,
                ..
            } => {
                let row = &raster[y * width * 3 * sample_size..];
                let pixel = &row[x * 3 * sample_size..];
                let r = ppm_sample(&pixel[..*sample_size], *maxval);
                let g = ppm_sample(&pixel[*sample_size..2 * sample_size], *maxval);
                let b = ppm_sample(&pixel[2 * sample_size..3 * sample_size], *maxval);
                (r, g, b)
            }
        }
    }
}

fn main(args: ImageViewerArgs) {
    let image = unsafe { &(*map_shared(args.image)).0 };
    let image = parse_image(image);
    let min_width: usize = 400;
    let min_height: usize = 300;
    let scale = min_width
        .div_ceil(image.width())
        .min(min_height.div_ceil(image.height()));
    let window_width = image.width() * scale;
    let window_height = image.height() * scale;
    let window = args
        .windowing
        .create_window(window_width as u32, window_height as u32);
    let mut framebuffer = Framebuffer::map(window_width, window_height, window.framebuffer());
    for y in 0..image.height() {
        for x in 0..image.width() {
            let (r, g, b) = image.rgb(x, y);
            for sy in 0..scale {
                for sx in 0..scale {
                    framebuffer.set_pixel(x * scale + sx, y * scale + sy, r, g, b, 255);
                }
            }
        }
    }
    window.draw();
    loop {
        yield_();
    }
}

fn parse_image(image: &[u8]) -> Image<'_> {
    if image[..PPM_RAW_MAGIC.len()] == PPM_RAW_MAGIC {
        ppm_parse(image)
    } else {
        panic!("unsupported image format")
    }
}

fn ppm_parse(mut image: &[u8]) -> Image<'_> {
    ppm_parse_magic(&mut image);
    ppm_parse_whitespace_and_comments(&mut image);
    let width = ppm_parse_number(&mut image);
    ppm_parse_whitespace_and_comments(&mut image);
    let height = ppm_parse_number(&mut image);
    ppm_parse_whitespace_and_comments(&mut image);
    let maxval = ppm_parse_number(&mut image);
    assert!(maxval > 0);
    assert!(maxval < 65536);
    ppm_parse_single_whitespace(&mut image);
    let sample_size = if maxval < 256 { 1 } else { 2 };
    let raster = ppm_parse_raster(width, height, sample_size, &mut image);
    // TODO: Handle multiple images/invalid trailing data.
    Image::PpmRaw {
        width,
        height,
        maxval,
        sample_size,
        raster,
    }
}

fn ppm_parse_magic(image: &mut &[u8]) {
    assert_eq!(image[..PPM_RAW_MAGIC.len()], PPM_RAW_MAGIC);
    *image = &image[PPM_RAW_MAGIC.len()..];
}

fn ppm_parse_whitespace_and_comments(image: &mut &[u8]) {
    let mut skipped = false;
    loop {
        if image[0].is_ascii_whitespace() {
            *image = &image[1..];
            skipped = true;
        } else if image[0] == b'#' {
            while image[0] != b'\n' {
                *image = &image[1..];
            }
            *image = &image[1..];
            skipped = true;
        } else {
            assert!(skipped);
            break;
        }
    }
}

fn ppm_parse_number(image: &mut &[u8]) -> usize {
    let mut i = 0;
    while image[i].is_ascii_digit() {
        i += 1;
    }
    let number = str::from_utf8(&image[..i]).unwrap().parse().unwrap();
    *image = &image[i..];
    number
}

fn ppm_parse_single_whitespace(image: &mut &[u8]) {
    assert!(image[0].is_ascii_whitespace());
    *image = &image[1..];
}

fn ppm_parse_raster<'a>(
    width: usize,
    height: usize,
    sample_size: usize,
    image: &mut &'a [u8],
) -> &'a [u8] {
    let pixel_size = 3 * sample_size;
    let row_size = width * pixel_size;
    let image_size = height * row_size;
    let raster = &image[..image_size];
    *image = &image[image_size..];
    raster
}

fn ppm_sample(sample: &[u8], maxval: usize) -> u8 {
    let sample = match sample {
        [sample] => *sample as usize,
        [msb, lsb] => *msb as usize * 256 + *lsb as usize,
        _ => unreachable!(),
    };
    (sample * 255 / maxval) as u8
}

app! { main }
