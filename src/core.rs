use std::io::{self, Read};

#[derive(Debug)]
pub enum Error {
    Eof,
    InvalidVarint,
    InvalidWireType,
}

pub fn read_varint<R: Read>(reader: &mut R) -> Result<Option<u64>, Error> {
    let mut result = 0u64;
    let mut pos = 0;
    
    loop {
        let mut buf = [0u8; 1];
        match reader.read_exact(&mut buf) {
            Ok(()) => {
                let b = buf[0];
                result |= ((b & 0x7F) as u64) << pos;
                pos += 7;
                
                if b & 0x80 == 0 {
                    if b == 0 && pos != 7 {
                        return Err(Error::InvalidVarint);
                    }
                    return Ok(Some(result));
                }
                
                if pos >= 64 {
                    return Err(Error::InvalidVarint);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                if pos == 0 {
                    return Ok(None);
                }
                return Err(Error::Eof);
            }
            Err(_) => return Err(Error::Eof),
        }
    }
}

pub fn read_identifier<R: Read>(reader: &mut R) -> Result<Option<(u32, u8)>, Error> {
    match read_varint(reader)? {
        Some(id) => {
            let key = (id >> 3) as u32;
            let wire_type = (id & 0x07) as u8;
            Ok(Some((key, wire_type)))
        }
        None => Ok(None),
    }
}

pub fn read_value<R: Read>(reader: &mut R, wire_type: u8) -> Result<Option<Vec<u8>>, Error> {
    match wire_type {
        0 => {
            let mut buf = Vec::new();
            loop {
                let mut byte = [0u8; 1];
                match reader.read_exact(&mut byte) {
                    Ok(()) => {
                        buf.push(byte[0]);
                        if byte[0] & 0x80 == 0 {
                            return Ok(Some(buf));
                        }
                    }
                    Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                        if buf.is_empty() {
                            return Ok(None);
                        }
                        return Err(Error::Eof);
                    }
                    Err(_) => return Err(Error::Eof),
                }
            }
        }
        1 => {
            let mut buf = vec![0u8; 8];
            match reader.read_exact(&mut buf) {
                Ok(()) => Ok(Some(buf)),
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
                Err(_) => Err(Error::Eof),
            }
        }
        2 => {
            let length = match read_varint(reader)? {
                Some(len) => len as usize,
                None => return Ok(None),
            };
            
            let mut buf = vec![0u8; length];
            match reader.read_exact(&mut buf) {
                Ok(()) => Ok(Some(buf)),
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
                Err(_) => Err(Error::Eof),
            }
        }
        3 | 4 => {
            Ok(Some(vec![wire_type]))
        }
        5 => {
            let mut buf = vec![0u8; 4];
            match reader.read_exact(&mut buf) {
                Ok(()) => Ok(Some(buf)),
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => Ok(None),
                Err(_) => Err(Error::Eof),
            }
        }
        _ => Err(Error::InvalidWireType),
    }
}

pub fn parse_varint_bytes(buf: &[u8]) -> Result<u64, Error> {
    let mut result = 0u64;
    let mut pos = 0;
    
    for &b in buf {
        result |= ((b & 0x7F) as u64) << pos;
        pos += 7;
        
        if b & 0x80 == 0 {
            if b == 0 && pos != 7 {
                return Err(Error::InvalidVarint);
            }
            return Ok(result);
        }
        
        if pos >= 64 {
            return Err(Error::InvalidVarint);
        }
    }
    
    Err(Error::InvalidVarint)
}

pub fn zigzag_decode(n: u64) -> i64 {
    let negative = (n & 1) != 0;
    let x = (n >> 1) as i64;
    if negative {
        -(x + 1)
    } else {
        x
    }
}
