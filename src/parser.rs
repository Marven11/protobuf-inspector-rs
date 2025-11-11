use crate::core::{self, read_identifier, read_value};
use crate::formatter::{foreground_bold, indent};
use crate::types::*;
use std::collections::HashMap;
use std::io::Cursor;

pub struct Parser {
    pub types: HashMap<String, HashMap<u32, (String, String)>>,
    pub native_types: HashMap<String, Box<dyn TypeHandler>>,
    pub wire_types_not_matching: bool,
}

impl Parser {
    pub fn new() -> Self {
        let mut parser = Parser {
            types: HashMap::new(),
            native_types: HashMap::new(),
            wire_types_not_matching: false,
        };
        
        parser.types.insert("message".to_string(), HashMap::new());
        parser.types.insert("root".to_string(), HashMap::new());
        
        parser.register_native_type("varint", Box::new(VarintHandler));
        parser.register_native_type("int32", Box::new(Int32Handler));
        parser.register_native_type("int64", Box::new(Int64Handler));
        parser.register_native_type("uint32", Box::new(UInt32Handler));
        parser.register_native_type("uint64", Box::new(UInt64Handler));
        parser.register_native_type("sint32", Box::new(SInt32Handler));
        parser.register_native_type("sint64", Box::new(SInt64Handler));
        parser.register_native_type("bool", Box::new(BoolHandler));
        parser.register_native_type("enum", Box::new(VarintHandler));
        parser.register_native_type("32bit", Box::new(Bit32Handler));
        parser.register_native_type("64bit", Box::new(Bit64Handler));
        parser.register_native_type("chunk", Box::new(ChunkHandler));
        parser.register_native_type("bytes", Box::new(BytesHandler));
        parser.register_native_type("string", Box::new(StringHandler));
        parser.register_native_type("message", Box::new(ChunkHandler));
        parser.register_native_type("packed", Box::new(ChunkHandler));
        parser.register_native_type("float", Box::new(FloatHandler));
        parser.register_native_type("double", Box::new(DoubleHandler));
        parser.register_native_type("fixed32", Box::new(Fixed32Handler));
        parser.register_native_type("sfixed32", Box::new(SFixed32Handler));
        parser.register_native_type("fixed64", Box::new(Fixed64Handler));
        parser.register_native_type("sfixed64", Box::new(SFixed64Handler));
        
        parser
    }
    
    fn register_native_type(&mut self, name: &str, handler: Box<dyn TypeHandler>) {
        self.native_types.insert(name.to_string(), handler);
    }
    
    pub fn match_native_type(&self, type_name: &str) -> &dyn TypeHandler {
        let type_primary = type_name.split_whitespace().next().unwrap_or(type_name);
        if let Some(handler) = self.native_types.get(type_primary) {
            handler.as_ref()
        } else {
            self.native_types.get("message").unwrap().as_ref()
        }
    }
    
    pub fn parse_message(&mut self, data: &[u8], type_name: &str) -> Result<String, core::Error> {
        self.parse_message_with_depth(data, type_name, 0)
    }
    
    fn parse_message_with_depth(&mut self, data: &[u8], type_name: &str, depth: usize) -> Result<String, core::Error> {
        if depth > 10 {
            return Ok("recursion depth exceeded".to_string());
        }
        
        let mut cursor = Cursor::new(data);
        let mut lines = Vec::new();
        let mut keys_types = HashMap::new();
        
        while let Some((key, wire_type)) = self.read_next_identifier(&mut cursor)? {
            let line = self.process_field(&mut cursor, key, wire_type, type_name, depth, &mut keys_types)?;
            if let Some(line) = line {
                lines.push(line);
            }
        }
        
        if lines.is_empty() {
            lines.push("empty".to_string());
        }
        
        Ok(format!("{}:\n{}", type_name, indent(&lines.join("\n"), None)))
    }
    
    fn read_next_identifier(&self, cursor: &mut Cursor<&[u8]>) -> Result<Option<(u32, u8)>, core::Error> {
        match read_identifier(cursor) {
            Ok(Some((key, wire_type))) => Ok(Some((key, wire_type))),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }
    
    fn process_field(
        &mut self,
        cursor: &mut Cursor<&[u8]>,
        key: u32,
        wire_type: u8,
        type_name: &str,
        depth: usize,
        keys_types: &mut HashMap<u32, u8>,
    ) -> Result<Option<String>, core::Error> {
        // 处理group类型
        if wire_type == 3 || wire_type == 4 {
            return self.handle_group_type(key, wire_type);
        }
        
        // 读取值数据
        let value_data = self.read_field_value(cursor, wire_type)?;
        
        // 检查线类型一致性
        self.check_wire_type_consistency(key, wire_type, keys_types);
        
        // 解析字段
        let parsed_line = self.parse_field_value(key, wire_type, type_name, &value_data, depth)?;
        
        Ok(Some(parsed_line))
    }
    
    fn handle_group_type(&self, key: u32, wire_type: u8) -> Result<Option<String>, core::Error> {
        let group_type = if wire_type == 3 { "startgroup" } else { "endgroup" };
        let line = format!("{} <{}> = group (end {})", 
            foreground_bold(4, &key.to_string()), 
            group_type, 
            foreground_bold(4, &key.to_string())
        );
        Ok(Some(line))
    }
    
    fn read_field_value(&self, cursor: &mut Cursor<&[u8]>, wire_type: u8) -> Result<Vec<u8>, core::Error> {
        match read_value(cursor, wire_type) {
            Ok(Some(data)) => Ok(data),
            Ok(None) => Err(core::Error::Eof),
            Err(e) => Err(e),
        }
    }
    
    fn check_wire_type_consistency(&mut self, key: u32, wire_type: u8, keys_types: &mut HashMap<u32, u8>) {
        if let Some(&existing_type) = keys_types.get(&key) {
            if existing_type != wire_type {
                self.wire_types_not_matching = true;
            }
        }
        keys_types.insert(key, wire_type);
    }
    
    fn parse_field_value(
        &mut self,
        key: u32,
        wire_type: u8,
        type_name: &str,
        value_data: &[u8],
        depth: usize,
    ) -> Result<String, core::Error> {
        let (field_type, field_name) = self.get_field_type_info(type_name, key);
        let actual_type = if field_type == "message" {
            self.get_wire_type_name(wire_type)
        } else {
            &field_type
        };
        
        // 检查类型处理器的线类型匹配
        self.check_handler_wire_type_match(actual_type, wire_type, &field_type);
        
        // 解析值
        let mut parsed_value = self.parse_value_with_type(actual_type, value_data)?;
        
        // 尝试解析嵌套消息
        if actual_type == "chunk" && self.should_try_nested_parse(value_data) {
            if let Ok(nested_msg) = self.try_parse_nested_message(value_data, depth) {
                parsed_value = nested_msg;
            }
        }
        
        let display_name = if field_name.is_empty() {
            format!("<{}>", actual_type)
        } else {
            field_name
        };
        
        Ok(format!("{} {} = {}", foreground_bold(4, &key.to_string()), display_name, parsed_value))
    }
    
    fn check_handler_wire_type_match(&mut self, actual_type: &str, wire_type: u8, field_type: &str) {
        let wire_type_enum = match WireType::from_u8(wire_type) {
            Some(wt) => wt,
            None => return,
        };
        
        let handler_wire_type = self.match_native_type(actual_type).wire_type();
        
        if handler_wire_type != wire_type_enum && field_type != "message" {
            self.wire_types_not_matching = true;
        }
    }
    
    fn parse_value_with_type(&self, actual_type: &str, value_data: &[u8]) -> Result<String, core::Error> {
        self.match_native_type(actual_type)
            .parse(value_data, actual_type)
            .map_err(|e| format!("ERROR: {:?}", e))
            .map_err(|_| core::Error::InvalidVarint)
    }
    
    fn should_try_nested_parse(&self, value_data: &[u8]) -> bool {
        value_data.len() > 2 && value_data.len() < 100
    }
    
    fn try_parse_nested_message(&mut self, value_data: &[u8], depth: usize) -> Result<String, core::Error> {
        let mut test_cursor = Cursor::new(value_data);
        if let Ok(Some((_, wire))) = read_identifier(&mut test_cursor) {
            if wire == 0 || wire == 1 || wire == 2 || wire == 5 {
                let msg = self.parse_message_with_depth(value_data, "message", depth + 1)?;
                // 只有当解析结果看起来像有效的protobuf消息时才使用
                if !msg.contains("ERROR") && !msg.contains("empty") && 
                   msg.lines().count() <= 3 && msg.contains(":") {
                    return Ok(msg);
                }
            }
        }
        Err(core::Error::InvalidVarint)
    }
    
    fn get_field_type_info(&self, type_name: &str, key: u32) -> (String, String) {
        if let Some(type_map) = self.types.get(type_name) {
            if let Some((type_str, field_str)) = type_map.get(&key) {
                return (type_str.clone(), field_str.clone());
            }
        }
        ("message".to_string(), String::new())
    }
    
    fn get_wire_type_name(&self, wire_type: u8) -> &'static str {
        match wire_type {
            0 => "varint",
            1 => "64bit",
            2 => "chunk",
            3 => "startgroup",
            4 => "endgroup",
            5 => "32bit",
            _ => "message",
        }
    }
}
