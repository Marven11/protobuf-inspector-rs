use std::io::Cursor;
use crate::core::{read_identifier, read_value, parse_varint_bytes};

#[derive(Debug, Clone, PartialEq)]
pub enum GuesserError {
    Eof,
    InvalidData,
}

/// 猜测数据块是否为protobuf消息
pub fn guess_is_message(data: &[u8]) -> Result<bool, GuesserError> {
    let mut cursor = Cursor::new(data);
    let mut weird_value_count = 0;
    let mut valid_fields_found = 0;

    for _ in 0..3 {

        // 读取标识符
        let (field_number, wire_type) = match read_identifier(&mut cursor) {
            Ok(Some((key, wt))) => (key, wt),
            Ok(None) => break,
            Err(_) => return Err(GuesserError::InvalidData),
        };

        // 检查field number范围
        if field_number == 0 || (19000 <= field_number && field_number <= 19999) {
            return Err(GuesserError::InvalidData);
        }

        valid_fields_found += 1;

        // 根据wire type处理数据
        match wire_type {
            3 | 4 => { // StartGroup/EndGroup
                // 不增加异常计数
            }
            5 => { // 32bit
                match read_value(&mut cursor, wire_type) {
                    Ok(Some(_)) => {},
                    _ => return Err(GuesserError::Eof),
                }
            }
            1 => { // 64bit
                match read_value(&mut cursor, wire_type) {
                    Ok(Some(value_data)) => {
                        // 检查64位数据的最后字节是否为0或255
                        if !matches!(value_data.last(), Some(0 | 255)) {
                            weird_value_count += 1;
                        }
                    }
                    _ => return Err(GuesserError::Eof),
                }
            }
            2 => { // Chunk
                // 读取chunk长度
                let length = match read_value(&mut cursor, wire_type) {
                    Ok(Some(value_data)) => {
                        match parse_varint_bytes(&value_data) {
                            Ok(len) => len as usize,
                            Err(_) => return Err(GuesserError::InvalidData),
                        }
                    }
                    _ => return Err(GuesserError::Eof),
                };
                
                // 放宽chunk长度检查，允许更大的chunk
                if length > 500 || length == 0 {
                    weird_value_count += 1;
                }

                // 跳过chunk数据
                if cursor.position() as usize + length > data.len() {
                    return Err(GuesserError::Eof);
                }
                cursor.set_position(cursor.position() + length as u64);
            }
            0 => { // Varint
                match read_value(&mut cursor, wire_type) {
                    Ok(Some(value_data)) => {
                        let _ = parse_varint_bytes(&value_data)?;
                    }
                    _ => return Err(GuesserError::Eof),
                }
            }
            _ => return Err(GuesserError::InvalidData),
        }

        if cursor.position() as usize >= data.len() {
            break;
        }
    }

    // 放宽判断条件：如果至少找到一个有效字段且异常值不多，就认为是消息
    Ok(valid_fields_found > 0 && weird_value_count <= 1)
}

impl From<crate::core::Error> for GuesserError {
    fn from(_: crate::core::Error) -> Self {
        GuesserError::InvalidData
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guess_is_message() {
        // 有效的protobuf消息
        assert_eq!(guess_is_message(b"\x0a\x08POKECOIN"), Ok(true));
        
        // 纯字符串
        assert_eq!(guess_is_message(b"POKECOIN"), Ok(false));
        
        // 空数据
        assert_eq!(guess_is_message(b""), Ok(false));
        
        // 无效的varint
        assert_eq!(guess_is_message(b"\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff"), Ok(false));
    }
}