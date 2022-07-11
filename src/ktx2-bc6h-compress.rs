use ktx2_tools::{Writer, WriterHeader, WriterLevel};

fn main() {
    let mut args = std::env::args().skip(1);
    let input_filename = args.next().unwrap();
    let output_filename = args.next().unwrap();

    let bytes = std::fs::read(&input_filename).unwrap();
    let ktx2 = ktx2::Reader::new(&bytes).unwrap();

    let header = ktx2.header();

    let num_levels = header
        .level_count
        .min((header.pixel_width.min(header.pixel_height) as f32).log2() as u32 - 1);

    println!("{:#?} {}", header, num_levels);

    assert_eq!(header.format, Some(ktx2::Format::R16G16B16A16_SFLOAT));

    assert_eq!(header.face_count, 6);

    let writer = Writer {
        header: WriterHeader {
            format: Some(ktx2::Format::BC6H_UFLOAT_BLOCK),
            type_size: header.type_size,
            pixel_width: header.pixel_width,
            pixel_height: header.pixel_height,
            pixel_depth: header.pixel_depth,
            layer_count: header.layer_count,
            face_count: header.face_count,
            supercompression_scheme: Some(ktx2::SupercompressionScheme::Zstandard),
        },
        dfd_bytes: &bytes[header.index.dfd_byte_offset as usize
            ..(header.index.dfd_byte_offset + header.index.dfd_byte_length) as usize],
        key_value_pairs: &Default::default(),
        sgd_bytes: &[],
        levels_descending: ktx2
            .levels()
            .take(num_levels as usize)
            .enumerate()
            .map(|(i, level)| {
                let level_bytes = match header.supercompression_scheme {
                    Some(ktx2::SupercompressionScheme::Zstandard) => std::borrow::Cow::Owned(
                        zstd::bulk::decompress(level.data, level.uncompressed_byte_length as usize)
                            .unwrap(),
                    ),
                    Some(other) => todo!("{:?}", other),
                    None => std::borrow::Cow::Borrowed(level.data),
                };

                let width = header.pixel_width >> i;
                let height = header.pixel_height >> i;

                let mut compressed = Vec::new();

                for chunk in level_bytes.chunks(level_bytes.len() / 6) {
                    let compressed_chunk = intel_tex_2::bc6h::compress_blocks(
                        &intel_tex_2::bc6h::very_slow_settings(),
                        &intel_tex_2::RgbaSurface {
                            width,
                            height,
                            stride: width * 8,
                            data: chunk,
                        },
                    );

                    compressed.extend_from_slice(&compressed_chunk);
                }

                WriterLevel {
                    uncompressed_length: compressed.len(),
                    bytes: zstd::bulk::compress(&compressed, 0).unwrap(),
                }
            })
            .collect(),
    };

    writer
        .write(&mut std::fs::File::create(output_filename).unwrap())
        .unwrap();
}
