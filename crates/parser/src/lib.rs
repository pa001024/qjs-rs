#![forbid(unsafe_code)]

use ast::{BinaryOp, BindingKind, Expr, Identifier, Script, Stmt, VariableDeclaration};
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
    let expr = parser.parse_expression_inner()?;
    parser.expect_eof()?;
    Ok(expr)
}

pub fn parse_script(source: &str) -> Result<Script, ParseError> {
    let tokens = lex(source).map_err(|err| ParseError {
        message: err.message,
        position: err.position,
    })?;
    let mut parser = Parser::new(tokens);
    let mut statements = Vec::new();

    while !parser.is_eof() {
        let statement = parser.parse_statement()?;
        statements.push(statement);

        if parser.matches(&TokenKind::Semicolon) {
            continue;
        }
        if parser.is_eof() {
            break;
        }
        return Err(parser.error_current("expected ';' between statements"));
    }

    parser.expect_eof()?;
    Ok(Script { statements })
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

    fn parse_statement(&mut self) -> Result<Stmt, ParseError> {
        if self.matches_keyword("let") {
            return self.parse_variable_declaration(BindingKind::Let);
        }
        if self.matches_keyword("const") {
            return self.parse_variable_declaration(BindingKind::Const);
        }
        let expr = self.parse_expression_inner()?;
        Ok(Stmt::Expression(expr))
    }

    fn parse_variable_declaration(&mut self, kind: BindingKind) -> Result<Stmt, ParseError> {
        let name = self.expect_identifier("expected binding name")?;
        let initializer = if self.matches(&TokenKind::Equal) {
            Some(self.parse_expression_inner()?)
        } else {
            None
        };

        if kind == BindingKind::Const && initializer.is_none() {
            return Err(ParseError {
                message: "const declaration requires an initializer".to_string(),
                position: self.current_position(),
            });
        }

        Ok(Stmt::VariableDeclaration(VariableDeclaration {
            kind,
            name: Identifier(name),
            initializer,
        }))
    }

    fn parse_expression_inner(&mut self) -> Result<Expr, ParseError> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_additive()?;
        if self.matches(&TokenKind::Equal) {
            let assignment_position = self.previous_position();
            let value = self.parse_assignment()?;
            match left {
                Expr::Identifier(target) => Ok(Expr::Assign {
                    target,
                    value: Box::new(value),
                }),
                _ => Err(ParseError {
                    message: "invalid assignment target".to_string(),
                    position: assignment_position,
                }),
            }
        } else {
            Ok(left)
        }
    }

    fn parse_additive(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_multiplicative()?;
        loop {
            let op = if self.matches(&TokenKind::Plus) {
                BinaryOp::Add
            } else if self.matches(&TokenKind::Minus) {
                BinaryOp::Sub
            } else {
                break;
            };
            let right = self.parse_multiplicative()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;
        loop {
            let op = if self.matches(&TokenKind::Star) {
                BinaryOp::Mul
            } else if self.matches(&TokenKind::Slash) {
                BinaryOp::Div
            } else {
                break;
            };
            let right = self.parse_primary()?;
            expr = Expr::Binary {
                op,
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
                let expr = self.parse_expression_inner()?;
                self.expect(TokenKind::RParen, "expected ')' after expression")?;
                Ok(expr)
            }
            TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
            | TokenKind::Equal => Err(ParseError {
                message: "unexpected operator at expression start".to_string(),
                position,
            }),
            TokenKind::Semicolon => Err(ParseError {
                message: "unexpected ';'".to_string(),
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

    fn expect_identifier(&mut self, message: &str) -> Result<String, ParseError> {
        let token = self.current().ok_or(ParseError {
            message: message.to_string(),
            position: self.last_position(),
        })?;
        if let TokenKind::Identifier(name) = &token.kind {
            let cloned = name.clone();
            self.advance();
            Ok(cloned)
        } else {
            Err(ParseError {
                message: message.to_string(),
                position: token.span.start,
            })
        }
    }

    fn matches_keyword(&mut self, keyword: &str) -> bool {
        match self.current() {
            Some(token) => match &token.kind {
                TokenKind::Identifier(name) if name == keyword => {
                    self.advance();
                    true
                }
                _ => false,
            },
            None => false,
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

    fn is_eof(&self) -> bool {
        matches!(
            self.current(),
            Some(Token {
                kind: TokenKind::Eof,
                ..
            })
        )
    }

    fn error_current(&self, message: &str) -> ParseError {
        ParseError {
            message: message.to_string(),
            position: self.current_position(),
        }
    }

    fn current_position(&self) -> usize {
        self.current()
            .map(|token| token.span.start)
            .unwrap_or_else(|| self.last_position())
    }

    fn previous_position(&self) -> usize {
        self.tokens
            .get(self.pos.saturating_sub(1))
            .map(|token| token.span.start)
            .unwrap_or_default()
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
    use super::{parse_expression, parse_script};
    use ast::{BinaryOp, BindingKind, Expr, Identifier, Script, Stmt, VariableDeclaration};

    #[test]
    fn parses_additive_expression() {
        let parsed = parse_expression("1 + 2 - 3").expect("parser should succeed");
        let add = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Number(1.0)),
            right: Box::new(Expr::Number(2.0)),
        };
        let expected = Expr::Binary {
            op: BinaryOp::Sub,
            left: Box::new(add),
            right: Box::new(Expr::Number(3.0)),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_assignment_expression() {
        let parsed = parse_expression("x = 1 + 2").expect("parser should succeed");
        let expected = Expr::Assign {
            target: Identifier("x".to_string()),
            value: Box::new(Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Number(1.0)),
                right: Box::new(Expr::Number(2.0)),
            }),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn enforces_multiplicative_precedence() {
        let parsed = parse_expression("1 + 2 * 3").expect("parser should succeed");
        let right = Expr::Binary {
            op: BinaryOp::Mul,
            left: Box::new(Expr::Number(2.0)),
            right: Box::new(Expr::Number(3.0)),
        };
        let expected = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Number(1.0)),
            right: Box::new(right),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_script_with_variable_declarations() {
        let parsed = parse_script("let x = 1; const y = x + 2; x = y * 3; x;")
            .expect("script parsing should succeed");
        let expected = Script {
            statements: vec![
                Stmt::VariableDeclaration(VariableDeclaration {
                    kind: BindingKind::Let,
                    name: Identifier("x".to_string()),
                    initializer: Some(Expr::Number(1.0)),
                }),
                Stmt::VariableDeclaration(VariableDeclaration {
                    kind: BindingKind::Const,
                    name: Identifier("y".to_string()),
                    initializer: Some(Expr::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(Expr::Identifier(Identifier("x".to_string()))),
                        right: Box::new(Expr::Number(2.0)),
                    }),
                }),
                Stmt::Expression(Expr::Assign {
                    target: Identifier("x".to_string()),
                    value: Box::new(Expr::Binary {
                        op: BinaryOp::Mul,
                        left: Box::new(Expr::Identifier(Identifier("y".to_string()))),
                        right: Box::new(Expr::Number(3.0)),
                    }),
                }),
                Stmt::Expression(Expr::Identifier(Identifier("x".to_string()))),
            ],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn rejects_const_without_initializer() {
        let err = parse_script("const x;").expect_err("parser should fail");
        assert_eq!(err.message, "const declaration requires an initializer");
    }

    #[test]
    fn rejects_trailing_tokens() {
        let err = parse_expression("1 2").expect_err("parser should fail");
        assert_eq!(err.message, "unexpected trailing input");
    }
}
