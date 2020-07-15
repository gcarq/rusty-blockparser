use std::convert::From;
use std::fmt;
use std::io::{self, Read};

use byteorder::{LittleEndian, ReadBytesExt};

use crate::blockchain::proto::ToRaw;
use crate::common::utils;

/// Variable length integer
/// Also known as CompactSize
#[derive(Debug, Clone)]
pub struct VarUint {
    pub value: u64, // Represents bytes as uint value
    buf: Vec<u8>,   // Raw bytes used for serialization (uint8 .. uint64 possible). (little endian)
}

impl VarUint {
    #[inline]
    fn new(value: u64, buf: Vec<u8>) -> VarUint {
        VarUint {
            value: value as u64,
            buf,
        }
    }

    pub fn read_from<R: Read + ?Sized>(reader: &mut R) -> io::Result<VarUint> {
        let first = reader.read_u8()?; // read first length byte
        let vint = match first {
            0x00..=0xfc => VarUint::from(first),
            0xfd => VarUint::from(reader.read_u16::<LittleEndian>()?),
            0xfe => VarUint::from(reader.read_u32::<LittleEndian>()?),
            0xff => VarUint::from(reader.read_u64::<LittleEndian>()?),
        };
        Ok(vint)
    }
}

impl From<u8> for VarUint {
    #[inline]
    fn from(value: u8) -> Self {
        VarUint::new(value as u64, vec![value])
    }
}

impl From<u16> for VarUint {
    fn from(value: u16) -> Self {
        let mut buf: Vec<u8> = Vec::with_capacity(3);
        buf.push(0xfd);
        buf.extend_from_slice(&utils::le::u16_to_array(value));
        VarUint::new(value as u64, buf)
    }
}

impl From<u32> for VarUint {
    fn from(value: u32) -> Self {
        let mut buf: Vec<u8> = Vec::with_capacity(5);
        buf.push(0xfe);
        buf.extend_from_slice(&utils::le::u32_to_array(value));
        VarUint::new(value as u64, buf)
    }
}

impl From<u64> for VarUint {
    fn from(value: u64) -> Self {
        let mut buf: Vec<u8> = Vec::with_capacity(9);
        buf.push(0xff);
        buf.extend_from_slice(&utils::le::u64_to_array(value));
        VarUint::new(value as u64, buf)
    }
}

impl ToRaw for VarUint {
    #[inline]
    fn to_bytes(&self) -> Vec<u8> {
        self.buf.clone()
    }
}

impl fmt::Display for VarUint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

#[cfg(test)]
mod tests {
    use crate::blockchain::proto::varuint::VarUint;
    use crate::blockchain::proto::ToRaw;
    use std::io;

    #[test]
    fn test_varuint_u8() {
        let v: u8 = 250;

        let test = VarUint::from(v);
        assert_eq!(250, test.value);
        assert_eq!(1, test.to_bytes().len());
        assert_eq!(vec![0xfa], test.to_bytes());
    }

    #[test]
    fn test_varuint_u16() {
        let v: u16 = 4444;

        let test = VarUint::from(v);
        assert_eq!(4444, test.value as u16);
        assert_eq!(3, test.to_bytes().len());
        assert_eq!(vec![0xfd, 0x5c, 0x11], test.to_bytes());

        let v: u16 = 515;
        let test = VarUint::from(v);
        assert_eq!(515, test.value as u16);
        assert_eq!(3, test.to_bytes().len());
        assert_eq!(vec![0xfd, 0x03, 0x02], test.to_bytes());
    }

    #[test]
    fn test_varuint_u32() {
        let v: u32 = 3333333333;

        let test = VarUint::from(v);
        assert_eq!(3333333333, test.value);
        assert_eq!(v, test.value as u32);
        assert_eq!(5, test.to_bytes().len());
        assert_eq!(vec![0xfe, 0x55, 0xa1, 0xae, 0xc6], test.to_bytes());
    }

    #[test]
    fn test_varuint_u64() {
        let v: u64 = 9000000000000000000;

        let test = VarUint::from(v);
        assert_eq!(9000000000000000000, test.value);
        assert_eq!(v, test.value as u64);
        assert_eq!(9, test.to_bytes().len());
        assert_eq!(
            vec![0xff, 0x00, 0x00, 0x84, 0xe2, 0x50, 0x6c, 0xe6, 0x7c],
            test.to_bytes()
        );
    }

    #[test]
    fn test_varuint_read() {
        let mut cursor = io::Cursor::new([0xfe, 0x55, 0xa1, 0xae, 0xc6]);
        let test = VarUint::read_from(&mut cursor);
        assert_eq!(vec![0xfe, 0x55, 0xa1, 0xae, 0xc6], test.unwrap().to_bytes());
    }
}
