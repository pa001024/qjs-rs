#![forbid(unsafe_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Number(f64),
    String(String),
    TemplatePart {
        cooked: String,
        raw: String,
        has_escape: bool,
        invalid_escape: bool,
        tail: bool,
    },
    Identifier(String),
    Plus,
    PlusEqual,
    PlusPlus,
    Minus,
    MinusEqual,
    MinusMinus,
    Star,
    StarEqual,
    Slash,
    SlashEqual,
    Percent,
    PercentEqual,
    Amp,
    AmpEqual,
    Pipe,
    PipeEqual,
    Caret,
    CaretEqual,
    Tilde,
    Bang,
    Equal,
    EqualEqual,
    EqualEqualEqual,
    BangEqual,
    BangEqualEqual,
    Less,
    LessLess,
    LessLessEqual,
    LessEqual,
    Greater,
    GreaterGreater,
    GreaterGreaterEqual,
    GreaterGreaterGreater,
    GreaterGreaterGreaterEqual,
    GreaterEqual,
    AndAnd,
    AndAndEqual,
    OrOr,
    OrOrEqual,
    QuestionQuestion,
    QuestionQuestionEqual,
    Ellipsis,
    Dot,
    Comma,
    Colon,
    At,
    Question,
    Semicolon,
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub message: String,
    pub position: usize,
}

fn unicode_line_terminator_len(bytes: &[u8], pos: usize) -> Option<usize> {
    if pos + 2 < bytes.len()
        && bytes[pos] == 0xE2
        && bytes[pos + 1] == 0x80
        && matches!(bytes[pos + 2], 0xA8 | 0xA9)
    {
        Some(3)
    } else {
        None
    }
}

fn line_terminator_len_at(bytes: &[u8], pos: usize) -> Option<usize> {
    if pos >= bytes.len() {
        return None;
    }
    if bytes[pos] == b'\n' {
        return Some(1);
    }
    if bytes[pos] == b'\r' {
        if pos + 1 < bytes.len() && bytes[pos + 1] == b'\n' {
            return Some(2);
        }
        return Some(1);
    }
    unicode_line_terminator_len(bytes, pos)
}

fn skip_line_comment_payload(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() && line_terminator_len_at(bytes, pos).is_none() {
        pos += 1;
    }
    pos
}

fn is_ecmascript_whitespace(ch: char) -> bool {
    ch == '\u{FEFF}' || ch.is_whitespace()
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_'
        || ch == '$'
        || ch.is_ascii_alphabetic()
        || (!ch.is_ascii() && !is_ecmascript_whitespace(ch))
}

fn is_identifier_part(ch: char) -> bool {
    is_identifier_start(ch) || ch.is_ascii_digit()
}

fn decode_unicode_escape(source: &str, pos: usize) -> Option<(char, usize)> {
    let bytes = source.as_bytes();
    if pos + 2 > bytes.len() || bytes[pos] != b'\\' || bytes[pos + 1] != b'u' {
        return None;
    }
    if pos + 6 <= bytes.len() {
        let hex = std::str::from_utf8(&bytes[pos + 2..pos + 6]).ok()?;
        if hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
            let code_point = u32::from_str_radix(hex, 16).ok()?;
            let ch = char::from_u32(code_point)?;
            return Some((ch, 6));
        }
    }
    if pos + 3 > bytes.len() || bytes[pos + 2] != b'{' {
        return None;
    }
    let mut end = pos + 3;
    while end < bytes.len() && bytes[end] != b'}' {
        end += 1;
    }
    if end >= bytes.len() || bytes[end] != b'}' {
        return None;
    }
    let hex = std::str::from_utf8(&bytes[pos + 3..end]).ok()?;
    if hex.is_empty() || hex.len() > 6 || !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }
    let code_point = u32::from_str_radix(hex, 16).ok()?;
    if code_point > 0x10FFFF {
        return None;
    }
    let ch = char::from_u32(code_point)?;
    Some((ch, end + 1 - pos))
}

fn surrogate_escape_placeholder(code_unit: u32) -> Option<char> {
    if !(0xD800..=0xDFFF).contains(&code_unit) {
        return None;
    }
    let mapped = 0xE000 + (code_unit - 0xD800);
    char::from_u32(mapped)
}

fn decode_char(source: &str, pos: usize) -> Option<(char, usize)> {
    let tail = source.get(pos..)?;
    let ch = tail.chars().next()?;
    Some((ch, ch.len_utf8()))
}

fn parse_legacy_octal_escape(bytes: &[u8], pos: usize) -> (u32, usize) {
    let first = (bytes[pos] - b'0') as u32;
    let mut value = first;
    let mut consumed = 1usize;
    if pos + 1 < bytes.len() && matches!(bytes[pos + 1], b'0'..=b'7') {
        value = value * 8 + (bytes[pos + 1] - b'0') as u32;
        consumed = 2;
        if first <= 3 && pos + 2 < bytes.len() && matches!(bytes[pos + 2], b'0'..=b'7') {
            value = value * 8 + (bytes[pos + 2] - b'0') as u32;
            consumed = 3;
        }
    }
    (value, consumed)
}

fn read_unicode_escape_char(source: &str, escape_start: usize) -> Option<(char, usize)> {
    let bytes = source.as_bytes();
    if escape_start + 2 > bytes.len()
        || bytes[escape_start] != b'\\'
        || bytes[escape_start + 1] != b'u'
    {
        return None;
    }
    if escape_start + 6 <= bytes.len() {
        let hex = std::str::from_utf8(&bytes[escape_start + 2..escape_start + 6]).ok()?;
        if hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
            let code_unit = u32::from_str_radix(hex, 16).ok()?;
            let ch =
                surrogate_escape_placeholder(code_unit).or_else(|| char::from_u32(code_unit))?;
            return Some((ch, 6));
        }
    }
    decode_unicode_escape(source, escape_start)
}

#[derive(Debug)]
struct LexedTemplatePart {
    cooked: String,
    raw: String,
    has_escape: bool,
    invalid_escape: bool,
    tail: bool,
    end: usize,
}

fn lex_template_part(source: &str, start: usize) -> Result<LexedTemplatePart, LexError> {
    let bytes = source.as_bytes();
    let mut pos = start;
    let mut cooked = String::new();
    let mut raw = String::new();
    let mut has_escape = false;
    let mut invalid_escape = false;

    while pos < bytes.len() {
        let byte = bytes[pos];
        if byte == b'`' {
            return Ok(LexedTemplatePart {
                cooked,
                raw,
                has_escape,
                invalid_escape,
                tail: true,
                end: pos + 1,
            });
        }
        if byte == b'$' && pos + 1 < bytes.len() && bytes[pos + 1] == b'{' {
            return Ok(LexedTemplatePart {
                cooked,
                raw,
                has_escape,
                invalid_escape,
                tail: false,
                end: pos + 2,
            });
        }
        if byte == b'\\' {
            has_escape = true;
            let escape_start = pos;
            raw.push('\\');
            pos += 1;
            if pos >= bytes.len() {
                return Err(LexError {
                    message: "unterminated template literal".to_string(),
                    position: start.saturating_sub(1),
                });
            }
            if let Some(line_terminator_len) = line_terminator_len_at(bytes, pos) {
                if bytes[pos] == b'\r' || bytes[pos] == b'\n' {
                    raw.push('\n');
                } else {
                    let Some((line_ch, _)) = decode_char(source, pos) else {
                        return Err(LexError {
                            message: "unterminated template literal".to_string(),
                            position: start.saturating_sub(1),
                        });
                    };
                    raw.push(line_ch);
                }
                pos += line_terminator_len;
                continue;
            }
            let escaped = bytes[pos];
            match escaped {
                b'`' => {
                    raw.push('`');
                    cooked.push('`');
                    pos += 1;
                }
                b'$' => {
                    raw.push('$');
                    cooked.push('$');
                    pos += 1;
                }
                b'\\' => {
                    raw.push('\\');
                    cooked.push('\\');
                    pos += 1;
                }
                b'n' => {
                    raw.push('n');
                    cooked.push('\n');
                    pos += 1;
                }
                b'r' => {
                    raw.push('r');
                    cooked.push('\r');
                    pos += 1;
                }
                b't' => {
                    raw.push('t');
                    cooked.push('\t');
                    pos += 1;
                }
                b'b' => {
                    raw.push('b');
                    cooked.push('\u{0008}');
                    pos += 1;
                }
                b'f' => {
                    raw.push('f');
                    cooked.push('\u{000c}');
                    pos += 1;
                }
                b'v' => {
                    raw.push('v');
                    cooked.push('\u{000b}');
                    pos += 1;
                }
                b'0' => {
                    raw.push('0');
                    if pos + 1 < bytes.len() && bytes[pos + 1].is_ascii_digit() {
                        invalid_escape = true;
                    } else {
                        cooked.push('\0');
                    }
                    pos += 1;
                }
                b'x' => {
                    raw.push('x');
                    if pos + 2 < bytes.len() {
                        let maybe_hex = std::str::from_utf8(&bytes[pos + 1..pos + 3]).ok();
                        if let Some(hex) = maybe_hex {
                            if hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
                                let code_point = u32::from_str_radix(hex, 16).ok();
                                if let Some(code_point) = code_point {
                                    if let Some(ch) = char::from_u32(code_point) {
                                        raw.push_str(&source[pos + 1..pos + 3]);
                                        cooked.push(ch);
                                        pos += 3;
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                    invalid_escape = true;
                    pos += 1;
                }
                b'u' => {
                    raw.push('u');
                    if let Some((ch, escape_len)) = read_unicode_escape_char(source, escape_start) {
                        raw.push_str(&source[pos + 1..escape_start + escape_len]);
                        cooked.push(ch);
                        pos = escape_start + escape_len;
                    } else {
                        invalid_escape = true;
                        pos += 1;
                    }
                }
                _ => {
                    let Some((ch, len)) = decode_char(source, pos) else {
                        return Err(LexError {
                            message: "unterminated template literal".to_string(),
                            position: start.saturating_sub(1),
                        });
                    };
                    raw.push_str(&source[pos..pos + len]);
                    if ch.is_ascii_digit() {
                        invalid_escape = true;
                    } else {
                        cooked.push(ch);
                    }
                    pos += len;
                }
            }
            continue;
        }
        if byte == b'\r' {
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'\n' {
                pos += 2;
            } else {
                pos += 1;
            }
            raw.push('\n');
            cooked.push('\n');
            continue;
        }
        if byte == b'\n' {
            raw.push('\n');
            cooked.push('\n');
            pos += 1;
            continue;
        }
        if let Some(line_terminator_len) = unicode_line_terminator_len(bytes, pos) {
            let Some((ch, _)) = decode_char(source, pos) else {
                return Err(LexError {
                    message: "unterminated template literal".to_string(),
                    position: start.saturating_sub(1),
                });
            };
            raw.push(ch);
            cooked.push(ch);
            pos += line_terminator_len;
            continue;
        }
        let Some((ch, len)) = decode_char(source, pos) else {
            return Err(LexError {
                message: "unterminated template literal".to_string(),
                position: start.saturating_sub(1),
            });
        };
        raw.push(ch);
        cooked.push(ch);
        pos += len;
    }

    Err(LexError {
        message: "unterminated template literal".to_string(),
        position: start.saturating_sub(1),
    })
}

fn is_regexp_allowed_after(token: Option<&TokenKind>) -> bool {
    match token {
        None => true,
        Some(
            TokenKind::Number(_)
            | TokenKind::String(_)
            | TokenKind::TemplatePart { .. }
            | TokenKind::PlusPlus
            | TokenKind::MinusMinus
            | TokenKind::RParen
            | TokenKind::RBracket
            | TokenKind::RBrace,
        ) => false,
        Some(TokenKind::Identifier(name)) => {
            matches!(
                name.as_str(),
                "return"
                    | "throw"
                    | "case"
                    | "delete"
                    | "void"
                    | "typeof"
                    | "instanceof"
                    | "in"
                    | "new"
                    | "do"
                    | "else"
                    | "yield"
                    | "await"
            )
        }
        Some(_) => true,
    }
}

fn lex_regexp_literal_end(source: &str, start: usize) -> Result<usize, LexError> {
    let bytes = source.as_bytes();
    let mut pos = start + 1;
    let mut in_character_class = false;

    while pos < bytes.len() {
        if let Some(line_terminator_len) = line_terminator_len_at(bytes, pos) {
            return Err(LexError {
                message: "unterminated regular expression literal".to_string(),
                position: pos + line_terminator_len - 1,
            });
        }
        let byte = bytes[pos];
        if byte == b'\\' {
            pos += 1;
            if pos >= bytes.len() {
                return Err(LexError {
                    message: "unterminated regular expression literal".to_string(),
                    position: start,
                });
            }
            if line_terminator_len_at(bytes, pos).is_some() {
                return Err(LexError {
                    message: "unterminated regular expression literal".to_string(),
                    position: pos,
                });
            }
            let Some((_, len)) = decode_char(source, pos) else {
                return Err(LexError {
                    message: "unterminated regular expression literal".to_string(),
                    position: start,
                });
            };
            pos += len;
            continue;
        }
        if byte == b'[' {
            in_character_class = true;
            pos += 1;
            continue;
        }
        if byte == b']' {
            in_character_class = false;
            pos += 1;
            continue;
        }
        if byte == b'/' && !in_character_class {
            pos += 1;
            while pos < bytes.len() {
                let Some((flag, len)) = decode_char(source, pos) else {
                    break;
                };
                if !flag.is_ascii_alphabetic() {
                    break;
                }
                pos += len;
            }
            return Ok(pos);
        }
        let Some((_, len)) = decode_char(source, pos) else {
            return Err(LexError {
                message: "unterminated regular expression literal".to_string(),
                position: start,
            });
        };
        pos += len;
    }

    Err(LexError {
        message: "unterminated regular expression literal".to_string(),
        position: start,
    })
}

pub fn lex(source: &str) -> Result<Vec<Token>, LexError> {
    let mut tokens = Vec::new();
    let bytes = source.as_bytes();
    let mut pos = 0usize;
    let mut line_start = true;
    let mut brace_depth = 0usize;
    let mut template_expr_brace_targets = Vec::new();

    while pos < bytes.len() {
        if let Some(length) = line_terminator_len_at(bytes, pos) {
            pos += length;
            line_start = true;
            continue;
        }
        let byte = bytes[pos];
        if byte.is_ascii_whitespace() || byte == 0x0B {
            pos += 1;
            continue;
        }
        if let Some((ch, len)) = decode_char(source, pos) {
            if !ch.is_ascii() && is_ecmascript_whitespace(ch) {
                pos += len;
                continue;
            }
        }

        if pos + 3 < bytes.len() && &bytes[pos..pos + 4] == b"<!--" {
            pos = skip_line_comment_payload(bytes, pos + 4);
            line_start = true;
            continue;
        }

        let previous_line_start = line_start;
        if previous_line_start && pos + 2 < bytes.len() && &bytes[pos..pos + 3] == b"-->" {
            pos = skip_line_comment_payload(bytes, pos + 3);
            line_start = true;
            continue;
        }

        line_start = false;

        if byte == b'+' {
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'+' {
                tokens.push(Token {
                    kind: TokenKind::PlusPlus,
                    span: Span {
                        start: pos,
                        end: pos + 2,
                    },
                });
                pos += 2;
                continue;
            }
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'=' {
                tokens.push(Token {
                    kind: TokenKind::PlusEqual,
                    span: Span {
                        start: pos,
                        end: pos + 2,
                    },
                });
                pos += 2;
                continue;
            }
            tokens.push(Token {
                kind: TokenKind::Plus,
                span: Span {
                    start: pos,
                    end: pos + 1,
                },
            });
            pos += 1;
            continue;
        }

        if byte == b'-' {
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'-' {
                tokens.push(Token {
                    kind: TokenKind::MinusMinus,
                    span: Span {
                        start: pos,
                        end: pos + 2,
                    },
                });
                pos += 2;
                continue;
            }
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'=' {
                tokens.push(Token {
                    kind: TokenKind::MinusEqual,
                    span: Span {
                        start: pos,
                        end: pos + 2,
                    },
                });
                pos += 2;
                continue;
            }
            tokens.push(Token {
                kind: TokenKind::Minus,
                span: Span {
                    start: pos,
                    end: pos + 1,
                },
            });
            pos += 1;
            continue;
        }

        if byte == b'*' {
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'=' {
                tokens.push(Token {
                    kind: TokenKind::StarEqual,
                    span: Span {
                        start: pos,
                        end: pos + 2,
                    },
                });
                pos += 2;
                continue;
            }
            tokens.push(Token {
                kind: TokenKind::Star,
                span: Span {
                    start: pos,
                    end: pos + 1,
                },
            });
            pos += 1;
            continue;
        }

        if byte == b'/' {
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'/' {
                pos = skip_line_comment_payload(bytes, pos + 2);
                line_start = true;
                continue;
            }
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'*' {
                let start = pos;
                pos += 2;
                let mut terminated = false;
                let mut saw_line_terminator = false;
                while pos + 1 < bytes.len() {
                    if let Some(length) = line_terminator_len_at(bytes, pos) {
                        saw_line_terminator = true;
                        pos += length;
                        continue;
                    }
                    if bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
                        pos += 2;
                        terminated = true;
                        break;
                    }
                    pos += 1;
                }
                if !terminated {
                    return Err(LexError {
                        message: "unterminated block comment".to_string(),
                        position: start,
                    });
                }
                line_start = saw_line_terminator || previous_line_start;
                continue;
            }
            let token_start = pos;
            if is_regexp_allowed_after(tokens.last().map(|token| &token.kind)) {
                let regex_end = lex_regexp_literal_end(source, token_start)?;
                tokens.push(Token {
                    kind: TokenKind::Slash,
                    span: Span {
                        start: token_start,
                        end: regex_end,
                    },
                });
                pos = regex_end;
                continue;
            }
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'=' {
                tokens.push(Token {
                    kind: TokenKind::SlashEqual,
                    span: Span {
                        start: pos,
                        end: pos + 2,
                    },
                });
                pos += 2;
                continue;
            }
            tokens.push(Token {
                kind: TokenKind::Slash,
                span: Span {
                    start: token_start,
                    end: token_start + 1,
                },
            });
            pos = token_start + 1;
            continue;
        }

        if byte == b'%' {
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'=' {
                tokens.push(Token {
                    kind: TokenKind::PercentEqual,
                    span: Span {
                        start: pos,
                        end: pos + 2,
                    },
                });
                pos += 2;
                continue;
            }
            tokens.push(Token {
                kind: TokenKind::Percent,
                span: Span {
                    start: pos,
                    end: pos + 1,
                },
            });
            pos += 1;
            continue;
        }

        if byte == b'=' {
            let is_double = pos + 1 < bytes.len() && bytes[pos + 1] == b'=';
            let is_triple = is_double && pos + 2 < bytes.len() && bytes[pos + 2] == b'=';
            tokens.push(Token {
                kind: if is_triple {
                    TokenKind::EqualEqualEqual
                } else if is_double {
                    TokenKind::EqualEqual
                } else {
                    TokenKind::Equal
                },
                span: Span {
                    start: pos,
                    end: if is_triple {
                        pos + 3
                    } else if is_double {
                        pos + 2
                    } else {
                        pos + 1
                    },
                },
            });
            pos += if is_triple {
                3
            } else if is_double {
                2
            } else {
                1
            };
            continue;
        }

        if byte == b'!' {
            let is_double = pos + 1 < bytes.len() && bytes[pos + 1] == b'=';
            let is_triple = is_double && pos + 2 < bytes.len() && bytes[pos + 2] == b'=';
            tokens.push(Token {
                kind: if is_triple {
                    TokenKind::BangEqualEqual
                } else if is_double {
                    TokenKind::BangEqual
                } else {
                    TokenKind::Bang
                },
                span: Span {
                    start: pos,
                    end: if is_triple {
                        pos + 3
                    } else if is_double {
                        pos + 2
                    } else {
                        pos + 1
                    },
                },
            });
            pos += if is_triple {
                3
            } else if is_double {
                2
            } else {
                1
            };
            continue;
        }

        if byte == b'<' {
            let is_shift = pos + 1 < bytes.len() && bytes[pos + 1] == b'<';
            let is_shift_assign = is_shift && pos + 2 < bytes.len() && bytes[pos + 2] == b'=';
            let is_double = !is_shift && pos + 1 < bytes.len() && bytes[pos + 1] == b'=';
            tokens.push(Token {
                kind: if is_shift_assign {
                    TokenKind::LessLessEqual
                } else if is_shift {
                    TokenKind::LessLess
                } else if is_double {
                    TokenKind::LessEqual
                } else {
                    TokenKind::Less
                },
                span: Span {
                    start: pos,
                    end: if is_shift_assign {
                        pos + 3
                    } else if is_shift || is_double {
                        pos + 2
                    } else {
                        pos + 1
                    },
                },
            });
            pos += if is_shift_assign {
                3
            } else if is_shift || is_double {
                2
            } else {
                1
            };
            continue;
        }

        if byte == b'>' {
            let is_shift = pos + 1 < bytes.len() && bytes[pos + 1] == b'>';
            let is_shift2 = is_shift && pos + 2 < bytes.len() && bytes[pos + 2] == b'>';
            let is_shift3_assign = is_shift2 && pos + 3 < bytes.len() && bytes[pos + 3] == b'=';
            let is_shift_assign =
                is_shift && !is_shift2 && pos + 2 < bytes.len() && bytes[pos + 2] == b'=';
            let is_double = !is_shift && pos + 1 < bytes.len() && bytes[pos + 1] == b'=';
            tokens.push(Token {
                kind: if is_shift3_assign {
                    TokenKind::GreaterGreaterGreaterEqual
                } else if is_shift2 {
                    TokenKind::GreaterGreaterGreater
                } else if is_shift_assign {
                    TokenKind::GreaterGreaterEqual
                } else if is_shift {
                    TokenKind::GreaterGreater
                } else if is_double {
                    TokenKind::GreaterEqual
                } else {
                    TokenKind::Greater
                },
                span: Span {
                    start: pos,
                    end: if is_shift3_assign {
                        pos + 4
                    } else if is_shift2 || is_shift_assign {
                        pos + 3
                    } else if is_shift || is_double {
                        pos + 2
                    } else {
                        pos + 1
                    },
                },
            });
            pos += if is_shift3_assign {
                4
            } else if is_shift2 || is_shift_assign {
                3
            } else if is_shift || is_double {
                2
            } else {
                1
            };
            continue;
        }

        if byte == b'&' {
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'&' {
                if pos + 2 < bytes.len() && bytes[pos + 2] == b'=' {
                    tokens.push(Token {
                        kind: TokenKind::AndAndEqual,
                        span: Span {
                            start: pos,
                            end: pos + 3,
                        },
                    });
                    pos += 3;
                    continue;
                }
                tokens.push(Token {
                    kind: TokenKind::AndAnd,
                    span: Span {
                        start: pos,
                        end: pos + 2,
                    },
                });
                pos += 2;
                continue;
            }
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'=' {
                tokens.push(Token {
                    kind: TokenKind::AmpEqual,
                    span: Span {
                        start: pos,
                        end: pos + 2,
                    },
                });
                pos += 2;
                continue;
            }
            tokens.push(Token {
                kind: TokenKind::Amp,
                span: Span {
                    start: pos,
                    end: pos + 1,
                },
            });
            pos += 1;
            continue;
        }

        if byte == b'|' {
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'|' {
                if pos + 2 < bytes.len() && bytes[pos + 2] == b'=' {
                    tokens.push(Token {
                        kind: TokenKind::OrOrEqual,
                        span: Span {
                            start: pos,
                            end: pos + 3,
                        },
                    });
                    pos += 3;
                    continue;
                }
                tokens.push(Token {
                    kind: TokenKind::OrOr,
                    span: Span {
                        start: pos,
                        end: pos + 2,
                    },
                });
                pos += 2;
                continue;
            }
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'=' {
                tokens.push(Token {
                    kind: TokenKind::PipeEqual,
                    span: Span {
                        start: pos,
                        end: pos + 2,
                    },
                });
                pos += 2;
                continue;
            }
            tokens.push(Token {
                kind: TokenKind::Pipe,
                span: Span {
                    start: pos,
                    end: pos + 1,
                },
            });
            pos += 1;
            continue;
        }

        if byte == b'^' {
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'=' {
                tokens.push(Token {
                    kind: TokenKind::CaretEqual,
                    span: Span {
                        start: pos,
                        end: pos + 2,
                    },
                });
                pos += 2;
                continue;
            }
            tokens.push(Token {
                kind: TokenKind::Caret,
                span: Span {
                    start: pos,
                    end: pos + 1,
                },
            });
            pos += 1;
            continue;
        }

        if byte == b'~' {
            tokens.push(Token {
                kind: TokenKind::Tilde,
                span: Span {
                    start: pos,
                    end: pos + 1,
                },
            });
            pos += 1;
            continue;
        }

        if byte == b';' {
            tokens.push(Token {
                kind: TokenKind::Semicolon,
                span: Span {
                    start: pos,
                    end: pos + 1,
                },
            });
            pos += 1;
            continue;
        }

        if byte == b'.' {
            if pos + 2 < bytes.len() && bytes[pos + 1] == b'.' && bytes[pos + 2] == b'.' {
                tokens.push(Token {
                    kind: TokenKind::Ellipsis,
                    span: Span {
                        start: pos,
                        end: pos + 3,
                    },
                });
                pos += 3;
                continue;
            }
            if pos + 1 < bytes.len() && bytes[pos + 1].is_ascii_digit() {
                let start = pos;
                pos += 1; // consume '.'
                while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                    pos += 1;
                }
                if pos < bytes.len() && matches!(bytes[pos], b'e' | b'E') {
                    let exponent_start = pos;
                    pos += 1;
                    if pos < bytes.len() && matches!(bytes[pos], b'+' | b'-') {
                        pos += 1;
                    }
                    let exponent_digits_start = pos;
                    while pos < bytes.len() && bytes[pos].is_ascii_digit() {
                        pos += 1;
                    }
                    if exponent_digits_start == pos {
                        let raw = &source[start..exponent_start];
                        return Err(LexError {
                            message: format!("invalid number literal '{}'", &source[start..pos]),
                            position: start + raw.len(),
                        });
                    }
                }
                let raw = &source[start..pos];
                let value = raw.parse::<f64>().map_err(|_| LexError {
                    message: format!("invalid number literal '{raw}'"),
                    position: start,
                })?;
                tokens.push(Token {
                    kind: TokenKind::Number(value),
                    span: Span { start, end: pos },
                });
                continue;
            }
            tokens.push(Token {
                kind: TokenKind::Dot,
                span: Span {
                    start: pos,
                    end: pos + 1,
                },
            });
            pos += 1;
            continue;
        }

        if byte == b',' {
            tokens.push(Token {
                kind: TokenKind::Comma,
                span: Span {
                    start: pos,
                    end: pos + 1,
                },
            });
            pos += 1;
            continue;
        }

        if byte == b':' {
            tokens.push(Token {
                kind: TokenKind::Colon,
                span: Span {
                    start: pos,
                    end: pos + 1,
                },
            });
            pos += 1;
            continue;
        }

        if byte == b'@' {
            tokens.push(Token {
                kind: TokenKind::At,
                span: Span {
                    start: pos,
                    end: pos + 1,
                },
            });
            pos += 1;
            continue;
        }

        if byte == b'?' {
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'?' {
                if pos + 2 < bytes.len() && bytes[pos + 2] == b'=' {
                    tokens.push(Token {
                        kind: TokenKind::QuestionQuestionEqual,
                        span: Span {
                            start: pos,
                            end: pos + 3,
                        },
                    });
                    pos += 3;
                    continue;
                }
                tokens.push(Token {
                    kind: TokenKind::QuestionQuestion,
                    span: Span {
                        start: pos,
                        end: pos + 2,
                    },
                });
                pos += 2;
                continue;
            }
            tokens.push(Token {
                kind: TokenKind::Question,
                span: Span {
                    start: pos,
                    end: pos + 1,
                },
            });
            pos += 1;
            continue;
        }

        if byte == b'(' {
            tokens.push(Token {
                kind: TokenKind::LParen,
                span: Span {
                    start: pos,
                    end: pos + 1,
                },
            });
            pos += 1;
            continue;
        }

        if byte == b')' {
            tokens.push(Token {
                kind: TokenKind::RParen,
                span: Span {
                    start: pos,
                    end: pos + 1,
                },
            });
            pos += 1;
            continue;
        }

        if byte == b'[' {
            tokens.push(Token {
                kind: TokenKind::LBracket,
                span: Span {
                    start: pos,
                    end: pos + 1,
                },
            });
            pos += 1;
            continue;
        }

        if byte == b']' {
            tokens.push(Token {
                kind: TokenKind::RBracket,
                span: Span {
                    start: pos,
                    end: pos + 1,
                },
            });
            pos += 1;
            continue;
        }

        if byte == b'{' {
            tokens.push(Token {
                kind: TokenKind::LBrace,
                span: Span {
                    start: pos,
                    end: pos + 1,
                },
            });
            brace_depth += 1;
            pos += 1;
            continue;
        }

        if byte == b'}' {
            if let Some(target_depth) = template_expr_brace_targets.last().copied() {
                if brace_depth == target_depth {
                    tokens.push(Token {
                        kind: TokenKind::RBrace,
                        span: Span {
                            start: pos,
                            end: pos + 1,
                        },
                    });
                    pos += 1;
                    template_expr_brace_targets.pop();

                    let template_part_start = pos;
                    let template_part = lex_template_part(source, template_part_start)?;
                    tokens.push(Token {
                        kind: TokenKind::TemplatePart {
                            cooked: template_part.cooked,
                            raw: template_part.raw,
                            has_escape: template_part.has_escape,
                            invalid_escape: template_part.invalid_escape,
                            tail: template_part.tail,
                        },
                        span: Span {
                            start: template_part_start,
                            end: template_part.end,
                        },
                    });
                    pos = template_part.end;
                    if !template_part.tail {
                        template_expr_brace_targets.push(brace_depth);
                    }
                    continue;
                }
            }
            tokens.push(Token {
                kind: TokenKind::RBrace,
                span: Span {
                    start: pos,
                    end: pos + 1,
                },
            });
            brace_depth = brace_depth.saturating_sub(1);
            pos += 1;
            continue;
        }

        if byte == b'`' {
            let token_start = pos;
            let template_part = lex_template_part(source, token_start + 1)?;
            tokens.push(Token {
                kind: TokenKind::TemplatePart {
                    cooked: template_part.cooked,
                    raw: template_part.raw,
                    has_escape: template_part.has_escape,
                    invalid_escape: template_part.invalid_escape,
                    tail: template_part.tail,
                },
                span: Span {
                    start: token_start,
                    end: template_part.end,
                },
            });
            pos = template_part.end;
            if !template_part.tail {
                template_expr_brace_targets.push(brace_depth);
            }
            continue;
        }

        if byte.is_ascii_digit() {
            let start = pos;
            if byte == b'0' && pos + 1 < bytes.len() {
                let radix = match bytes[pos + 1] {
                    b'x' | b'X' => 16u32,
                    b'b' | b'B' => 2u32,
                    b'o' | b'O' => 8u32,
                    _ => 0u32,
                };
                if radix != 0 {
                    pos += 2;
                    let digits_start = pos;
                    while pos < bytes.len() {
                        let is_digit = match radix {
                            16 => bytes[pos].is_ascii_hexdigit(),
                            2 => matches!(bytes[pos], b'0' | b'1'),
                            8 => matches!(bytes[pos], b'0'..=b'7'),
                            _ => false,
                        };
                        if !is_digit {
                            break;
                        }
                        pos += 1;
                    }
                    if digits_start == pos {
                        return Err(LexError {
                            message: format!("invalid number literal '{}'", &source[start..pos]),
                            position: start,
                        });
                    }
                    let digits_end = pos;
                    let raw = &source[start..digits_end];
                    let digits = &raw[2..];
                    let value = u64::from_str_radix(digits, radix)
                        .map(|number| number as f64)
                        .map_err(|_| LexError {
                            message: format!("invalid number literal '{raw}'"),
                            position: start,
                        })?;
                    if pos < bytes.len() && bytes[pos] == b'n' {
                        pos += 1;
                    }
                    tokens.push(Token {
                        kind: TokenKind::Number(value),
                        span: Span { start, end: pos },
                    });
                    continue;
                }
                if matches!(bytes[pos + 1], b'0'..=b'7') {
                    let mut end = pos + 1;
                    while end < bytes.len() && bytes[end].is_ascii_digit() {
                        end += 1;
                    }
                    let digits = &bytes[(pos + 1)..end];
                    let has_non_octal = digits.iter().any(|digit| !matches!(digit, b'0'..=b'7'));
                    if !has_non_octal {
                        let digits_end = end;
                        pos = digits_end;
                        let raw = &source[start..digits_end];
                        let value = u64::from_str_radix(&raw[1..], 8)
                            .map(|number| number as f64)
                            .map_err(|_| LexError {
                                message: format!("invalid number literal '{raw}'"),
                                position: start,
                            })?;
                        if pos < bytes.len() && bytes[pos] == b'n' {
                            pos += 1;
                        }
                        tokens.push(Token {
                            kind: TokenKind::Number(value),
                            span: Span { start, end: pos },
                        });
                        continue;
                    }
                }
            }
            let mut has_dot = false;
            let mut has_exponent = false;
            let mut has_separator = false;
            let mut prev_was_separator = false;
            while pos < bytes.len() {
                let current = bytes[pos];
                if current.is_ascii_digit() {
                    prev_was_separator = false;
                    pos += 1;
                    continue;
                }
                if current == b'_' {
                    let next_is_digit = pos + 1 < bytes.len() && bytes[pos + 1].is_ascii_digit();
                    let prev_is_digit = pos > start && bytes[pos - 1].is_ascii_digit();
                    if prev_was_separator || !next_is_digit || !prev_is_digit {
                        return Err(LexError {
                            message: format!("invalid number literal '{}'", &source[start..=pos]),
                            position: pos,
                        });
                    }
                    has_separator = true;
                    prev_was_separator = true;
                    pos += 1;
                    continue;
                }
                if current == b'.' && !has_dot {
                    if prev_was_separator {
                        return Err(LexError {
                            message: format!("invalid number literal '{}'", &source[start..=pos]),
                            position: pos,
                        });
                    }
                    has_dot = true;
                    prev_was_separator = false;
                    pos += 1;
                    continue;
                }
                break;
            }
            if prev_was_separator {
                return Err(LexError {
                    message: format!("invalid number literal '{}'", &source[start..pos]),
                    position: pos.saturating_sub(1),
                });
            }
            if pos < bytes.len() && matches!(bytes[pos], b'e' | b'E') {
                let exponent_start = pos;
                pos += 1;
                if pos < bytes.len() && matches!(bytes[pos], b'+' | b'-') {
                    pos += 1;
                }
                let exponent_digits_start = pos;
                let mut exponent_prev_separator = false;
                while pos < bytes.len() {
                    let current = bytes[pos];
                    if current.is_ascii_digit() {
                        exponent_prev_separator = false;
                        pos += 1;
                        continue;
                    }
                    if current == b'_' {
                        let next_is_digit =
                            pos + 1 < bytes.len() && bytes[pos + 1].is_ascii_digit();
                        let prev_is_digit =
                            pos > exponent_digits_start && bytes[pos - 1].is_ascii_digit();
                        if pos == exponent_digits_start
                            || exponent_prev_separator
                            || !next_is_digit
                            || !prev_is_digit
                        {
                            return Err(LexError {
                                message: format!(
                                    "invalid number literal '{}'",
                                    &source[start..=pos]
                                ),
                                position: pos,
                            });
                        }
                        has_separator = true;
                        exponent_prev_separator = true;
                        pos += 1;
                        continue;
                    }
                    break;
                }
                if exponent_digits_start == pos || exponent_prev_separator {
                    let raw = &source[start..exponent_start];
                    return Err(LexError {
                        message: format!("invalid number literal '{}'", &source[start..pos]),
                        position: start + raw.len(),
                    });
                }
                has_exponent = true;
            }
            let numeric_end = pos;
            let raw = &source[start..numeric_end];
            let normalized = if has_separator {
                raw.replace('_', "")
            } else {
                raw.to_string()
            };
            let value = normalized.parse::<f64>().map_err(|_| LexError {
                message: format!("invalid number literal '{raw}'"),
                position: start,
            })?;
            if !has_dot && !has_exponent && pos < bytes.len() && bytes[pos] == b'n' {
                pos += 1;
            }
            tokens.push(Token {
                kind: TokenKind::Number(value),
                span: Span { start, end: pos },
            });
            continue;
        }

        if byte == b'\'' || byte == b'"' {
            let quote = byte;
            let start = pos;
            pos += 1;
            let mut value = String::new();
            let mut terminated = false;
            while pos < bytes.len() {
                let current = bytes[pos];
                if current == quote {
                    pos += 1;
                    terminated = true;
                    break;
                }
                if current == b'\\' {
                    pos += 1;
                    if pos >= bytes.len() {
                        return Err(LexError {
                            message: "unterminated string literal".to_string(),
                            position: start,
                        });
                    }
                    if let Some(line_terminator_len) = line_terminator_len_at(bytes, pos) {
                        pos += line_terminator_len;
                        continue;
                    }
                    let escaped = bytes[pos];
                    let (ch, advance) =
                        match escaped {
                            b'\'' => ('\'', 1usize),
                            b'"' => ('"', 1usize),
                            b'\\' => ('\\', 1usize),
                            b'n' => ('\n', 1usize),
                            b'r' => ('\r', 1usize),
                            b't' => ('\t', 1usize),
                            b'b' => ('\u{0008}', 1usize),
                            b'f' => ('\u{000c}', 1usize),
                            b'v' => ('\u{000b}', 1usize),
                            b'0' => {
                                if pos + 1 < bytes.len() && matches!(bytes[pos + 1], b'0'..=b'7') {
                                    let (code_point, len) = parse_legacy_octal_escape(bytes, pos);
                                    let ch = char::from_u32(code_point)
                                        .unwrap_or(char::REPLACEMENT_CHARACTER);
                                    (ch, len)
                                } else {
                                    ('\0', 1usize)
                                }
                            }
                            b'1'..=b'7' => {
                                let (code_point, len) = parse_legacy_octal_escape(bytes, pos);
                                let ch = char::from_u32(code_point)
                                    .unwrap_or(char::REPLACEMENT_CHARACTER);
                                (ch, len)
                            }
                            b'u' => {
                                if pos + 4 < bytes.len() {
                                    let hex = std::str::from_utf8(&bytes[pos + 1..pos + 5])
                                        .map_err(|_| LexError {
                                            message: "invalid unicode escape".to_string(),
                                            position: pos.saturating_sub(1),
                                        })?;
                                    if hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
                                        let code_unit =
                                            u32::from_str_radix(hex, 16).map_err(|_| LexError {
                                                message: "invalid unicode escape".to_string(),
                                                position: pos.saturating_sub(1),
                                            })?;
                                        let ch = surrogate_escape_placeholder(code_unit)
                                            .or_else(|| char::from_u32(code_unit))
                                            .ok_or(LexError {
                                                message: "invalid unicode escape".to_string(),
                                                position: pos.saturating_sub(1),
                                            })?;
                                        (ch, 5usize)
                                    } else {
                                        let escape_start = pos.saturating_sub(1);
                                        let Some((ch, escape_len)) =
                                            decode_unicode_escape(source, escape_start)
                                        else {
                                            return Err(LexError {
                                                message: "invalid unicode escape".to_string(),
                                                position: pos.saturating_sub(1),
                                            });
                                        };
                                        if escape_len <= 1 {
                                            return Err(LexError {
                                                message: "invalid unicode escape".to_string(),
                                                position: pos.saturating_sub(1),
                                            });
                                        }
                                        (ch, escape_len - 1)
                                    }
                                } else {
                                    let escape_start = pos.saturating_sub(1);
                                    let Some((ch, escape_len)) =
                                        decode_unicode_escape(source, escape_start)
                                    else {
                                        return Err(LexError {
                                            message: "invalid unicode escape".to_string(),
                                            position: pos.saturating_sub(1),
                                        });
                                    };
                                    if escape_len <= 1 {
                                        return Err(LexError {
                                            message: "invalid unicode escape".to_string(),
                                            position: pos.saturating_sub(1),
                                        });
                                    }
                                    (ch, escape_len - 1)
                                }
                            }
                            b'x' => {
                                if pos + 2 >= bytes.len() {
                                    return Err(LexError {
                                        message: "unterminated hex escape".to_string(),
                                        position: pos.saturating_sub(1),
                                    });
                                }
                                let hex = std::str::from_utf8(&bytes[pos + 1..pos + 3]).map_err(
                                    |_| LexError {
                                        message: "invalid hex escape".to_string(),
                                        position: pos.saturating_sub(1),
                                    },
                                )?;
                                let code_point =
                                    u32::from_str_radix(hex, 16).map_err(|_| LexError {
                                        message: "invalid hex escape".to_string(),
                                        position: pos.saturating_sub(1),
                                    })?;
                                let ch = char::from_u32(code_point).ok_or(LexError {
                                    message: "invalid hex escape".to_string(),
                                    position: pos.saturating_sub(1),
                                })?;
                                (ch, 3usize)
                            }
                            _ => {
                                let Some((ch, len)) = decode_char(source, pos) else {
                                    return Err(LexError {
                                        message: "unterminated string literal".to_string(),
                                        position: start,
                                    });
                                };
                                (ch, len)
                            }
                        };
                    value.push(ch);
                    pos += advance;
                    continue;
                }
                if current.is_ascii() {
                    value.push(current as char);
                    pos += 1;
                } else {
                    let Some((ch, len)) = decode_char(source, pos) else {
                        return Err(LexError {
                            message: "unterminated string literal".to_string(),
                            position: start,
                        });
                    };
                    if ch == '\n' || ch == '\r' || ch == '\u{2028}' || ch == '\u{2029}' {
                        return Err(LexError {
                            message: "unterminated string literal".to_string(),
                            position: start,
                        });
                    }
                    value.push(ch);
                    pos += len;
                }
                continue;
            }
            if !terminated {
                return Err(LexError {
                    message: "unterminated string literal".to_string(),
                    position: start,
                });
            }
            tokens.push(Token {
                kind: TokenKind::String(value),
                span: Span { start, end: pos },
            });
            continue;
        }

        let starts_identifier = if let Some((escaped, _)) = decode_unicode_escape(source, pos) {
            is_identifier_start(escaped)
        } else if let Some((ch, _)) = decode_char(source, pos) {
            is_identifier_start(ch)
        } else {
            false
        };
        if starts_identifier {
            let start = pos;
            let mut ident = String::new();

            if let Some((escaped, len)) = decode_unicode_escape(source, pos) {
                ident.push(escaped);
                pos += len;
            } else if let Some((ch, len)) = decode_char(source, pos) {
                ident.push(ch);
                pos += len;
            }

            while pos < bytes.len() {
                if let Some((escaped, len)) = decode_unicode_escape(source, pos) {
                    if !is_identifier_part(escaped) {
                        return Err(LexError {
                            message: "invalid identifier escape".to_string(),
                            position: pos,
                        });
                    }
                    ident.push(escaped);
                    pos += len;
                    continue;
                }

                let Some((ch, len)) = decode_char(source, pos) else {
                    break;
                };
                if !is_identifier_part(ch) {
                    break;
                }
                ident.push(ch);
                pos += len;
            }

            tokens.push(Token {
                kind: TokenKind::Identifier(ident),
                span: Span { start, end: pos },
            });
            continue;
        }

        return Err(LexError {
            message: format!("unexpected character '{}'", byte as char),
            position: pos,
        });
    }

    tokens.push(Token {
        kind: TokenKind::Eof,
        span: Span {
            start: source.len(),
            end: source.len(),
        },
    });
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::{TokenKind, lex};

    #[test]
    fn lexes_add_expression() {
        let tokens = lex("1 + 2").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Number(1.0));
        assert_eq!(tokens[1].kind, TokenKind::Plus);
        assert_eq!(tokens[2].kind, TokenKind::Number(2.0));
        assert_eq!(tokens[3].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_scientific_notation_numbers() {
        let tokens = lex("1e3 + 2E-2").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Number(1000.0));
        assert_eq!(tokens[1].kind, TokenKind::Plus);
        assert_eq!(tokens[2].kind, TokenKind::Number(0.02));
        assert_eq!(tokens[3].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_leading_decimal_numbers() {
        let tokens = lex(".5 + .25e1").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Number(0.5));
        assert_eq!(tokens[1].kind, TokenKind::Plus);
        assert_eq!(tokens[2].kind, TokenKind::Number(2.5));
        assert_eq!(tokens[3].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_hex_numbers() {
        let tokens = lex("0x0 + 0x1F + 0X10").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Number(0.0));
        assert_eq!(tokens[1].kind, TokenKind::Plus);
        assert_eq!(tokens[2].kind, TokenKind::Number(31.0));
        assert_eq!(tokens[3].kind, TokenKind::Plus);
        assert_eq!(tokens[4].kind, TokenKind::Number(16.0));
        assert_eq!(tokens[5].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_binary_and_octal_numbers() {
        let tokens = lex("0b10 + 0B11 + 0o10 + 0O7").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Number(2.0));
        assert_eq!(tokens[1].kind, TokenKind::Plus);
        assert_eq!(tokens[2].kind, TokenKind::Number(3.0));
        assert_eq!(tokens[3].kind, TokenKind::Plus);
        assert_eq!(tokens[4].kind, TokenKind::Number(8.0));
        assert_eq!(tokens[5].kind, TokenKind::Plus);
        assert_eq!(tokens[6].kind, TokenKind::Number(7.0));
        assert_eq!(tokens[7].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_legacy_octal_numbers() {
        let tokens = lex("070 + 08 + 09").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Number(56.0));
        assert_eq!(tokens[1].kind, TokenKind::Plus);
        assert_eq!(tokens[2].kind, TokenKind::Number(8.0));
        assert_eq!(tokens[3].kind, TokenKind::Plus);
        assert_eq!(tokens[4].kind, TokenKind::Number(9.0));
        assert_eq!(tokens[5].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_bigint_suffix_as_numeric_baseline() {
        let tokens = lex("0n + 0x10n + 0o7n + 0b11n").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Number(0.0));
        assert_eq!(tokens[1].kind, TokenKind::Plus);
        assert_eq!(tokens[2].kind, TokenKind::Number(16.0));
        assert_eq!(tokens[3].kind, TokenKind::Plus);
        assert_eq!(tokens[4].kind, TokenKind::Number(7.0));
        assert_eq!(tokens[5].kind, TokenKind::Plus);
        assert_eq!(tokens[6].kind, TokenKind::Number(3.0));
        assert_eq!(tokens[7].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_numeric_separators_in_decimal_and_bigint_baseline() {
        let tokens = lex("1_000 + 2_500n + 3_2.5_0 + 9e1_0").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Number(1000.0));
        assert_eq!(tokens[1].kind, TokenKind::Plus);
        assert_eq!(tokens[2].kind, TokenKind::Number(2500.0));
        assert_eq!(tokens[3].kind, TokenKind::Plus);
        assert_eq!(tokens[4].kind, TokenKind::Number(32.5));
        assert_eq!(tokens[5].kind, TokenKind::Plus);
        assert_eq!(tokens[6].kind, TokenKind::Number(9e10));
        assert_eq!(tokens[7].kind, TokenKind::Eof);
    }

    #[test]
    fn rejects_invalid_numeric_separator_placements() {
        assert!(lex("1__0").is_err());
        assert!(lex("1_.0").is_err());
        assert!(lex("1._0").is_err());
        assert!(lex("1e_2").is_err());
        assert!(lex("1e2_").is_err());
    }

    #[test]
    fn rejects_hex_without_digits() {
        let err = lex("0x").expect_err("tokenization should fail");
        assert!(err.message.starts_with("invalid number literal"));
    }

    #[test]
    fn rejects_binary_and_octal_without_digits() {
        assert!(lex("0b").is_err());
        assert!(lex("0o").is_err());
    }

    #[test]
    fn rejects_invalid_scientific_notation_literal() {
        let err = lex("1e+").expect_err("tokenization should fail");
        assert_eq!(err.message, "invalid number literal '1e+'");
    }

    #[test]
    fn lexes_all_arithmetic_operators() {
        let tokens = lex("8 - 2 * 3 / 4").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Number(8.0));
        assert_eq!(tokens[1].kind, TokenKind::Minus);
        assert_eq!(tokens[2].kind, TokenKind::Number(2.0));
        assert_eq!(tokens[3].kind, TokenKind::Star);
        assert_eq!(tokens[4].kind, TokenKind::Number(3.0));
        assert_eq!(tokens[5].kind, TokenKind::Slash);
        assert_eq!(tokens[6].kind, TokenKind::Number(4.0));
        assert_eq!(tokens[7].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_increment_and_decrement_tokens() {
        let tokens = lex("x++ --y").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Identifier("x".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::PlusPlus);
        assert_eq!(tokens[2].kind, TokenKind::MinusMinus);
        assert_eq!(tokens[3].kind, TokenKind::Identifier("y".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_parentheses() {
        let tokens = lex("(a + 3)").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::LParen);
        assert_eq!(tokens[1].kind, TokenKind::Identifier("a".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Plus);
        assert_eq!(tokens[3].kind, TokenKind::Number(3.0));
        assert_eq!(tokens[4].kind, TokenKind::RParen);
        assert_eq!(tokens[5].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_string_literals() {
        let tokens = lex("'a\\n' \"b\"").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::String("a\n".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::String("b".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_string_line_continuations() {
        let lf_tokens = lex("'a\\\nb'").expect("tokenization should succeed");
        assert_eq!(lf_tokens[0].kind, TokenKind::String("ab".to_string()));
        assert_eq!(lf_tokens[1].kind, TokenKind::Eof);

        let crlf_tokens = lex("'a\\\r\nb'").expect("tokenization should succeed");
        assert_eq!(crlf_tokens[0].kind, TokenKind::String("ab".to_string()));
        assert_eq!(crlf_tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_hex_and_unicode_string_escapes() {
        let tokens = lex("'\\x61' \"\\u0062\"").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::String("a".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::String("b".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_zero_and_control_string_escapes() {
        let tokens = lex("'\\0\\b\\f\\v'").expect("tokenization should succeed");
        assert_eq!(
            tokens[0].kind,
            TokenKind::String("\0\u{0008}\u{000c}\u{000b}".to_string())
        );
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_unicode_codepoint_string_escape() {
        let tokens = lex("\"\\u{1F600}\"").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::String("😀".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_surrogate_unicode_escape_in_string() {
        let tokens = lex("\"\\uD800\\uDC00\"").expect("tokenization should succeed");
        let TokenKind::String(value) = &tokens[0].kind else {
            panic!("expected string token");
        };
        let mut chars = value.chars();
        assert_eq!(chars.next().map(|ch| ch as u32), Some(0xE000));
        assert_eq!(chars.next().map(|ch| ch as u32), Some(0xE400));
        assert_eq!(chars.next(), None);
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_unicode_escape_in_identifier() {
        let tokens = lex("var \\u0061 = 1;").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Identifier("var".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Identifier("a".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Equal);
        assert_eq!(tokens[3].kind, TokenKind::Number(1.0));
        assert_eq!(tokens[4].kind, TokenKind::Semicolon);
        assert_eq!(tokens[5].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_unicode_codepoint_escape_in_identifier() {
        let tokens = lex("var _\\u{1F600} = 1;").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Identifier("var".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Identifier("_😀".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Equal);
        assert_eq!(tokens[3].kind, TokenKind::Number(1.0));
        assert_eq!(tokens[4].kind, TokenKind::Semicolon);
        assert_eq!(tokens[5].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_non_ascii_identifier() {
        let tokens = lex("var π = 3;").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Identifier("var".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Identifier("π".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Equal);
        assert_eq!(tokens[3].kind, TokenKind::Number(3.0));
        assert_eq!(tokens[4].kind, TokenKind::Semicolon);
        assert_eq!(tokens[5].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_assignment_and_terminator() {
        let tokens = lex("let x = 1;").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Identifier("let".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Identifier("x".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Equal);
        assert_eq!(tokens[3].kind, TokenKind::Number(1.0));
        assert_eq!(tokens[4].kind, TokenKind::Semicolon);
        assert_eq!(tokens[5].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_braces() {
        let tokens = lex("{ let x = 1; }").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::LBrace);
        assert_eq!(tokens[1].kind, TokenKind::Identifier("let".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Identifier("x".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Equal);
        assert_eq!(tokens[4].kind, TokenKind::Number(1.0));
        assert_eq!(tokens[5].kind, TokenKind::Semicolon);
        assert_eq!(tokens[6].kind, TokenKind::RBrace);
        assert_eq!(tokens[7].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_brackets() {
        let tokens = lex("arr[0]").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Identifier("arr".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::LBracket);
        assert_eq!(tokens[2].kind, TokenKind::Number(0.0));
        assert_eq!(tokens[3].kind, TokenKind::RBracket);
        assert_eq!(tokens[4].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_function_syntax() {
        let tokens =
            lex("function add(a, b) { return a + b; }").expect("tokenization should succeed");
        assert_eq!(
            tokens[0].kind,
            TokenKind::Identifier("function".to_string())
        );
        assert_eq!(tokens[1].kind, TokenKind::Identifier("add".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::LParen);
        assert_eq!(tokens[3].kind, TokenKind::Identifier("a".to_string()));
        assert_eq!(tokens[4].kind, TokenKind::Comma);
        assert_eq!(tokens[5].kind, TokenKind::Identifier("b".to_string()));
        assert_eq!(tokens[6].kind, TokenKind::RParen);
        assert_eq!(tokens[7].kind, TokenKind::LBrace);
        assert_eq!(tokens[8].kind, TokenKind::Identifier("return".to_string()));
    }

    #[test]
    fn skips_line_and_block_comments() {
        let tokens =
            lex("1 + 2 // comment #1\n/* block #2 */ + 3").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Number(1.0));
        assert_eq!(tokens[1].kind, TokenKind::Plus);
        assert_eq!(tokens[2].kind, TokenKind::Number(2.0));
        assert_eq!(tokens[3].kind, TokenKind::Plus);
        assert_eq!(tokens[4].kind, TokenKind::Number(3.0));
        assert_eq!(tokens[5].kind, TokenKind::Eof);
    }

    #[test]
    fn treats_unicode_line_separators_as_whitespace() {
        let tokens = lex("1\u{2028}+\u{2029}2").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Number(1.0));
        assert_eq!(tokens[1].kind, TokenKind::Plus);
        assert_eq!(tokens[2].kind, TokenKind::Number(2.0));
        assert_eq!(tokens[3].kind, TokenKind::Eof);
    }

    #[test]
    fn treats_vertical_tab_as_whitespace() {
        let tokens = lex("1\u{000B}+2").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Number(1.0));
        assert_eq!(tokens[1].kind, TokenKind::Plus);
        assert_eq!(tokens[2].kind, TokenKind::Number(2.0));
        assert_eq!(tokens[3].kind, TokenKind::Eof);
    }

    #[test]
    fn line_comment_ends_on_unicode_line_separator() {
        let tokens = lex("1//a\u{2028}+2").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Number(1.0));
        assert_eq!(tokens[1].kind, TokenKind::Plus);
        assert_eq!(tokens[2].kind, TokenKind::Number(2.0));
        assert_eq!(tokens[3].kind, TokenKind::Eof);
    }

    #[test]
    fn skips_annex_b_html_open_comment() {
        let tokens = lex("1<!--x\n+2").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Number(1.0));
        assert_eq!(tokens[1].kind, TokenKind::Plus);
        assert_eq!(tokens[2].kind, TokenKind::Number(2.0));
        assert_eq!(tokens[3].kind, TokenKind::Eof);
    }

    #[test]
    fn skips_annex_b_html_close_comment_at_line_start() {
        let tokens = lex("1\n-->x\n+2").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Number(1.0));
        assert_eq!(tokens[1].kind, TokenKind::Plus);
        assert_eq!(tokens[2].kind, TokenKind::Number(2.0));
        assert_eq!(tokens[3].kind, TokenKind::Eof);
    }

    #[test]
    fn skips_annex_b_html_close_comment_after_unicode_line_separator() {
        let tokens = lex("1\u{2028}-->x\u{2029}+2").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Number(1.0));
        assert_eq!(tokens[1].kind, TokenKind::Plus);
        assert_eq!(tokens[2].kind, TokenKind::Number(2.0));
        assert_eq!(tokens[3].kind, TokenKind::Eof);
    }

    #[test]
    fn keeps_html_close_comment_sequence_as_tokens_when_not_line_start() {
        let tokens = lex("x-->1").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Identifier("x".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::MinusMinus);
        assert_eq!(tokens[2].kind, TokenKind::Greater);
        assert_eq!(tokens[3].kind, TokenKind::Number(1.0));
        assert_eq!(tokens[4].kind, TokenKind::Eof);
    }

    #[test]
    fn allows_html_close_comment_after_leading_block_comment() {
        let tokens = lex("/*lead*/-->x\n1").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Number(1.0));
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn keeps_html_close_comment_sequence_after_non_line_start_block_comment() {
        let tokens = lex("a/*mid*/-->1").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Identifier("a".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::MinusMinus);
        assert_eq!(tokens[2].kind, TokenKind::Greater);
        assert_eq!(tokens[3].kind, TokenKind::Number(1.0));
        assert_eq!(tokens[4].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_switch_syntax() {
        let tokens = lex("switch (x) { case 1: break; default: continue; }")
            .expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Identifier("switch".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::LParen);
        assert_eq!(tokens[2].kind, TokenKind::Identifier("x".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::RParen);
        assert_eq!(tokens[4].kind, TokenKind::LBrace);
        assert_eq!(tokens[5].kind, TokenKind::Identifier("case".to_string()));
        assert_eq!(tokens[7].kind, TokenKind::Colon);
        assert_eq!(
            tokens[10].kind,
            TokenKind::Identifier("default".to_string())
        );
        assert_eq!(tokens[11].kind, TokenKind::Colon);
    }

    #[test]
    fn lexes_unary_and_comparison_operators() {
        let tokens =
            lex("!a == b != c === d !== e < f <= g > h >= i").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Bang);
        assert_eq!(tokens[1].kind, TokenKind::Identifier("a".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::EqualEqual);
        assert_eq!(tokens[4].kind, TokenKind::BangEqual);
        assert_eq!(tokens[6].kind, TokenKind::EqualEqualEqual);
        assert_eq!(tokens[8].kind, TokenKind::BangEqualEqual);
        assert_eq!(tokens[10].kind, TokenKind::Less);
        assert_eq!(tokens[12].kind, TokenKind::LessEqual);
        assert_eq!(tokens[14].kind, TokenKind::Greater);
        assert_eq!(tokens[16].kind, TokenKind::GreaterEqual);
    }

    #[test]
    fn lexes_logical_operators() {
        let tokens = lex("a && b || c").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Identifier("a".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::AndAnd);
        assert_eq!(tokens[2].kind, TokenKind::Identifier("b".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::OrOr);
        assert_eq!(tokens[4].kind, TokenKind::Identifier("c".to_string()));
        assert_eq!(tokens[5].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_nullish_and_logical_assignment_operators() {
        let tokens =
            lex("a ?? b; x &&= y; m ||= n; p ??= q;").expect("tokenization should succeed");
        let kinds: Vec<TokenKind> = tokens.into_iter().map(|token| token.kind).collect();
        assert!(kinds.contains(&TokenKind::QuestionQuestion));
        assert!(kinds.contains(&TokenKind::AndAndEqual));
        assert!(kinds.contains(&TokenKind::OrOrEqual));
        assert!(kinds.contains(&TokenKind::QuestionQuestionEqual));
    }

    #[test]
    fn lexes_member_access_dot() {
        let tokens = lex("obj.value").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Identifier("obj".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Dot);
        assert_eq!(tokens[2].kind, TokenKind::Identifier("value".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_ellipsis_token() {
        let tokens = lex("...x").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Ellipsis);
        assert_eq!(tokens[1].kind, TokenKind::Identifier("x".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_decorator_at_token() {
        let tokens = lex("@sealed").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::At);
        assert_eq!(tokens[1].kind, TokenKind::Identifier("sealed".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_regular_expression_literal_with_escape() {
        let tokens = lex(r"/\;/u").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Slash);
        assert_eq!(tokens[0].span.start, 0);
        assert_eq!(tokens[0].span.end, 5);
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_regex_literal_equal_body_before_slash_equal_tokenization() {
        let tokens = lex("/=/").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Slash);
        assert_eq!(tokens[0].span.start, 0);
        assert_eq!(tokens[0].span.end, 3);
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn keeps_slash_equal_in_expression_context() {
        let tokens = lex("a /= b").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Identifier("a".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::SlashEqual);
        assert_eq!(tokens[2].kind, TokenKind::Identifier("b".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Eof);
    }

    #[test]
    fn treats_zero_width_no_break_space_as_whitespace_after_regex_literal() {
        let tokens = lex("/x/g\u{FEFF};").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Slash);
        assert_eq!(tokens[0].span.start, 0);
        assert_eq!(tokens[0].span.end, 4);
        assert_eq!(tokens[1].kind, TokenKind::Semicolon);
        assert_eq!(tokens[2].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_template_literal_parts() {
        let tokens = lex("`a${b}c`").expect("tokenization should succeed");
        assert_eq!(
            tokens[0].kind,
            TokenKind::TemplatePart {
                cooked: "a".to_string(),
                raw: "a".to_string(),
                has_escape: false,
                invalid_escape: false,
                tail: false,
            }
        );
        assert_eq!(tokens[1].kind, TokenKind::Identifier("b".to_string()));
        assert_eq!(tokens[2].kind, TokenKind::RBrace);
        assert_eq!(
            tokens[3].kind,
            TokenKind::TemplatePart {
                cooked: "c".to_string(),
                raw: "c".to_string(),
                has_escape: false,
                invalid_escape: false,
                tail: true,
            }
        );
        assert_eq!(tokens[4].kind, TokenKind::Eof);
    }

    #[test]
    fn lexes_template_literal_without_substitution() {
        let tokens = lex("`ok`").expect("tokenization should succeed");
        assert_eq!(
            tokens[0].kind,
            TokenKind::TemplatePart {
                cooked: "ok".to_string(),
                raw: "ok".to_string(),
                has_escape: false,
                invalid_escape: false,
                tail: true,
            }
        );
        assert_eq!(tokens[1].kind, TokenKind::Eof);
    }

    #[test]
    fn preserves_template_raw_and_cooked_values() {
        let tokens = lex("`\\n`").expect("tokenization should succeed");
        assert_eq!(
            tokens[0].kind,
            TokenKind::TemplatePart {
                cooked: "\n".to_string(),
                raw: "\\n".to_string(),
                has_escape: true,
                invalid_escape: false,
                tail: true,
            }
        );
    }

    #[test]
    fn preserves_template_line_continuation_raw_values() {
        let tokens = lex("`\\\n`").expect("tokenization should succeed");
        assert_eq!(
            tokens[0].kind,
            TokenKind::TemplatePart {
                cooked: "".to_string(),
                raw: "\\\n".to_string(),
                has_escape: true,
                invalid_escape: false,
                tail: true,
            }
        );

        let tokens = lex("`\\\u{2028}`").expect("tokenization should succeed");
        assert_eq!(
            tokens[0].kind,
            TokenKind::TemplatePart {
                cooked: "".to_string(),
                raw: "\\\u{2028}".to_string(),
                has_escape: true,
                invalid_escape: false,
                tail: true,
            }
        );
    }

    #[test]
    fn marks_invalid_template_escape_sequences() {
        let tokens = lex("`\\xg`").expect("tokenization should succeed");
        assert_eq!(
            tokens[0].kind,
            TokenKind::TemplatePart {
                cooked: "g".to_string(),
                raw: "\\xg".to_string(),
                has_escape: true,
                invalid_escape: true,
                tail: true,
            }
        );
    }

    #[test]
    fn errors_on_invalid_character() {
        let err = lex("1 # 2").expect_err("tokenization should fail");
        assert_eq!(err.position, 2);
    }
}
