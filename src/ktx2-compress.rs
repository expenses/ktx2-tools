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
        levels: ktx2
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

struct Writer<'a> {
    header: WriterHeader,
    dfd_bytes: &'a [u8],
    kvd_bytes: &'a [u8],
    sgd_bytes: &'a [u8],
    levels: Vec<WriterLevel>,
}

impl<'a> Writer<'a> {
    fn write<T: std::io::Write>(&self, writer: &mut T) -> std::io::Result<()> {
        let dfd_offset = ktx2::Header::LENGTH + self.levels.len() * ktx2::LevelIndex::LENGTH;

        writer.write_all(
            &ktx2::Header {
                format: self.header.format,
                type_size: self.header.type_size,
                pixel_width: self.header.pixel_width,
                pixel_height: self.header.pixel_height,
                pixel_depth: self.header.pixel_depth,
                layer_count: self.header.layer_count,
                face_count: self.header.face_count,
                supercompression_scheme: self.header.supercompression_scheme,
                level_count: self.levels.len() as u32,
                index: ktx2::Index {
                    dfd_byte_length: self.dfd_bytes.len() as u32,
                    kvd_byte_length: self.kvd_bytes.len() as u32,
                    sgd_byte_length: self.sgd_bytes.len() as u64,
                    dfd_byte_offset: dfd_offset as u32,
                    kvd_byte_offset: (dfd_offset + self.kvd_bytes.len()) as u32,
                    sgd_byte_offset: (dfd_offset + self.kvd_bytes.len() + self.kvd_bytes.len())
                        as u64,
                },
            }
            .as_bytes()[..],
        )?;

        let mut offset =
            dfd_offset + self.dfd_bytes.len() + self.kvd_bytes.len() + self.sgd_bytes.len();

        for level in &self.levels {
            writer.write_all(
                &ktx2::LevelIndex {
                    byte_offset: offset as u64,
                    byte_length: level.bytes.len() as u64,
                    uncompressed_byte_length: level.uncompressed_length as u64,
                }
                .as_bytes(),
            )?;

            offset += level.bytes.len();
        }

        writer.write_all(self.dfd_bytes)?;
        writer.write_all(self.kvd_bytes)?;
        writer.write_all(self.sgd_bytes)?;

        for level in &self.levels {
            writer.write_all(&level.bytes)?;
        }

        Ok(())
    }
}

struct WriterLevel {
    uncompressed_length: usize,
    bytes: Vec<u8>,
}

struct WriterHeader {
    format: Option<ktx2::Format>,
    type_size: u32,
    pixel_width: u32,
    pixel_height: u32,
    pixel_depth: u32,
    layer_count: u32,
    face_count: u32,
    supercompression_scheme: Option<ktx2::SupercompressionScheme>,
}
