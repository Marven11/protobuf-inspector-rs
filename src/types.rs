use crate::core::{parse_varint_bytes, zigzag_decode};
use crate::formatter::{foreground, foreground_bold};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WireType {
    Varint = 0,
    Bit64 = 1,
    Chunk = 2,
    StartGroup = 3,
    EndGroup = 4,
    Bit32 = 5,
}

impl WireType {
    pub fn from_u8(n: u8) -> Option<Self> {
        match n {
            0 => Some(WireType::Varint),
            1 => Some(WireType::Bit64),
            2 => Some(WireType::Chunk),
            3 => Some(WireType::StartGroup),
            4 => Some(WireType::EndGroup),
            5 => Some(WireType::Bit32),
            _ => None,
        }
    }
}

pub trait TypeHandler {
    fn parse(&self, data: &[u8], type_name: &str) -> Result<String, crate::core::Error>;
    fn wire_type(&self) -> WireType;
}

pub struct VarintHandler;
pub struct Bit32Handler;
pub struct Bit64Handler;
pub struct ChunkHandler;

impl TypeHandler for VarintHandler {
    fn parse(&self, data: &[u8], _type_name: &str) -> Result<String, crate::core::Error> {
        let val = parse_varint_bytes(data)?;
        Ok(format!("{}", foreground_bold(3, &val.to_string())))
    }
    
    fn wire_type(&self) -> WireType {
        WireType::Varint
    }
}

impl TypeHandler for Bit32Handler {
    fn parse(&self, data: &[u8], _type_name: &str) -> Result<String, crate::core::Error> {
        if data.len() != 4 {
            return Err(crate::core::Error::Eof);
        }
        let signed = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let unsigned = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let floating = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        Ok(format!("0x{:08X} / {} / {:+#?}", unsigned, signed, floating))
    }
    
    fn wire_type(&self) -> WireType {
        WireType::Bit32
    }
}

impl TypeHandler for Bit64Handler {
    fn parse(&self, data: &[u8], _type_name: &str) -> Result<String, crate::core::Error> {
        if data.len() != 8 {
            return Err(crate::core::Error::Eof);
        }
        let signed = i64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7]
        ]);
        let unsigned = u64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7]
        ]);
        let floating = f64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7]
        ]);
        Ok(format!("0x{:016X} / {} / {:+#?}", unsigned, signed, floating))
    }
    
    fn wire_type(&self) -> WireType {
        WireType::Bit64
    }
}

impl TypeHandler for ChunkHandler {
    fn parse(&self, data: &[u8], _type_name: &str) -> Result<String, crate::core::Error> {
        if data.is_empty() {
            return Ok("empty chunk".to_string());
        }
        
        // 首先尝试作为字符串显示
        if let Ok(s) = std::str::from_utf8(data) {
            if is_probable_string(s) {
                return Ok(format!("{}", foreground(2, &format!("\"{}\"", s))));
            }
        }
        
        // 使用增强的猜测逻辑决定如何显示所有chunk数据
        match crate::guesser::guess_is_message(data) {
            Ok(true) => {
                // 如果猜测为消息，显示为嵌套消息格式
                Ok(format!("message ({} bytes)", data.len()))
            }
            Ok(false) => {
                // 如果猜测不是消息，显示为bytes
                Ok(format!("bytes ({:?})", data))
            }
            Err(_) => {
                // 猜测过程中出错，保守显示为bytes
                Ok(format!("bytes ({:?})", data))
            }
        }
    }
    
    fn wire_type(&self) -> WireType {
        WireType::Chunk
    }
}

fn is_probable_string(s: &str) -> bool {
    let total = s.len();
    if total == 0 {
        return false;
    }
    
    let mut controlchars = 0;
    let mut printable = 0;
    
    for c in s.chars() {
        let c_val = c as u32;
        // 控制字符（除了常见的空白字符）
        if c_val < 0x20 && c != '\n' && c != '\t' && c != '\r' || c_val == 0x7F {
            controlchars += 1;
        }
        // 可打印字符：字母、数字、标点、中文等
        if c.is_alphanumeric() || c.is_whitespace() || 
           c_val >= 0x4E00 && c_val <= 0x9FFF || // 常用汉字
           c_val >= 0x3400 && c_val <= 0x4DBF || // 扩展汉字
           c_val >= 0x2000 && c_val <= 0x206F || // 常用标点
           c_val >= 0x3000 && c_val <= 0x303F {  // CJK符号和标点
            printable += 1;
        }
    }
    
    // 允许少量控制字符
    if controlchars as f64 / total as f64 > 0.05 {
        return false;
    }
    // 至少80%的字符应该是可打印的
    if (printable as f64) / (total as f64) < 0.8 {
        return false;
    }
    true
}

pub struct SInt32Handler;
pub struct SInt64Handler;
pub struct Int32Handler;
pub struct Int64Handler;
pub struct UInt32Handler;
pub struct UInt64Handler;
pub struct BoolHandler;
pub struct StringHandler;
pub struct BytesHandler;
pub struct FloatHandler;
pub struct DoubleHandler;
pub struct Fixed32Handler;
pub struct SFixed32Handler;
pub struct Fixed64Handler;
pub struct SFixed64Handler;

impl TypeHandler for SInt32Handler {
    fn parse(&self, data: &[u8], _type_name: &str) -> Result<String, crate::core::Error> {
        let val = parse_varint_bytes(data)?;
        let decoded = zigzag_decode(val);
        Ok(format!("{}", foreground_bold(3, &decoded.to_string())))
    }
    
    fn wire_type(&self) -> WireType {
        WireType::Varint
    }
}

impl TypeHandler for SInt64Handler {
    fn parse(&self, data: &[u8], _type_name: &str) -> Result<String, crate::core::Error> {
        let val = parse_varint_bytes(data)?;
        let decoded = zigzag_decode(val);
        Ok(format!("{}", foreground_bold(3, &decoded.to_string())))
    }
    
    fn wire_type(&self) -> WireType {
        WireType::Varint
    }
}

impl TypeHandler for Int32Handler {
    fn parse(&self, data: &[u8], _type_name: &str) -> Result<String, crate::core::Error> {
        let mut val = parse_varint_bytes(data)?;
        if val >= (1u64 << 63) {
            val = val.wrapping_sub(u64::MAX).wrapping_sub(1);
        }
        if val >= (1u64 << 31) && val < u64::MAX.saturating_sub(20000) {
            return Err(crate::core::Error::InvalidVarint);
        }
        Ok(format!("{}", foreground_bold(3, &((val as i64).to_string()))))
    }
    
    fn wire_type(&self) -> WireType {
        WireType::Varint
    }
}

impl TypeHandler for Int64Handler {
    fn parse(&self, data: &[u8], _type_name: &str) -> Result<String, crate::core::Error> {
        let mut val = parse_varint_bytes(data)?;
        if val >= (1u64 << 63) {
            val = val.wrapping_sub(u64::MAX).wrapping_sub(1);
        }
        Ok(format!("{}", foreground_bold(3, &((val as i64).to_string()))))
    }
    
    fn wire_type(&self) -> WireType {
        WireType::Varint
    }
}

impl TypeHandler for UInt32Handler {
    fn parse(&self, data: &[u8], _type_name: &str) -> Result<String, crate::core::Error> {
        let val = parse_varint_bytes(data)?;
        if val >= (1u64 << 32) {
            return Err(crate::core::Error::InvalidVarint);
        }
        Ok(format!("{}", foreground_bold(3, &val.to_string())))
    }
    
    fn wire_type(&self) -> WireType {
        WireType::Varint
    }
}

impl TypeHandler for UInt64Handler {
    fn parse(&self, data: &[u8], _type_name: &str) -> Result<String, crate::core::Error> {
        let val = parse_varint_bytes(data)?;
        Ok(format!("{}", foreground_bold(3, &val.to_string())))
    }
    
    fn wire_type(&self) -> WireType {
        WireType::Varint
    }
}

impl TypeHandler for BoolHandler {
    fn parse(&self, data: &[u8], _type_name: &str) -> Result<String, crate::core::Error> {
        let val = parse_varint_bytes(data)?;
        if val >= (1u64 << 1) {
            return Err(crate::core::Error::InvalidVarint);
        }
        Ok(format!("{}", foreground_bold(3, &val.to_string())))
    }
    
    fn wire_type(&self) -> WireType {
        WireType::Varint
    }
}

impl TypeHandler for StringHandler {
    fn parse(&self, data: &[u8], _type_name: &str) -> Result<String, crate::core::Error> {
        let s = std::str::from_utf8(data)
            .map_err(|_| crate::core::Error::Eof)?;
        Ok(format!("{}", foreground(2, &format!("\"{}\"", s))))
    }
    
    fn wire_type(&self) -> WireType {
        WireType::Chunk
    }
}

impl TypeHandler for BytesHandler {
    fn parse(&self, data: &[u8], _type_name: &str) -> Result<String, crate::core::Error> {
        // 显示bytes长度和hex dump
        let hex_dump = crate::formatter::hex_dump(data);
        if data.len() > 0 {
            Ok(format!("bytes ({})\n{}", data.len(), crate::formatter::indent(&hex_dump, None)))
        } else {
            Ok("bytes (0)".to_string())
        }
    }
    
    fn wire_type(&self) -> WireType {
        WireType::Chunk
    }
}

impl TypeHandler for FloatHandler {
    fn parse(&self, data: &[u8], _type_name: &str) -> Result<String, crate::core::Error> {
        if data.len() != 4 {
            return Err(crate::core::Error::Eof);
        }
        let val = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        Ok(format!("{}", foreground_bold(3, &format!("{:+#?}", val))))
    }
    
    fn wire_type(&self) -> WireType {
        WireType::Bit32
    }
}

impl TypeHandler for DoubleHandler {
    fn parse(&self, data: &[u8], _type_name: &str) -> Result<String, crate::core::Error> {
        if data.len() != 8 {
            return Err(crate::core::Error::Eof);
        }
        let val = f64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7]
        ]);
        Ok(format!("{}", foreground_bold(3, &format!("{:+#?}", val))))
    }
    
    fn wire_type(&self) -> WireType {
        WireType::Bit64
    }
}

impl TypeHandler for Fixed32Handler {
    fn parse(&self, data: &[u8], _type_name: &str) -> Result<String, crate::core::Error> {
        if data.len() != 4 {
            return Err(crate::core::Error::Eof);
        }
        let val = i32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        Ok(format!("{}", foreground_bold(3, &val.to_string())))
    }
    
    fn wire_type(&self) -> WireType {
        WireType::Bit32
    }
}

impl TypeHandler for SFixed32Handler {
    fn parse(&self, data: &[u8], _type_name: &str) -> Result<String, crate::core::Error> {
        if data.len() != 4 {
            return Err(crate::core::Error::Eof);
        }
        let val = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        Ok(format!("{}", foreground_bold(3, &val.to_string())))
    }
    
    fn wire_type(&self) -> WireType {
        WireType::Bit32
    }
}

impl TypeHandler for Fixed64Handler {
    fn parse(&self, data: &[u8], _type_name: &str) -> Result<String, crate::core::Error> {
        if data.len() != 8 {
            return Err(crate::core::Error::Eof);
        }
        let val = i64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7]
        ]);
        Ok(format!("{}", foreground_bold(3, &val.to_string())))
    }
    
    fn wire_type(&self) -> WireType {
        WireType::Bit64
    }
}

impl TypeHandler for SFixed64Handler {
    fn parse(&self, data: &[u8], _type_name: &str) -> Result<String, crate::core::Error> {
        if data.len() != 8 {
            return Err(crate::core::Error::Eof);
        }
        let val = u64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7]
        ]);
        Ok(format!("{}", foreground_bold(3, &val.to_string())))
    }
    
    fn wire_type(&self) -> WireType {
        WireType::Bit64
    }
}
