fn main() {
    let filename = std::env::args().nth(1).unwrap();

    let dds = ddsfile::Dds::read(std::fs::File::open(&filename).unwrap()).unwrap();

    dbg!(&dds);

    let num_array_layers = dds.get_num_array_layers();

    assert_eq!(num_array_layers, 1);

    let data = dds.get_data(0).unwrap();

    dbg!(&data.len());

    let header = ktx2_tools::WriterHeader {
        format: Some(match dds.header10.as_ref().unwrap().dxgi_format {
            ddsfile::DxgiFormat::R9G9B9E5_SharedExp => ktx2::Format::E5B9G9R9_UFLOAT_PACK32,
            _ => panic!()
        }),
        type_size: 4,
        pixel_width: dds.header.width,
        pixel_height: dds.header.height,
        pixel_depth: dds.header.depth.unwrap_or(0),
        layer_count: 0,
        face_count: 1,
        supercompression_scheme: Some(ktx2::SupercompressionScheme::Zstandard)
    };

    let dfd = [
        // DFD
        0x0c, 0x00, 0x00, 0x00, // UInt32 dfdTotalSize = 0x3C (60)
        0x01, 0x01, 0x01, 0x01, // vendorId = 0 (17 bits), descriptorType = 0
        0x00, 0x00, 0x0, 0x00, // versionNumber = 2, descriptorBlockSize = 0 (120)
    ];

    dbg!(&dfd.len());

    let writer = ktx2_tools::Writer {
        header: header,
        dfd_bytes: &dfd,
        key_value_pairs: &Default::default(),
        sgd_bytes: &[],
        uncompressed_levels_descending: &[std::borrow::Cow::Borrowed(data)]
    };

    writer.write(&mut std::fs::File::create("out.ktx2").unwrap()).unwrap()
}
