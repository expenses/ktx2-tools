use ktx2_tools::{Writer, WriterHeader, WriterLevel};

fn main() {
    let filename = std::env::args().nth(1).unwrap();
    let output = std::env::args().nth(2).unwrap();

    let bytes = std::fs::read(&filename).unwrap();

    let ktx2 = ktx2::Reader::new(&bytes).unwrap();

    let header = ktx2.header();

    println!("{:#?}", header);

    if let Some(scheme) = header.supercompression_scheme {
        panic!("Expected there to be no scheme, got: {:?}", scheme);
    }

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
        kvd_bytes: &bytes[header.index.kvd_byte_offset as usize
            ..(header.index.kvd_byte_offset + header.index.kvd_byte_length) as usize],
        sgd_bytes: &bytes[header.index.sgd_byte_offset as usize
            ..(header.index.sgd_byte_offset + header.index.sgd_byte_length) as usize],
        levels_descending: ktx2
            .levels()
            .map(|level| WriterLevel {
                uncompressed_length: level.uncompressed_byte_length as usize,
                bytes: zstd::bulk::compress(level.bytes, 0).unwrap(),
            })
            .collect(),
    };

    writer
        .write(&mut std::fs::File::create(output).unwrap())
        .unwrap();
}
