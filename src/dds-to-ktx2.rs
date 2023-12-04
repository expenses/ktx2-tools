fn main() {
    let filename = std::env::args().nth(1).unwrap();

    let dds = ddsfile::Dds::read(std::fs::File::open(&filename).unwrap()).unwrap();

    dbg!(&dds);

    let num_array_layers = dds.get_num_array_layers();

    assert_eq!(num_array_layers, 1);

    let num_mipmap_levels = dds.get_num_mipmap_levels();

    let format = dds.header10.as_ref().unwrap().dxgi_format;

    let header = ktx2_tools::WriterHeader {
        format: Some(match format {
            ddsfile::DxgiFormat::R9G9B9E5_SharedExp => ktx2::Format::E5B9G9R9_UFLOAT_PACK32,
            ddsfile::DxgiFormat::BC1_UNorm => ktx2::Format::BC1_RGB_SRGB_BLOCK,
            _ => panic!("{:?}", format),
        }),
        type_size: 4,
        pixel_width: dds.header.width,
        pixel_height: dds.header.height,
        pixel_depth: dds.header.depth.unwrap_or(0),
        layer_count: 0,
        face_count: 1,
        supercompression_scheme: None, //Some(ktx2::SupercompressionScheme::Zstandard)
    };

    let width = dds.header.width as usize;
    let height = dds.header.height as usize;
    let mut offset = 0;

    let mut levels = Vec::new();

    for i in 0..num_mipmap_levels {
        let level_width = (width >> i).max(4);
        let level_height = (height >> i).max(4);
        let data: &[u8] = &dds.data[offset..offset + level_width * level_height / 2];
        offset += data.len();

        levels.push(std::borrow::Cow::Borrowed(data));
    }

    dbg!(offset, dds.data.len());

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
        uncompressed_levels_descending: &levels,
    };

    writer
        .write(&mut std::fs::File::create("out.ktx2").unwrap())
        .unwrap()
}
