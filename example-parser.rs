use core::{convert::TryInto, option::Option};

use reader::DataReader;


// pub struct DataReader<'a> {
//     /// 读取的数据
//     pub input: InputData<'a>,
//     /// 下一次要读取的数据的索引
//     cursor: usize,
// }

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    // 我们需要在读取下一个datareader时恢复状态，需要保存EoF的元数据
    VarintEof,  // 读取VARINT变长数据时出现EoF
    Eof(usize), // 读取定长数据时出现EoF
    InvalidData,
}

pub trait MapError<T> {
    fn map_custom_err(self) -> Result<T, Error>;
}

impl<T, E> MapError<T> for Result<T, E> {
    fn map_custom_err(self) -> Result<T, Error> {
        self.map_err(|_| Error::InvalidData)
    }
}

#[repr(u8)]
#[derive(Debug, Clone, PartialEq)]
enum WireType {
    Varint = 0,
    SixtyFourBit = 1,
    Chunk = 2,
    StartGroup = 3,
    EndGroup = 4,
    ThirtyTwoBit = 5,
}

#[derive(Debug, Clone, PartialEq)]
enum TaggedData {
    ChunkMetadata { length: u64 },
    Others, // 数字和SGROUP/EGROUP标记
}

struct MessageContext {
    start: usize,
    length: Option<usize>,
}

struct ParserContext {
    messages: Vec<MessageContext>,
    // 读取上一个datareader时留下了什么错误
    error: Option<Error>,
}

struct ProtobufParser<'a> {
    reader: DataReader<'a>,
    context: ParserContext,
}

impl TryFrom<u8> for WireType {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(WireType::Varint),
            1 => Ok(WireType::SixtyFourBit),
            2 => Ok(WireType::Chunk),
            3 => Ok(WireType::StartGroup),
            4 => Ok(WireType::EndGroup),
            5 => Ok(WireType::ThirtyTwoBit),
            _ => Err(()),
        }
    }
}

impl MessageContext {
    fn new(start: usize, length: Option<usize>) -> Self {
        Self { start, length }
    }
    fn shift(&mut self, extra_length: usize) {
        self.length = self.length.map(|length| length - self.start - extra_length);
        self.start = 0;
    }
}

impl ParserContext {
    /// 准备读取下一个HTTP Body块
    fn prepare_for_next_slice(&mut self, remain_length: usize, current_error: Option<Error>) {
        if matches!(self.error, Some(Error::InvalidData)) {
            return;
        }
        self.messages.iter_mut().for_each(|msg_ctx| msg_ctx.shift(remain_length));
        self.error = current_error;
    }
}

fn read_varint(reader: &mut DataReader) -> Result<u64, Error> {
    let mut result: u64 = 0;
    let mut pos = 0;
    loop {
        let x: u8 = reader.read1().map_err(|_| Error::VarintEof)?;
        let bits = (x & 0b01111111u8) as u64;
        result |= bits << (pos * 7);
        pos += 1;
        if x & 0b10000000u8 == 0 {
            break;
        }
        if pos == 10 {
            return Err(Error::InvalidData);
        }
    }
    Ok(result)
}

fn read_tag(reader: &mut DataReader) -> Result<(u32, WireType), Error> {
    let tag_varint = read_varint(reader)?;
    let field_number: u32 = (tag_varint >> 3).try_into().map_custom_err()?;
    let wire_type = WireType::try_from((tag_varint & 0b111) as u8).map_custom_err()?;

    if field_number == 0 || (19000 <= field_number && field_number <= 19999) {
        return Err(Error::InvalidData);
    }
    Ok((field_number, wire_type))
}

fn read_tagged_data(reader: &mut DataReader) -> Result<TaggedData, Error> {
    let (_field_number, wire_type) = read_tag(reader)?;
    // [TODO]: 检查field number
    match wire_type {
        WireType::StartGroup | WireType::EndGroup => Ok(TaggedData::Others),
        WireType::Varint => {
            let _ = read_varint(reader)?;
            Ok(TaggedData::Others)
        }
        WireType::ThirtyTwoBit => {
            reader.skip(4).map_err(|_| Error::Eof(4 - reader.remains()))?;
            Ok(TaggedData::Others)
        }
        WireType::SixtyFourBit => {
            reader.skip(8).map_err(|_| Error::Eof(8 - reader.remains()))?;
            Ok(TaggedData::Others)
        }
        WireType::Chunk => {
            let length = read_varint(reader)?;
            Ok(TaggedData::ChunkMetadata { length })
        }
    }
}

fn find_next_chunk(reader: &mut DataReader, message_context: &mut MessageContext) -> Result<Option<usize>, Error> {
    while !reader.eof() {
        if let Some(message_length) = message_context.length {
            let current_length = reader.position() - message_context.start;
            if current_length == message_length {
                break; // 当前message读取完毕，没有找到chunk
            }
            if current_length > message_length {
                return Err(Error::InvalidData); // 已经读取的长度超出期望的message长度
            }
        }
        if let TaggedData::ChunkMetadata { length } = read_tagged_data(reader)? {
            // 一般来说不会出错
            let length: usize = length.try_into().map_custom_err()?;
            return Ok(Some(length));
        }
    }
    Ok(None)
}

fn guess_is_message(mut reader: DataReader<'_>) -> Result<bool, Error> {
    let mut is_ctrl_char_found = false;
    let mut weird_value_count = 0;

    for _ in 0..3 {
        let bytes: &[u8] = reader.peek(4.min(reader.remains())).map_custom_err()?;
        is_ctrl_char_found |= bytes.iter().any(|c| *c < 32 && *c != b'\n');

        let (_field_number, wire_type) = read_tag(&mut reader)?;

        // 虽然说field number一般设置为16，但是也有可能为了方便编号等原因设置为100000等大数
        // 通过field number来猜测可能是不准确的

        match wire_type {
            WireType::StartGroup | WireType::EndGroup => {}
            WireType::ThirtyTwoBit => {
                reader.skip(4).map_custom_err()?;
            }
            WireType::SixtyFourBit => {
                let data = reader.read(8).map_custom_err()?;
                if !matches!(data.last(), Some(0 | 255)) {
                    weird_value_count += 1;
                }
            }
            WireType::Chunk => {
                let length = read_varint(&mut reader)? as usize;
                if length > 100 || length == 0 {
                    weird_value_count += 1;
                }
                reader.skip(length).map_custom_err()?;
            }
            WireType::Varint => {
                read_varint(&mut reader)?;
            }
        }

        if reader.eof() {
            break;
        }
    }
    Ok(is_ctrl_char_found && weird_value_count <= 1)
}

fn read_next_token_raw<'a>(reader: &mut DataReader<'a>, parser_context: &mut ParserContext) -> Option<&'a [u8]> {
    match parser_context.error {
        None => {}
        Some(Error::VarintEof) => {
            let _ = read_varint(reader);
            parser_context.error = None;
        }
        Some(Error::Eof(length)) => match reader.skip(length) {
            Ok(_) => {
                parser_context.error = None;
            }
            Err(_) => {
                parser_context.prepare_for_next_slice(reader.remains(), Some(Error::Eof(length - reader.remains())));
                return None;
            }
        },
        Some(Error::InvalidData) => return None,
    }

    while !reader.eof() {
        let message_context = parser_context.messages.last_mut()?;
        let chunk_length = find_next_chunk(reader, message_context);
        match chunk_length {
            Ok(Some(length)) => {
                let chunk = reader.peek(length).ok()?;
                let chunk_reader = DataReader::new(chunk);

                if guess_is_message(chunk_reader).unwrap_or(false) {
                    let new_message_context = MessageContext::new(reader.position(), Some(length));
                    parser_context.messages.push(new_message_context);
                    continue;
                } else {
                    reader.skip(length).ok()?;
                    return Some(chunk);
                }
            }
            Ok(None) => {
                // message遍历完毕，没有更多chunk
                match message_context.length {
                    Some(_) => {
                        parser_context.messages.pop()?;
                        continue;
                    }
                    None => {
                        let error = if reader.eof() { None } else { Some(Error::InvalidData) };
                        parser_context.prepare_for_next_slice(reader.remains(), error);
                        return None;
                    }
                }
            }
            Err(Error::InvalidData) => {
                // 当前chunk作为message读取发生错误，实际上是一个普通bytearray
                match message_context.length {
                    Some(length) => {
                        // message_context.start来自当前reader, 一般不会出错
                        reader.seek(message_context.start).ok()?;
                        // [TODO] 这里读字符串读到一半EOF了怎么办
                        let data = reader.read(length).ok();
                        let _ = parser_context.messages.pop();
                        return data;
                    }
                    None => {
                        // 当前chunk是根message，整个payload都存在格式错误
                        parser_context.prepare_for_next_slice(reader.remains(), Some(Error::InvalidData));
                        return None;
                    }
                }
            }
            Err(e) => {
                parser_context.prepare_for_next_slice(reader.remains(), Some(e));
                return None;
            }
        }
    }
    parser_context.error = Some(Error::Eof(0));
    None
}

impl ParserContext {
    fn new() -> Self {
        Self { messages: vec![MessageContext::new(0, None)], error: None }
    }
}

impl<'a> ProtobufParser<'a> {
    fn next_token(&mut self) -> Option<&'a [u8]> {
        read_next_token_raw(&mut self.reader, &mut self.context)
    }
}

impl<'a> Iterator for ProtobufParser<'a> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<&'a [u8]> {
        self.next_token()
    }
}

#[cfg(test)]
const PROTOBUF_EXAMPLE: &[u8] = b"\x08\x8f\x81\xeb\xcf\xe0*\x12\x08kotlin46:\x05\x00\x01\x03\x04\x07B\x00H\xfa\x01U\x00\x00HCr\n\n\x08POKECOINr\x0c\n\x08STARDUST\x10d";

#[cfg(test)]
macro_rules! assert_datareader {
    ($fn:expr, $bytes:expr, $expected:pat) => {
        let bytes = $bytes;
        let mut data = DataReader::new($bytes);
        let result = $fn(&mut data);
        assert!(matches!(result, $expected), "{result:?} {bytes:?}");
    };
    ($fn:expr, $bytes:expr, $expected:pat, $($arg:expr),*) => {
        let bytes = $bytes;
        let mut data = DataReader::new($bytes);
        let result =$fn(&mut data, $($arg),*);
        assert!(matches!(result, $expected), "{result:?} {bytes:?}");
    };
}

#[test]
fn test_read_varint() {
    assert_datareader!(read_varint, b"\x01", Ok(1));
    assert_datareader!(read_varint, b"\x96\x01", Ok(150));
    assert_datareader!(read_varint, b"\xb5\x81\xd5\xc8\x06", Ok(1763000501));
}

#[test]
fn test_read_tag() {
    assert_datareader!(read_tag, b"\x0a", Ok((1, WireType::Chunk)));
    assert_datareader!(read_tag, b"\x18", Ok((3, WireType::Varint)));
}

#[test]
fn test_read_tagged_data() {
    assert_datareader!(read_tagged_data, b"\x18\xb5\x81\xd5\xc8\x06", Ok(TaggedData::Others));
    assert_datareader!(
        read_tagged_data,
        b"\x0a\x07\x53\x55\x43\x43\x45\x53\x53",
        Ok(TaggedData::ChunkMetadata { length: 7 })
    );
    assert_datareader!(read_tagged_data, b"\x0a\x00", Ok(TaggedData::ChunkMetadata { length: 0 }));
    assert_datareader!(read_tagged_data, &PROTOBUF_EXAMPLE[..7], Ok(TaggedData::Others));
}

#[test]
fn test_guess_is_message() {
    assert_eq!(guess_is_message(DataReader::new(b"\x0a\x08POKECOIN")), Ok(true));
    assert_eq!(guess_is_message(DataReader::new(b"POKECOIN")), Ok(false));
}

#[test]
fn test_find_next_chunk() {
    let mut message_context = MessageContext::new(0, None);
    assert_datareader!(find_next_chunk, b"\x0a\x07\x53\x55\x43\x43\x45\x53\x53", Ok(Some(7)), &mut message_context);

    // POKECOIN竟然是格式正确的message
    // assert_datareader!(find_next_chunk, b"POKECOIN", Err(Error::InvalidData), &mut message_context);

    assert_datareader!(find_next_chunk, PROTOBUF_EXAMPLE, Ok(Some(8)), &mut message_context);
    // 在读取完一个TaggedData时EoF
    assert_datareader!(find_next_chunk, &PROTOBUF_EXAMPLE[..3], Err(Error::VarintEof), &mut message_context);
    assert_datareader!(find_next_chunk, &PROTOBUF_EXAMPLE[..6], Err(Error::VarintEof), &mut message_context);
    // 在读取完一个TaggedData之后EoF: 报告当前message中找不到chunk
    assert_datareader!(find_next_chunk, &PROTOBUF_EXAMPLE[..7], Ok(None), &mut message_context);
}

#[test]
fn test_parser() {
    let mut parser = ProtobufParser { reader: DataReader::new(PROTOBUF_EXAMPLE), context: ParserContext::new() };
    assert_eq!(parser.next_token(), Some(b"kotlin46" as &[u8]));
    assert_eq!(parser.next_token(), Some(b"\x00\x01\x03\x04\x07" as &[u8]));
    assert_eq!(parser.next_token(), Some(b"" as &[u8]));

    // [TODO]: 有以下问题：
    // 1. b"POKECOIN"是大致合法的message，作为message解析时为{10: 79, 9:SGROUP, 8: 8.443537e08i32}
    // 2. message {1: "POKECOIN"}同样也是b"\x0a\x08POKECOIN"，也就是b"\n\x08POKECOIN"

    // 因此类似b"\x0a\x08POKECOIN"的payload有三种合法的解析结果
    // {1: {10: 79, 9:SGROUP, 8: 8.443537e08i32} }
    // {1: "POKECOIN"}
    // b"\x0a\x08POKECOIN"

    assert_eq!(parser.next_token(), Some(b"POKECOIN" as &[u8]));
    assert_eq!(parser.next_token(), Some(b"STARDUST" as &[u8]));
    assert_eq!(parser.next_token(), None);

    // let result: Vec<&[u8]> = parser.into_iter().collect();
    // let expected: &[&[u8]] = &[b"kotlin46", b"\x00\x01\x03\x04\x07", b"POKECOIN", b"STARDUST"];
    // assert_eq!(result.as_slice(), expected);
}
