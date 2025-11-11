pub fn foreground(color: u8, text: &str) -> String {
    format!("\x1b[3{}m{}\x1b[m", color, text)
}

pub fn bold(text: &str) -> String {
    format!("\x1b[1m{}\x1b[m", text)
}

pub fn dim(text: &str) -> String {
    format!("\x1b[2m{}\x1b[m", text)
}

pub fn foreground_color(color: u8) -> impl Fn(&str) -> String {
    move |text: &str| foreground(color, text)
}

pub fn foreground_bold(color: u8, text: &str) -> String {
    bold(&foreground(color, text))
}

pub fn indent(text: &str, indent_str: Option<&str>) -> String {
    let indent = indent_str.unwrap_or("    ");
    text.lines()
        .map(|line| {
            if line.is_empty() {
                line.to_string()
            } else {
                format!("{}{}", indent, line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn hex_dump(data: &[u8]) -> String {
    const BYTES_PER_LINE: usize = 24;
    let mut lines = Vec::new();
    let mut offset = 0;
    
    for chunk in data.chunks(BYTES_PER_LINE) {
        let hexdump: String = chunk
            .iter()
            .map(|&b| format!("{:02X}", b))
            .collect::<Vec<_>>()
            .join(" ");
        
        let padded_hexdump = if chunk.len() < BYTES_PER_LINE {
            let padding = "   ".repeat(BYTES_PER_LINE - chunk.len());
            format!("{}{}", hexdump, padding)
        } else {
            hexdump
        };
        
        let printable: String = chunk
            .iter()
            .map(|&b| {
                if b >= 0x20 && b < 0x7F {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        
        lines.push(format!("{:04x}   {}  {}", offset, padded_hexdump, printable));
        offset += chunk.len();
    }
    
    lines.join("\n")
}
