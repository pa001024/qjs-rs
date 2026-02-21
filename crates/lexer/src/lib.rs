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
    Identifier(String),
    Plus,
    Minus,
    Star,
    Slash,
    Bang,
    Equal,
    EqualEqual,
    EqualEqualEqual,
    BangEqual,
    BangEqualEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    AndAnd,
    OrOr,
    Dot,
    Comma,
    Colon,
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

pub fn lex(source: &str) -> Result<Vec<Token>, LexError> {
    let mut tokens = Vec::new();
    let bytes = source.as_bytes();
    let mut pos = 0usize;

    while pos < bytes.len() {
        let byte = bytes[pos];
        if byte.is_ascii_whitespace() {
            pos += 1;
            continue;
        }

        if byte == b'+' {
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
                pos += 2;
                while pos < bytes.len() && bytes[pos] != b'\n' {
                    pos += 1;
                }
                continue;
            }
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'*' {
                let start = pos;
                pos += 2;
                let mut terminated = false;
                while pos + 1 < bytes.len() {
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
                continue;
            }
            tokens.push(Token {
                kind: TokenKind::Slash,
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
            let is_double = pos + 1 < bytes.len() && bytes[pos + 1] == b'=';
            tokens.push(Token {
                kind: if is_double {
                    TokenKind::LessEqual
                } else {
                    TokenKind::Less
                },
                span: Span {
                    start: pos,
                    end: if is_double { pos + 2 } else { pos + 1 },
                },
            });
            pos += if is_double { 2 } else { 1 };
            continue;
        }

        if byte == b'>' {
            let is_double = pos + 1 < bytes.len() && bytes[pos + 1] == b'=';
            tokens.push(Token {
                kind: if is_double {
                    TokenKind::GreaterEqual
                } else {
                    TokenKind::Greater
                },
                span: Span {
                    start: pos,
                    end: if is_double { pos + 2 } else { pos + 1 },
                },
            });
            pos += if is_double { 2 } else { 1 };
            continue;
        }

        if byte == b'&' {
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'&' {
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
            return Err(LexError {
                message: "unexpected character '&'".to_string(),
                position: pos,
            });
        }

        if byte == b'|' {
            if pos + 1 < bytes.len() && bytes[pos + 1] == b'|' {
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
            return Err(LexError {
                message: "unexpected character '|'".to_string(),
                position: pos,
            });
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
            pos += 1;
            continue;
        }

        if byte == b'}' {
            tokens.push(Token {
                kind: TokenKind::RBrace,
                span: Span {
                    start: pos,
                    end: pos + 1,
                },
            });
            pos += 1;
            continue;
        }

        if byte.is_ascii_digit() {
            let start = pos;
            let mut has_dot = false;
            while pos < bytes.len() {
                let current = bytes[pos];
                if current.is_ascii_digit() {
                    pos += 1;
                    continue;
                }
                if current == b'.' && !has_dot {
                    has_dot = true;
                    pos += 1;
                    continue;
                }
                break;
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
                    let escaped = bytes[pos];
                    let ch = match escaped {
                        b'\'' => '\'',
                        b'"' => '"',
                        b'\\' => '\\',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        _ => {
                            return Err(LexError {
                                message: format!(
                                    "unsupported escape sequence '\\{}'",
                                    escaped as char
                                ),
                                position: pos.saturating_sub(1),
                            });
                        }
                    };
                    value.push(ch);
                    pos += 1;
                    continue;
                }
                value.push(current as char);
                pos += 1;
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

        if byte.is_ascii_alphabetic() || byte == b'_' || byte == b'$' {
            let start = pos;
            while pos < bytes.len() {
                let current = bytes[pos];
                if current.is_ascii_alphanumeric() || current == b'_' || current == b'$' {
                    pos += 1;
                    continue;
                }
                break;
            }
            let ident = source[start..pos].to_string();
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
    fn lexes_member_access_dot() {
        let tokens = lex("obj.value").expect("tokenization should succeed");
        assert_eq!(tokens[0].kind, TokenKind::Identifier("obj".to_string()));
        assert_eq!(tokens[1].kind, TokenKind::Dot);
        assert_eq!(tokens[2].kind, TokenKind::Identifier("value".to_string()));
        assert_eq!(tokens[3].kind, TokenKind::Eof);
    }

    #[test]
    fn errors_on_invalid_character() {
        let err = lex("1 @ 2").expect_err("tokenization should fail");
        assert_eq!(err.position, 2);
    }
}
