fn main() {
    let filename = std::env::args().nth(1).unwrap();

    let file = std::fs::read(&filename).unwrap();
    let ktx2 = ktx2::Reader::new(&file).unwrap();

    let header = ktx2.header();

    let mut width = header.pixel_width;
    let mut height = header.pixel_height;

    println!("[File {}]", filename);
    println!("Width: {}", width);
    println!("Height: {}", height);
    println!("Depth: {}", header.pixel_depth);
    println!("Level Count: {}", header.level_count);
    println!(
        "Supercompression Scheme: {:?}",
        header.supercompression_scheme
    );
    println!("Format: {:?}", header.format);
    println!();

    for (i, dfd) in ktx2.data_format_descriptors().enumerate() {
        println!("[Data format descriptor {}]", i);
        println!("Vendor ID: {}", dfd.header.vendor_id);
        println!("Version number: {}", dfd.header.version_number);

        if let Ok(basis_dfd) = ktx2::BasicDataFormatDescriptor::parse(dfd.data) {
            println!("[[Basis Universal data format descriptor]]");
            println!("Color model: {:?}", basis_dfd.color_model);
            println!("Color primaries: {:?}", basis_dfd.color_primaries);
            println!("Transfer function: {:?}", basis_dfd.transfer_function);
            println!("Flags: {:?}", basis_dfd.flags);
        }

        println!();
    }

    println!("[Key Value Pairs]");

    for (key, value) in ktx2.key_value_data() {
        println!("{}: {}", key, String::from_utf8_lossy(value));
    }

    println!();

    if true {
        for (i, level) in ktx2.levels().enumerate() {
            println!("[Level {} (width: {}, height: {})]", i, width, height);
            println!("Byte length: {}", level.bytes.len());
            println!(
                "Uncompressed byte length: {}",
                level.uncompressed_byte_length
            );
            println!();

            width >>= 1;
            height >>= 1;
        }
    }
}
