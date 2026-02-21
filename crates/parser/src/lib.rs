#![forbid(unsafe_code)]

use ast::{BinaryOp, Expr, Identifier};
use lexer::{Token, TokenKind, lex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub position: usize,
}

pub fn parse_expression(source: &str) -> Result<Expr, ParseError> {
    let tokens = lex(source).map_err(|err| ParseError {
        message: err.message,
        position: err.position,
    })?;
    let mut parser = Parser::new(tokens);
    let expr = parser.parse_additive()?;
    parser.expect_eof()?;
    Ok(expr)
}

#[derive(Debug)]
struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn parse_additive(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;
        while self.matches(&TokenKind::Plus) {
            let right = self.parse_primary()?;
            expr = Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let token = self.current().ok_or(ParseError {
            message: "unexpected end of input".to_string(),
            position: 0,
        })?;
        let kind = token.kind.clone();
        let position = token.span.start;

        match kind {
            TokenKind::Number(value) => {
                self.advance();
                Ok(Expr::Number(value))
            }
            TokenKind::Identifier(name) => {
                self.advance();
                Ok(Expr::Identifier(Identifier(name)))
            }
            TokenKind::LParen => {
                self.advance();
                let expr = self.parse_additive()?;
                self.expect(TokenKind::RParen, "expected ')' after expression")?;
                Ok(expr)
            }
            TokenKind::Plus => Err(ParseError {
                message: "unexpected '+' at expression start".to_string(),
                position,
            }),
            TokenKind::RParen => Err(ParseError {
                message: "unexpected ')'".to_string(),
                position,
            }),
            TokenKind::Eof => Err(ParseError {
                message: "empty input".to_string(),
                position,
            }),
        }
    }

    fn matches(&mut self, expected: &TokenKind) -> bool {
        match self.current() {
            Some(token) if &token.kind == expected => {
                self.advance();
                true
            }
            _ => false,
        }
    }

    fn expect(&mut self, expected: TokenKind, message: &str) -> Result<(), ParseError> {
        let token = self.current().ok_or(ParseError {
            message: message.to_string(),
            position: self.last_position(),
        })?;
        if token.kind == expected {
            self.advance();
            Ok(())
        } else {
            Err(ParseError {
                message: message.to_string(),
                position: token.span.start,
            })
        }
    }

    fn expect_eof(&mut self) -> Result<(), ParseError> {
        let token = self.current().ok_or(ParseError {
            message: "unexpected end of token stream".to_string(),
            position: self.last_position(),
        })?;
        if token.kind == TokenKind::Eof {
            Ok(())
        } else {
            Err(ParseError {
                message: "unexpected trailing input".to_string(),
                position: token.span.start,
            })
        }
    }

    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn last_position(&self) -> usize {
        self.tokens
            .last()
            .map(|token| token.span.end)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::parse_expression;
    use ast::{BinaryOp, Expr};

    #[test]
    fn parses_additive_expression() {
        let parsed = parse_expression("1 + 2").expect("parser should succeed");
        let expected = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Number(1.0)),
            right: Box::new(Expr::Number(2.0)),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_parenthesized_expression() {
        let parsed = parse_expression("(1 + 2) + 3").expect("parser should succeed");
        let left = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Number(1.0)),
            right: Box::new(Expr::Number(2.0)),
        };
        let expected = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(left),
            right: Box::new(Expr::Number(3.0)),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn rejects_trailing_tokens() {
        let err = parse_expression("1 2").expect_err("parser should fail");
        assert_eq!(err.message, "unexpected trailing input");
    }
}
