fn main() {
    let filename = std::env::args().nth(1).unwrap();
    let output = std::env::args().nth(2).unwrap();
    let bytes = std::fs::read(&filename).unwrap();

    let ktx2 = ktx2::Reader::new(&bytes).unwrap();

    let header = ktx2.header();

    println!("{:#?}", header);

    let level = ktx2.levels().next().unwrap();

    let level_bytes = match header.supercompression_scheme {
        Some(ktx2::SupercompressionScheme::Zstandard) => std::borrow::Cow::Owned(
            zstd::bulk::decompress(level.bytes, level.uncompressed_byte_length as usize)
                .unwrap(),
        ),
        Some(other) => todo!("{:?}", other),
        None => std::borrow::Cow::Borrowed(level.bytes),
    };

    std::fs::write(output, level_bytes).unwrap();   
}
