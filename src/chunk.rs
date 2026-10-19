use std::{error::Error, fmt};

#[derive(Debug)]
pub struct Chunk {
    header: Header,
    prototype: Prototype,
}

#[derive(Debug)]
pub enum ChunkError {
    NotImplemented,
    InvalidPlatform,
    VersionMismatch,
    InvalidFormat,
}

const HEADER_SIZE: usize = 12;

#[derive(Debug)]
pub struct Header {
    signature: [u8; 4],
    version: u8,
    format: u8,
    little_endian: bool,
    int_size: u8,
    size_t_size: u8,
    instruction_size: u8,
    number_size: u8,
    integral_numbers: bool,
}

#[derive(Debug)]
enum Constant {
    Nil,
    Boolean(bool),
    Number(f64),
    String(Option<String>),
}

#[derive(Debug)]
struct Local {
    name: Option<String>,
    start_pc: i32,
    end_pc: i32,
}

#[derive(Debug)]
pub struct Prototype {
    source: Option<String>,
    line_defined: i32,
    last_line_defined: i32,
    num_upvalues: u8,
    num_parameters: u8,
    is_vararg: u8,
    max_stack_size: u8,
    code: Vec<u32>,
    constants: Vec<Constant>,
    prototypes: Vec<Prototype>,
    line_info: Vec<i32>,
    locals: Vec<Local>,
    upvalues: Vec<Option<String>>,
}

// ========== Reader helper ==========

struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], ChunkError> {
        let value = self.bytes.get(..n).ok_or(ChunkError::InvalidFormat)?;
        self.bytes = &self.bytes[n..];
        Ok(value)
    }

    fn read_8(&mut self) -> Result<u8, ChunkError> {
        let raw: [u8; 1] = self
            .take(1)?
            .try_into()
            .map_err(|_| ChunkError::InvalidFormat)?;
        Ok(raw[0])
    }

    fn read_32<T>(&mut self, decode: impl FnOnce([u8; 4]) -> T) -> Result<T, ChunkError> {
        let raw: [u8; 4] = self
            .take(4)?
            .try_into()
            .map_err(|_| ChunkError::InvalidFormat)?;
        Ok(decode(raw))
    }

    fn read_64<T>(&mut self, decode: impl FnOnce([u8; 8]) -> T) -> Result<T, ChunkError> {
        let raw: [u8; 8] = self
            .take(8)?
            .try_into()
            .map_err(|_| ChunkError::InvalidFormat)?;
        Ok(decode(raw))
    }

    fn read_string(&mut self) -> Result<Option<String>, ChunkError> {
        let size = self.read_32(u32::from_le_bytes)?;
        if size == 0 {
            return Ok(None);
        }
        let str_bytes = self.take(size as usize)?;
        if str_bytes.last() != Some(&0) {
            return Err(ChunkError::InvalidFormat);
        }
        let string = String::from_utf8(str_bytes[..str_bytes.len() - 1].to_vec())
            .map_err(|_| ChunkError::InvalidFormat)?;
        Ok(Some(string))
    }

    fn read_vec<T>(
        &mut self,
        read_element: impl Fn(&mut Self) -> Result<T, ChunkError>,
    ) -> Result<Vec<T>, ChunkError> {
        let size = self.read_32(i32::from_le_bytes)?;
        if size < 0 {
            return Err(ChunkError::InvalidFormat);
        }
        let mut vec = Vec::with_capacity(size as usize);
        for _ in 0..size {
            vec.push(read_element(self)?);
        }
        Ok(vec)
    }
}

// ========== Implementation ==========

impl fmt::Display for ChunkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChunkError::NotImplemented => write!(f, "Not implemented"),
            ChunkError::InvalidPlatform => write!(f, "Invalid platform"),
            ChunkError::VersionMismatch => write!(f, "Version mismatch"),
            ChunkError::InvalidFormat => write!(f, "Invalid format"),
        }
    }
}

impl Error for ChunkError {}

impl TryFrom<&[u8]> for Header {
    type Error = ChunkError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() < HEADER_SIZE {
            return Err(ChunkError::InvalidFormat);
        }

        let signature = [bytes[0], bytes[1], bytes[2], bytes[3]];
        let version = bytes[4];
        let format = bytes[5];
        let little_endian = bytes[6] != 0;
        let int_size = bytes[7];
        let size_t_size = bytes[8];
        let instruction_size = bytes[9];
        let number_size = bytes[10];
        let integral_numbers = bytes[11] != 0;

        Ok(Self {
            signature,
            version,
            format,
            little_endian,
            int_size,
            size_t_size,
            instruction_size,
            number_size,
            integral_numbers,
        })
    }
}

impl TryFrom<&[u8]> for Prototype {
    type Error = ChunkError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Self::read_from(&mut Reader::new(bytes))
    }
}

impl Prototype {
    fn read_from(reader: &mut Reader<'_>) -> Result<Self, ChunkError> {
        let source = reader.read_string()?;
        let line_defined = reader.read_32(i32::from_le_bytes)?;
        let last_line_defined = reader.read_32(i32::from_le_bytes)?;
        let num_upvalues = reader.read_8()?;
        let num_parameters = reader.read_8()?;
        let is_vararg = reader.read_8()?;
        let max_stack_size = reader.read_8()?;
        let code = reader.read_vec(|r| r.read_32(u32::from_le_bytes))?;
        let constants = reader.read_vec(|r| {
            let type_byte = r.read_8()?;
            match type_byte {
                0 => Ok(Constant::Nil),
                1 => {
                    let value = r.read_8()?;
                    Ok(Constant::Boolean(value != 0))
                }
                3 => {
                    let value = r.read_64(f64::from_le_bytes)?;
                    Ok(Constant::Number(value))
                }
                4 => {
                    let value = r.read_string()?;
                    Ok(Constant::String(value))
                }
                _ => Err(ChunkError::InvalidFormat),
            }
        })?;
        let prototypes = reader.read_vec(Prototype::read_from)?;
        let line_info = reader.read_vec(|r| r.read_32(i32::from_le_bytes))?;
        let locals = reader.read_vec(|r| {
            let name = r.read_string()?;
            let start_pc = r.read_32(i32::from_le_bytes)?;
            let end_pc = r.read_32(i32::from_le_bytes)?;
            Ok(Local {
                name,
                start_pc,
                end_pc,
            })
        })?;
        let upvalues = reader.read_vec(|r| r.read_string())?;

        Ok(Self {
            source,
            line_defined,
            last_line_defined,
            num_upvalues,
            num_parameters,
            is_vararg,
            max_stack_size,
            code,
            constants,
            prototypes,
            line_info,
            locals,
            upvalues,
        })
    }
}

impl Chunk {
    // Note: will be tested by integration tests
    pub fn parse(bytes: &[u8]) -> Result<Self, ChunkError> {
        if bytes.is_empty() {
            return Err(ChunkError::InvalidFormat);
        }
        let header = Header::try_from(bytes)?;
        if header.signature != [0x1B, 0x4C, 0x75, 0x61] || header.format != 0x00 {
            return Err(ChunkError::InvalidFormat);
        }
        if header.version != 0x51 {
            return Err(ChunkError::VersionMismatch);
        }
        if !header.little_endian
            || header.int_size != 4
            || header.size_t_size != 4
            || header.instruction_size != 4
            || header.number_size != 8
            || header.integral_numbers
        {
            return Err(ChunkError::InvalidPlatform);
        }
        let prototype = Prototype::try_from(bytes[HEADER_SIZE..].as_ref())?;
        Ok(Self { header, prototype })
    }
}

// ========== Tests ==========

#[cfg(test)]
mod tests {
    use super::*;

     #[test]
    fn reader_take_advances_only_when_bytes_are_available() {
        let mut reader = Reader::new(&[1, 2, 3]);

        assert_eq!(reader.take(2).unwrap(), &[1, 2]);
        assert!(matches!(reader.take(2), Err(ChunkError::InvalidFormat)));
        assert_eq!(reader.take(1).unwrap(), &[3]);
        assert_eq!(reader.take(0).unwrap(), &[]);
    }

    #[test]
    fn reader_decodes_fixed_width_values() {
        let mut bytes = vec![
            0x12, 0xEF, 0xCD, 0xAB, 0x90, 0x78, 0x56, 0x34, 0x12, 0, 0, 0, 0,
        ];
        bytes.extend_from_slice(&1.0f64.to_le_bytes());
        let mut reader = Reader::new(&bytes);

        assert_eq!(reader.read_8().unwrap(), 0x12);
        assert_eq!(reader.read_32(u32::from_le_bytes).unwrap(), 0x90AB_CDEF);
        assert_eq!(
            reader.read_64(u64::from_le_bytes).unwrap(),
            0x0000_0000_1234_5678
        );
        assert_eq!(reader.read_64(f64::from_le_bytes).unwrap(), 1.0);
        assert!(matches!(reader.read_8(), Err(ChunkError::InvalidFormat)));
    }

    #[test]
    fn reader_decodes_strings_and_rejects_invalid_strings() {
        let bytes = [0, 0, 0, 0, 2, 0, 0, 0, b'x', 0];
        let mut reader = Reader::new(&bytes);

        assert_eq!(reader.read_string().unwrap(), None);
        assert_eq!(reader.read_string().unwrap().as_deref(), Some("x"));

        for bytes in [
            &[1, 0, 0, 0, b'x'][..],
            &[2, 0, 0, 0, 0xFF, 0],
            &[2, 0, 0, 0, b'x'],
        ] {
            let mut reader = Reader::new(bytes);
            assert!(matches!(
                reader.read_string(),
                Err(ChunkError::InvalidFormat)
            ));
        }
    }

    #[test]
    fn reader_decodes_vectors_and_rejects_invalid_lengths_or_elements() {
        let bytes = [2, 0, 0, 0, 7, 8];
        let mut reader = Reader::new(&bytes);
        assert_eq!(reader.read_vec(|reader| reader.read_8()).unwrap(), [7, 8]);

        let negative_length = (-1i32).to_le_bytes();
        let mut reader = Reader::new(&negative_length);
        assert!(matches!(
            reader.read_vec(|reader| reader.read_8()),
            Err(ChunkError::InvalidFormat)
        ));

        let truncated_element = [1, 0, 0, 0];
        let mut reader = Reader::new(&truncated_element);
        assert!(matches!(
            reader.read_vec(|reader| reader.read_32(u32::from_le_bytes)),
            Err(ChunkError::InvalidFormat)
        ));
    }

    #[test]
    fn prototype_decodes_constants_and_locals() {
        let mut bytes = vec![0; 12];
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());

        bytes.extend_from_slice(&4i32.to_le_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&[1, 1]);
        bytes.push(3);
        bytes.extend_from_slice(&1.5f64.to_le_bytes());
        bytes.push(4);
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(b"x\0");

        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&1i32.to_le_bytes());
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(b"v\0");
        bytes.extend_from_slice(&3i32.to_le_bytes());
        bytes.extend_from_slice(&7i32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());

        let prototype = Prototype::try_from(bytes.as_slice()).unwrap();

        assert_eq!(prototype.constants.len(), 4);
        assert!(matches!(prototype.constants[0], Constant::Nil));
        assert!(matches!(prototype.constants[1], Constant::Boolean(true)));
        assert!(matches!(prototype.constants[2], Constant::Number(value) if value == 1.5));
        assert!(matches!(
            &prototype.constants[3],
            Constant::String(Some(value)) if value == "x"
        ));

        assert_eq!(prototype.locals.len(), 1);
        assert_eq!(prototype.locals[0].name.as_deref(), Some("v"));
        assert_eq!(prototype.locals[0].start_pc, 3);
        assert_eq!(prototype.locals[0].end_pc, 7);
    }

    #[test]
    fn header_decodes_its_fields() {
        let bytes = [0x1B, 0x4C, 0x75, 0x61, 0x51, 0x02, 0x01, 4, 4, 4, 8, 0];
        let header = Header::try_from(&bytes[..]).unwrap();

        assert_eq!(header.signature, [0x1B, 0x4C, 0x75, 0x61]);
        assert_eq!(header.version, 0x51);
        assert_eq!(header.format, 0x02);
        assert!(header.little_endian);
        assert_eq!(
            (
                header.int_size,
                header.size_t_size,
                header.instruction_size,
                header.number_size,
            ),
            (4, 4, 4, 8)
        );
        assert!(!header.integral_numbers);
    }

    #[test]
    fn header_rejects_insufficient_input() {
        assert!(matches!(
            Header::try_from(&[0; HEADER_SIZE - 1][..]),
            Err(ChunkError::InvalidFormat)
        ));
    }

    #[test]
    fn chunk_errors_have_display_messages() {
        assert_eq!(ChunkError::NotImplemented.to_string(), "Not implemented");
        assert_eq!(ChunkError::InvalidPlatform.to_string(), "Invalid platform");
        assert_eq!(ChunkError::VersionMismatch.to_string(), "Version mismatch");
        assert_eq!(ChunkError::InvalidFormat.to_string(), "Invalid format");
    }
}
