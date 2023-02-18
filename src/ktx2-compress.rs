use ktx2_tools::{Writer, WriterHeader};
use std::borrow::Cow;

fn main() {
    let filename = std::env::args().nth(1).unwrap();
    let output = std::env::args().nth(2).unwrap();

    let bytes = std::fs::read(filename).unwrap();

    let ktx2 = ktx2::Reader::new(&bytes).unwrap();

    let header = ktx2.header();

    println!("{:#?}", header);

    let is_zstd_compressed = match header.supercompression_scheme {
        Some(ktx2::SupercompressionScheme::Zstandard) => true,
        None => false,
        Some(other) => todo!("{:?}", other),
    };

    let writer = Writer {
        header: WriterHeader {
            format: header.format,
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
        uncompressed_levels_descending: &ktx2
            .levels()
            .map(|level| {
                if is_zstd_compressed {
                    Cow::Owned(
                        zstd::bulk::decompress(level.data, level.uncompressed_byte_length as usize)
                            .unwrap(),
                    )
                } else {
                    Cow::Borrowed(level.data)
                }
            })
            .collect::<Vec<_>>(),
    };

    writer
        .write(&mut std::fs::File::create(output).unwrap())
        .unwrap();
}
