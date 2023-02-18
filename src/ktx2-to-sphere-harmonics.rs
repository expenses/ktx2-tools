use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Opts {
    input: PathBuf,
    output: Option<PathBuf>,
}

fn main() {
    let opts = Opts::from_args();

    let bytes = std::fs::read(&opts.input).unwrap();

    let ktx2 = ktx2::Reader::new(&bytes).unwrap();

    let header = ktx2.header();

    let level = ktx2.levels().next().unwrap();

    assert_eq!(header.format, Some(ktx2::Format::R32G32B32A32_SFLOAT));

    let level_bytes = match header.supercompression_scheme {
        Some(ktx2::SupercompressionScheme::Zstandard) => std::borrow::Cow::Owned(
            zstd::bulk::decompress(level.data, level.uncompressed_byte_length as usize).unwrap(),
        ),
        Some(other) => todo!("{:?}", other),
        None => std::borrow::Cow::Borrowed(level.data),
    };

    let images: Vec<image::ImageBuffer<image::Rgb<f32>, Vec<f32>>> = level_bytes
        .chunks(level_bytes.len() / 6)
        .map(|bytes| {
            let rgb_floats: Vec<f32> = bytes
                .chunks(16)
                .flat_map(|rgba| {
                    [
                        f32::from_le_bytes(<[u8; 4]>::try_from(&rgba[0..4]).unwrap()),
                        f32::from_le_bytes(<[u8; 4]>::try_from(&rgba[4..8]).unwrap()),
                        f32::from_le_bytes(<[u8; 4]>::try_from(&rgba[8..12]).unwrap()),
                    ]
                })
                .collect();

            image::ImageBuffer::from_raw(header.pixel_width, header.pixel_height, rgb_floats)
                .unwrap()
        })
        .collect();

    let coefficients = cubemap_spherical_harmonics::process(&images).unwrap();

    if let Some(ref output_filename) = opts.output {
        let bytes: Vec<u8> = coefficients
            .iter()
            .flat_map(|vec| vec.to_array())
            .flat_map(|float| float.to_le_bytes())
            .collect();

        std::fs::write(output_filename, bytes).unwrap();
    } else {
        println!("{:?}", coefficients);
    }
}
