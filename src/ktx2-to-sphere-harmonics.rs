fn main() {
    let mut args = std::env::args().skip(1);
    let input_filename = args.next().unwrap();
    let output_filename = args.next().unwrap();
    let bytes = std::fs::read(&input_filename).unwrap();

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

    let res: Vec<u8> = cubemap_spherical_harmonics::process(&images)
        .unwrap()
        .iter()
        .flat_map(|vec| vec.to_array())
        .flat_map(|float| float.to_le_bytes())
        .collect();

    std::fs::write(output_filename, &res).unwrap();
}
