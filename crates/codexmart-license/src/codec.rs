//! Human-friendly codes: request codes, activation keys and license serial
//! numbers.
//!
//! Codes use 32 symbols that are hard to confuse when copied from a phone
//! screen: the digits 2-9 and the letters A-Z without I and O. The
//! characters 0, 1, I and O never appear. Input is case-insensitive and
//! spaces, hyphens and line breaks are ignored.
//!
//! Every group of a request code or activation key ends in a check symbol
//! (Luhn mod 32 over the group's position and its four data symbols), so a
//! mistyped character is reported together with the group it is in. Check
//! symbols only catch typing mistakes. They prove nothing about where a code
//! came from: authenticity comes from the Ed25519 signature inside an
//! activation key.

use std::fmt;

pub const ALPHABET: &[u8; 32] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";

/// Data symbols per group (each group also has one check symbol).
pub const GROUP_DATA: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeKind {
    pub prefix: &'static str,
    pub groups: usize,
    salt: u8,
    pub name: &'static str,
}

pub const REQUEST: CodeKind = CodeKind { prefix: "CXMR", groups: 8, salt: 7, name: "request code" };
pub const ACTIVATION: CodeKind = CodeKind { prefix: "CXMA", groups: 34, salt: 19, name: "activation key" };
/// License serial numbers: `CXMT-XXXX-XXXX-XXXX-XXXX`, one check symbol at
/// the end, like a bank card number.
pub const SERIAL_PREFIX: &str = "CXMT";
const SERIAL_SALT: u8 = 29;

const KINDS: [CodeKind; 2] = [REQUEST, ACTIVATION];

pub fn symbol_value(c: char) -> Option<u8> {
    ALPHABET.iter().position(|&a| a as char == c).map(|p| p as u8)
}

fn symbol_char(v: u8) -> char {
    ALPHABET[(v & 31) as usize] as char
}

/// Splits bytes into 5-bit symbols (most significant bit first), padding
/// the last symbol and then the whole stream with zeros up to `symbols`.
pub fn encode_symbols(bytes: &[u8], symbols: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(symbols);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for &b in bytes {
        acc = (acc << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(((acc >> bits) & 31) as u8);
        }
        acc &= (1 << bits) - 1;
    }
    if bits > 0 {
        out.push(((acc << (5 - bits)) & 31) as u8);
    }
    out.resize(out.len().max(symbols), 0);
    out
}

/// Reverse of [`encode_symbols`]. Padding bits must be zero.
pub fn decode_symbols(symbols: &[u8], byte_len: usize) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(byte_len);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for &s in symbols {
        acc = (acc << 5) | (s & 31) as u32;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            let byte = (acc >> bits) & 0xFF;
            if out.len() < byte_len {
                out.push(byte as u8);
            } else if byte != 0 {
                return None;
            }
            acc &= (1 << bits) - 1;
        }
    }
    (out.len() == byte_len && acc == 0).then_some(out)
}

/// Luhn mod 32 check symbol for `values` (each 0..32).
pub fn luhn32(values: &[u8]) -> u8 {
    let mut factor = 2u32;
    let mut sum = 0u32;
    for &v in values.iter().rev() {
        let addend = factor * v as u32;
        sum += addend / 32 + addend % 32;
        factor = 3 - factor;
    }
    ((32 - sum % 32) % 32) as u8
}

fn group_check(kind: &CodeKind, index: usize, data: &[u8]) -> u8 {
    let mut values = Vec::with_capacity(GROUP_DATA + 1);
    values.push(((kind.salt as usize + index) % 32) as u8);
    values.extend_from_slice(data);
    luhn32(&values)
}

/// Formats data symbols (a multiple of four) as `PREFIX-XXXXX-XXXXX-...`.
pub fn format_code(kind: &CodeKind, symbols: &[u8]) -> String {
    debug_assert_eq!(symbols.len(), kind.groups * GROUP_DATA);
    let mut out = String::from(kind.prefix);
    for (i, chunk) in symbols.chunks(GROUP_DATA).enumerate() {
        out.push('-');
        out.extend(chunk.iter().map(|&s| symbol_char(s)));
        out.push(symbol_char(group_check(kind, i + 1, chunk)));
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeError {
    Empty { name: &'static str },
    WrongKind { expected: &'static str, found: &'static str },
    WrongPrefix { name: &'static str, prefix: &'static str },
    InvalidCharacter { ch: char, group: usize },
    Incomplete { name: &'static str, groups: usize, expected: usize },
    TooLong { name: &'static str, groups: usize, expected: usize },
    Checksum { groups: Vec<(usize, String)> },
}

impl fmt::Display for CodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodeError::Empty { name } => write!(f, "Enter the {name}."),
            CodeError::WrongKind { expected, found } => {
                write!(f, "This is a {found}, not an {expected}.")?;
                if *found == REQUEST.name {
                    write!(
                        f,
                        " Send the request code to the software owner and enter the activation key you receive back."
                    )?;
                }
                Ok(())
            }
            CodeError::WrongPrefix { name, prefix } => write!(f, "A {name} starts with {prefix}."),
            CodeError::InvalidCharacter { ch, group } => {
                write!(f, "Group {group} contains \"{ch}\", which is never used in codes. Codes have no 0, 1, I or O.")
            }
            CodeError::Incomplete { name, groups, expected } => {
                write!(f, "The {name} is incomplete: {groups} of {expected} groups entered.")
            }
            CodeError::TooLong { name, groups, expected } => {
                write!(f, "The {name} is too long: {groups} groups entered, {expected} expected.")
            }
            CodeError::Checksum { groups } => {
                let list: Vec<String> = groups.iter().map(|(n, text)| format!("{n} ({text})")).collect();
                let plural = if groups.len() == 1 { "group" } else { "groups" };
                write!(f, "Typing mistake in {plural} {}. Check against the code you received.", list.join(", "))
            }
        }
    }
}

impl std::error::Error for CodeError {}

/// One group of a code being typed, for live feedback in the UI.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupStatus {
    pub text: String,
    /// `None` while the group is still being typed.
    pub ok: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inspection {
    pub groups: Vec<GroupStatus>,
    pub result: Result<Vec<u8>, CodeError>,
}

fn normalise(input: &str) -> String {
    input.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect()
}

/// Checks a code as typed so far: per-group status plus the decoded data
/// symbols when the code is complete and every check symbol matches.
pub fn inspect(kind: &CodeKind, input: &str) -> Inspection {
    let mut text = normalise(input);
    let total = kind.groups * (GROUP_DATA + 1);
    if text.is_empty() {
        return Inspection { groups: vec![], result: Err(CodeError::Empty { name: kind.name }) };
    }
    if let Some(rest) = text.strip_prefix(kind.prefix) {
        text = rest.to_string();
    } else if let Some(other) = KINDS.iter().find(|k| k.prefix != kind.prefix && text.starts_with(k.prefix)) {
        return Inspection {
            groups: vec![],
            result: Err(CodeError::WrongKind { expected: kind.name, found: other.name }),
        };
    } else if text.starts_with(SERIAL_PREFIX) && text.len() <= SERIAL_PREFIX.len() + 16 {
        return Inspection {
            groups: vec![],
            result: Err(CodeError::WrongKind { expected: kind.name, found: "license serial number" }),
        };
    } else if text.len() != total && kind.prefix.starts_with(&text[..text.len().min(kind.prefix.len())]) {
        // Still typing the prefix.
        return Inspection {
            groups: vec![],
            result: Err(CodeError::Incomplete { name: kind.name, groups: 0, expected: kind.groups }),
        };
    } else if text.len() != total {
        return Inspection {
            groups: vec![],
            result: Err(CodeError::WrongPrefix { name: kind.name, prefix: kind.prefix }),
        };
    }

    let chars: Vec<char> = text.chars().collect();
    let mut groups = Vec::new();
    let mut symbols = Vec::with_capacity(kind.groups * GROUP_DATA);
    let mut bad = Vec::new();
    let mut invalid = None;
    for (i, chunk) in chars.chunks(GROUP_DATA + 1).enumerate() {
        let group_text: String = chunk.iter().collect();
        let values: Option<Vec<u8>> = chunk.iter().map(|&c| symbol_value(c)).collect();
        let ok = match &values {
            None => {
                if invalid.is_none() {
                    let ch = *chunk.iter().find(|&&c| symbol_value(c).is_none()).unwrap();
                    invalid = Some(CodeError::InvalidCharacter { ch, group: i + 1 });
                }
                Some(false)
            }
            Some(v) if v.len() == GROUP_DATA + 1 => {
                let ok = group_check(kind, i + 1, &v[..GROUP_DATA]) == v[GROUP_DATA];
                if !ok {
                    bad.push((i + 1, group_text.clone()));
                }
                symbols.extend_from_slice(&v[..GROUP_DATA]);
                Some(ok)
            }
            Some(_) => None,
        };
        groups.push(GroupStatus { text: group_text, ok });
    }

    let complete_groups = chars.len() / (GROUP_DATA + 1);
    let result = if let Some(e) = invalid {
        Err(e)
    } else if !bad.is_empty() {
        Err(CodeError::Checksum { groups: bad })
    } else if chars.len() < total {
        Err(CodeError::Incomplete { name: kind.name, groups: complete_groups, expected: kind.groups })
    } else if chars.len() > total {
        Err(CodeError::TooLong { name: kind.name, groups: chars.len().div_ceil(GROUP_DATA + 1), expected: kind.groups })
    } else {
        Ok(symbols)
    };
    Inspection { groups, result }
}

/// Decodes a complete code into its data symbols.
pub fn parse_code(kind: &CodeKind, input: &str) -> Result<Vec<u8>, CodeError> {
    inspect(kind, input).result
}

/// Formats a 9-byte license id as `CXMT-XXXX-XXXX-XXXX-XXXX`: 15 data
/// symbols and a Luhn mod 32 check symbol.
pub fn format_serial(license_id: &[u8; 9]) -> String {
    let mut symbols = encode_symbols(license_id, 15);
    let mut values = vec![SERIAL_SALT];
    values.extend_from_slice(&symbols);
    symbols.push(luhn32(&values));
    let mut out = String::from(SERIAL_PREFIX);
    for chunk in symbols.chunks(4) {
        out.push('-');
        out.extend(chunk.iter().map(|&s| symbol_char(s)));
    }
    out
}

/// Parses a license serial number (for searching the owner's history).
/// Returns `None` if the format or check symbol is wrong.
pub fn parse_serial(input: &str) -> Option<[u8; 9]> {
    let text = normalise(input);
    let text = text.strip_prefix(SERIAL_PREFIX).unwrap_or(&text);
    if text.len() != 16 {
        return None;
    }
    let values: Vec<u8> = text.chars().map(symbol_value).collect::<Option<_>>()?;
    let mut check_input = vec![SERIAL_SALT];
    check_input.extend_from_slice(&values[..15]);
    if luhn32(&check_input) != values[15] {
        return None;
    }
    decode_symbols(&values[..15], 9)?.try_into().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_round_trip_for_all_lengths() {
        for len in 0..40usize {
            let bytes: Vec<u8> = (0..len).map(|i| (i * 37 + 11) as u8).collect();
            let n = (len * 8).div_ceil(5);
            let symbols = encode_symbols(&bytes, n);
            assert_eq!(decode_symbols(&symbols, len).unwrap(), bytes);
        }
    }

    #[test]
    fn nonzero_padding_is_rejected() {
        let mut symbols = encode_symbols(&[0xAB], 2);
        symbols[1] |= 1; // padding bit
        assert!(decode_symbols(&symbols, 1).is_none());
    }

    #[test]
    fn alphabet_excludes_confusing_characters() {
        for c in ['0', '1', 'I', 'O'] {
            assert!(symbol_value(c).is_none());
        }
        assert_eq!(ALPHABET.len(), 32);
    }

    #[test]
    fn luhn32_detects_every_single_symbol_change() {
        let data = [3u8, 17, 29, 8, 0];
        let check = luhn32(&data);
        for pos in 0..data.len() {
            for v in 0..32u8 {
                if v == data[pos] {
                    continue;
                }
                let mut changed = data;
                changed[pos] = v;
                assert_ne!(luhn32(&changed), check, "change at {pos} to {v} not detected");
            }
        }
    }

    #[test]
    fn code_round_trip_and_tolerant_input() {
        let symbols: Vec<u8> = (0..32).map(|i| (i * 7 % 32) as u8).collect();
        let code = format_code(&REQUEST, &symbols);
        assert!(code.starts_with("CXMR-"));
        assert_eq!(code.len(), 4 + 8 * 6);
        assert_eq!(parse_code(&REQUEST, &code).unwrap(), symbols);
        let messy = format!("  {}  ", code.to_lowercase().replace('-', " "));
        assert_eq!(parse_code(&REQUEST, &messy).unwrap(), symbols);
        let no_prefix = code.trim_start_matches("CXMR-").to_string();
        assert_eq!(parse_code(&REQUEST, &no_prefix).unwrap(), symbols);
    }

    #[test]
    fn typing_mistakes_are_located_by_group() {
        let symbols: Vec<u8> = (0..32).map(|i| (i * 5 % 32) as u8).collect();
        let code = format_code(&REQUEST, &symbols);
        let mut chars: Vec<char> = code.chars().collect();
        // Group 3 starts at 4 + 1 + 2*6 = 17.
        chars[18] = if chars[18] == 'Z' { 'Y' } else { 'Z' };
        let typo: String = chars.into_iter().collect();
        match parse_code(&REQUEST, &typo) {
            Err(CodeError::Checksum { groups }) => assert_eq!(groups[0].0, 3),
            other => panic!("expected checksum error, got {other:?}"),
        }
    }

    #[test]
    fn swapped_adjacent_symbols_are_detected() {
        let symbols: Vec<u8> = (0..32).map(|i| ((i * 11 + 3) % 32) as u8).collect();
        let code = format_code(&REQUEST, &symbols);
        let mut chars: Vec<char> = code.chars().collect();
        chars.swap(5, 6);
        if chars[5] != chars[6] {
            assert!(parse_code(&REQUEST, &chars.into_iter().collect::<String>()).is_err());
        }
    }

    #[test]
    fn wrong_kind_and_bad_characters_are_explained() {
        let symbols = vec![1u8; 32];
        let request = format_code(&REQUEST, &symbols);
        assert!(matches!(parse_code(&ACTIVATION, &request), Err(CodeError::WrongKind { .. })));
        let with_o = request.replacen('-', "-O", 1);
        assert!(matches!(parse_code(&REQUEST, &with_o), Err(CodeError::InvalidCharacter { ch: 'O', group: 1 })));
        let short = &request[..request.len() - 6];
        assert!(matches!(parse_code(&REQUEST, short), Err(CodeError::Incomplete { groups: 7, .. })));
    }

    #[test]
    fn serial_round_trip_and_check() {
        let id = [9u8, 8, 7, 6, 5, 4, 3, 2, 1];
        let serial = format_serial(&id);
        assert_eq!(serial.len(), 24);
        assert!(serial.starts_with("CXMT-"));
        assert_eq!(parse_serial(&serial), Some(id));
        assert_eq!(parse_serial(&serial.to_lowercase().replace('-', " ")), Some(id));
        let mut typo: Vec<char> = serial.chars().collect();
        typo[6] = if typo[6] == '2' { '3' } else { '2' };
        assert_eq!(parse_serial(&typo.into_iter().collect::<String>()), None);
    }
}
