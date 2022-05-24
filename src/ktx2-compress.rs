fn main() {
    let filename = std::env::args().nth(1).unwrap();
    let output = std::env::args().nth(2).unwrap();
    
    let bytes = std::fs::read(&filename).unwrap();

    let ktx2 = ktx2::Reader::new(&bytes).unwrap();

    let mut header = ktx2.header();

    println!("{:#?}", header);

    if let Some(scheme) = header.supercompression_scheme {
        panic!("Expected there to be no scheme, got: {:?}", scheme);
    }

    header.supercompression_scheme = Some(ktx2::SupercompressionScheme::Zstandard);

    let writer = Writer {
        header,
        dfd_bytes: &bytes[header.index.dfd_byte_offset as usize..(header.index.dfd_byte_offset + header.index.dfd_byte_length) as usize],
        kvd_bytes: &bytes[header.index.kvd_byte_offset as usize..(header.index.kvd_byte_offset + header.index.kvd_byte_length) as usize],
        sgd_bytes: &bytes[header.index.sgd_byte_offset as usize..(header.index.sgd_byte_offset + header.index.sgd_byte_length) as usize],
        levels: ktx2.levels().map(|level| {
            WriterLevel {
                uncompressed_length: level.uncompressed_byte_length as usize,
                bytes: zstd::bulk::compress(level.bytes, 0).unwrap()
            }
        }).collect()
    };

    writer.write(&mut std::fs::File::create(output).unwrap()).unwrap();
}

struct Writer<'a> {
    header: ktx2::Header,
    dfd_bytes: &'a [u8],
    kvd_bytes: &'a [u8],
    sgd_bytes: &'a [u8],
    levels: Vec<WriterLevel>,
}

impl<'a> Writer<'a> {
    fn write<T: std::io::Write>(&self, writer: &mut T) -> std::io::Result<()> {
        writer.write_all(&self.header.as_bytes()[..])?;
        
        let mut first_level_offset = ktx2::Header::LENGTH + self.levels.len() * ktx2::LevelIndex::LENGTH + self.dfd_bytes.len() + self.kvd_bytes.len() + self.sgd_bytes.len();

        for level in &self.levels {
            writer.write_all(&ktx2::LevelIndex {
                byte_offset: first_level_offset as u64,
                byte_length: level.bytes.len() as u64,
                uncompressed_byte_length: level.uncompressed_length as u64
            }.as_bytes())?;

            first_level_offset += level.bytes.len();
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