use std::collections::HashMap;

use tracing::trace;

const FILE_MAGIC: &[u8] = b"CSB ";
const LITTLE_ENDIAN_MARK: &[u8] = b"\xFF\xFE";
const STRING_MAGIC: &[u8] = b"STRP";
const LINE_POOL_MAGIC: &[u8] = b"LNP ";
const LINE_TABLE_MAGIC: &[u8] = b"LNT ";
const HEADER_SIZE: usize = 24;

pub fn parse(bytes: &[u8]) -> Result<Vec<Vec<String>>, String> {
    if bytes.len() < HEADER_SIZE || !bytes.starts_with(FILE_MAGIC) {
        return Err(String::from("Missing CSB signature"));
    }

    if bytes.get(4..6) != Some(LITTLE_ENDIAN_MARK) {
        return Err(String::from("Only little endian CSB files are supported"));
    }

    let total_fields = read_u32(bytes, 12)? as usize;
    let total_lines = read_u32(bytes, 20)? as usize;

    let (strings, string_end) = read_string_pool(bytes, HEADER_SIZE)?;
    let (pool, pool_end) = read_line_pool(bytes, string_end, &strings)?;
    let lines = read_line_table(bytes, pool_end, &pool)?;

    if lines.len() != total_lines {
        return Err(format!("Expected {total_lines} lines but found {}", lines.len()));
    }

    let field_count: usize = lines.iter().map(Vec::len).sum();
    if field_count != total_fields {
        return Err(format!("Expected {total_fields} fields but found {field_count}"));
    }

    trace!(lines = total_lines, fields = total_fields, "Parsed CSB table");

    Ok(lines)
}

pub fn render(lines: &[Vec<String>], separator: &str) -> String {
    let mut output = String::new();

    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        output.push_str(&line.join(separator));
    }

    output
}

fn read_string_pool(bytes: &[u8], start: usize) -> Result<(HashMap<usize, String>, usize), String> {
    let end = read_block_end(bytes, start, STRING_MAGIC)?;
    let count = read_u64(bytes, start + 8)? as usize;

    let mut strings = HashMap::with_capacity(count);
    let mut cursor = start + 16;

    for _ in 0..count {
        let position = cursor;
        let terminator = bytes
            .get(cursor..)
            .and_then(|tail| tail.iter().position(|byte| *byte == 0))
            .ok_or("String pool is missing a terminator")?;

        strings.insert(
            position,
            String::from_utf8_lossy(&bytes[cursor..cursor + terminator]).into_owned(),
        );
        cursor += terminator + 1;
    }

    Ok((strings, end))
}

fn read_line_pool(
    bytes: &[u8],
    start: usize,
    strings: &HashMap<usize, String>,
) -> Result<(HashMap<usize, Vec<String>>, usize), String> {
    let end = read_block_end(bytes, start, LINE_POOL_MAGIC)?;
    let count = read_u64(bytes, start + 8)? as usize;

    let mut pool = HashMap::with_capacity(count);
    let mut cursor = start + 16;

    for _ in 0..count {
        let position = cursor;
        let column_count = read_u64(bytes, cursor)? as usize;
        let line_start = read_u64(bytes, cursor + 8)? as usize;
        cursor = line_start + column_count * 8;

        let mut line = Vec::with_capacity(column_count);

        for column in 0..column_count {
            let string_position = read_u64(bytes, line_start + column * 8)? as usize;
            let value = strings
                .get(&string_position)
                .ok_or("Line pool references an unknown string")?;

            line.push(value.clone());
        }

        pool.insert(position, line);
    }

    Ok((pool, end))
}

fn read_line_table(bytes: &[u8], start: usize, pool: &HashMap<usize, Vec<String>>) -> Result<Vec<Vec<String>>, String> {
    read_block_end(bytes, start, LINE_TABLE_MAGIC)?;
    let count = read_u64(bytes, start + 8)? as usize;

    let mut lines = Vec::with_capacity(count);

    for index in 0..count {
        let line_position = read_u64(bytes, start + 16 + index * 8)? as usize;
        let line = pool
            .get(&line_position)
            .ok_or("Line table references an unknown line")?;

        lines.push(line.clone());
    }

    Ok(lines)
}

fn read_block_end(bytes: &[u8], start: usize, magic: &[u8]) -> Result<usize, String> {
    if bytes.get(start..start + 4) != Some(magic) {
        return Err(format!("Missing {} block", String::from_utf8_lossy(magic).trim()));
    }

    let length = read_u32(bytes, start + 4)? as usize;
    let end = start + length;

    if end > bytes.len() {
        return Err(String::from("Block extends past the end of the file"));
    }

    Ok(end)
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let slice = bytes.get(offset..offset + 4).ok_or("Truncated CSB structure")?;
    let mut buffer = [0u8; 4];
    buffer.copy_from_slice(slice);
    Ok(u32::from_le_bytes(buffer))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let slice = bytes.get(offset..offset + 8).ok_or("Truncated CSB structure")?;
    let mut buffer = [0u8; 8];
    buffer.copy_from_slice(slice);
    Ok(u64::from_le_bytes(buffer))
}
