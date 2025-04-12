use ktx2_tools::{Writer, WriterHeader};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::borrow::Cow;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Opts {
    input: PathBuf,
    output: PathBuf,
    #[structopt(long)]
    no_zstd: bool,
    #[structopt(long)]
    srgb: bool,
}

fn main() {
    let opts = Opts::from_args();

    let image = image::open(&opts.input).unwrap();

    let image = image.into_rgba8();

    let has_alpha = image.pixels().any(|pixel| pixel.0[3] != 255);

    println!(
        "Width: {}\nHeight: {}\nHas alpha: {}",
        image.width(),
        image.height(),
        has_alpha
    );

    let settings = if has_alpha {
        intel_tex_2::bc7::alpha_slow_settings()
    } else {
        intel_tex_2::bc7::opaque_slow_settings()
    };

    let mut width = image.width().max(4);
    let mut height = image.height().max(4);

    let mut sizes = Vec::new();

    while width >= 4 && height >= 4 {
        sizes.push((width, height));

        width >>= 1;
        height >>= 1;
    }

    let format = if opts.srgb {
        ktx2::Format::BC7_SRGB_BLOCK
    } else {
        ktx2::Format::BC7_UNORM_BLOCK
    };

    let writer = Writer {
        header: WriterHeader {
            format: Some(format),
            type_size: 1,
            pixel_width: image.width().max(4),
            pixel_height: image.height().max(4),
            pixel_depth: 0,
            layer_count: 1,
            face_count: 1,
            supercompression_scheme: if opts.no_zstd {
                None
            } else {
                Some(ktx2::SupercompressionScheme::Zstandard)
            },
        },
        dfd_bytes: &4_u32.to_le_bytes(),
        key_value_pairs: &Default::default(),
        sgd_bytes: &[],
        uncompressed_levels_descending: &sizes
            .into_par_iter()
            .map(|(width, height)| {
                let resized = image::imageops::resize(
                    &image,
                    width,
                    height,
                    image::imageops::FilterType::Triangle,
                );

                let compressed = intel_tex_2::bc7::compress_blocks(
                    &settings,
                    &intel_tex_2::RgbaSurface {
                        data: &resized,
                        width: intel_tex_2::divide_up_by_multiple(width, 4) * 4,
                        height, //: intel_tex_2::divide_up_by_multiple(height, 4) * 4,
                        stride: width * 4,
                    },
                );

                Cow::Owned(compressed)
            })
            .collect::<Vec<_>>(),
    };

    writer
        .write(&mut std::fs::File::create(&opts.output).unwrap())
        .unwrap();
}
