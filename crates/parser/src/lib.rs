#![forbid(unsafe_code)]

use ast::{
    BinaryOp, BindingKind, Expr, FunctionDeclaration, Identifier, Script, Stmt, UnaryOp,
    VariableDeclaration,
};
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
    let statements = parser.parse_statement_list(None)?;
    parser.expect_eof()?;
    Ok(Script { statements })
}

#[derive(Debug)]
struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    function_depth: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            function_depth: 0,
        }
    }

    fn parse_statement_list(
        &mut self,
        terminator: Option<TokenKind>,
    ) -> Result<Vec<Stmt>, ParseError> {
        let mut statements = Vec::new();

        loop {
            if let Some(term) = &terminator {
                if self.check(term) {
                    break;
                }
            }
            if self.is_eof() {
                break;
            }

            let statement = self.parse_statement()?;
            let needs_separator = !matches!(
                statement,
                Stmt::Block(_)
                    | Stmt::FunctionDeclaration(_)
                    | Stmt::If { .. }
                    | Stmt::While { .. }
            );
            statements.push(statement);

            if self.matches(&TokenKind::Semicolon) {
                continue;
            }
            if let Some(term) = &terminator {
                if self.check(term) {
                    continue;
                }
            }
            if self.is_eof() {
                break;
            }
            if needs_separator {
                return Err(self.error_current("expected ';' between statements"));
            }
        }

        Ok(statements)
    }

    fn parse_statement(&mut self) -> Result<Stmt, ParseError> {
        if self.check(&TokenKind::LBrace) {
            return self.parse_block_statement();
        }
        if self.matches_keyword("function") {
            return self.parse_function_declaration_statement();
        }
        if self.matches_keyword("if") {
            return self.parse_if_statement();
        }
        if self.matches_keyword("while") {
            return self.parse_while_statement();
        }
        if self.matches_keyword("return") {
            return self.parse_return_statement();
        }
        if self.matches_keyword("let") {
            return self.parse_variable_declaration(BindingKind::Let);
        }
        if self.matches_keyword("const") {
            return self.parse_variable_declaration(BindingKind::Const);
        }
        let expr = self.parse_expression_inner()?;
        Ok(Stmt::Expression(expr))
    }

    fn parse_block_statement(&mut self) -> Result<Stmt, ParseError> {
        self.expect(TokenKind::LBrace, "expected '{' to start block")?;
        let statements = self.parse_statement_list(Some(TokenKind::RBrace))?;
        self.expect(TokenKind::RBrace, "expected '}' after block")?;
        Ok(Stmt::Block(statements))
    }

    fn parse_function_declaration_statement(&mut self) -> Result<Stmt, ParseError> {
        let name = Identifier(self.expect_identifier("expected function name")?);
        self.expect(TokenKind::LParen, "expected '(' after function name")?;
        let params = self.parse_parameter_list()?;
        self.expect(TokenKind::RParen, "expected ')' after parameters")?;

        self.expect(TokenKind::LBrace, "expected '{' before function body")?;
        self.function_depth += 1;
        let body = self.parse_statement_list(Some(TokenKind::RBrace));
        self.function_depth = self.function_depth.saturating_sub(1);
        let body = body?;
        self.expect(TokenKind::RBrace, "expected '}' after function body")?;

        Ok(Stmt::FunctionDeclaration(FunctionDeclaration {
            name,
            params,
            body,
        }))
    }

    fn parse_if_statement(&mut self) -> Result<Stmt, ParseError> {
        self.expect(TokenKind::LParen, "expected '(' after 'if'")?;
        let condition = self.parse_expression_inner()?;
        self.expect(TokenKind::RParen, "expected ')' after if condition")?;

        let consequent = self.parse_embedded_statement(true)?;
        let alternate = if self.matches_keyword("else") {
            Some(Box::new(self.parse_embedded_statement(false)?))
        } else {
            None
        };

        Ok(Stmt::If {
            condition,
            consequent: Box::new(consequent),
            alternate,
        })
    }

    fn parse_while_statement(&mut self) -> Result<Stmt, ParseError> {
        self.expect(TokenKind::LParen, "expected '(' after 'while'")?;
        let condition = self.parse_expression_inner()?;
        self.expect(TokenKind::RParen, "expected ')' after while condition")?;
        let body = self.parse_embedded_statement(false)?;
        Ok(Stmt::While {
            condition,
            body: Box::new(body),
        })
    }

    fn parse_embedded_statement(
        &mut self,
        allow_else_terminator: bool,
    ) -> Result<Stmt, ParseError> {
        let statement = self.parse_statement()?;
        let needs_separator = !matches!(
            statement,
            Stmt::Block(_) | Stmt::FunctionDeclaration(_) | Stmt::If { .. } | Stmt::While { .. }
        );
        if self.matches(&TokenKind::Semicolon) {
            return Ok(statement);
        }

        let can_end_without_separator = self.is_eof()
            || self.check(&TokenKind::RBrace)
            || (allow_else_terminator && self.check_keyword("else"));
        if can_end_without_separator {
            return Ok(statement);
        }

        if needs_separator {
            return Err(self.error_current("expected ';' between statements"));
        }

        Ok(statement)
    }

    fn parse_return_statement(&mut self) -> Result<Stmt, ParseError> {
        if self.function_depth == 0 {
            return Err(ParseError {
                message: "return outside function".to_string(),
                position: self.previous_position(),
            });
        }

        let has_expr = !matches!(
            self.current().map(|token| &token.kind),
            Some(TokenKind::Semicolon | TokenKind::RBrace | TokenKind::Eof) | None
        );
        if has_expr {
            let expr = self.parse_expression_inner()?;
            Ok(Stmt::Return(Some(expr)))
        } else {
            Ok(Stmt::Return(None))
        }
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
        let left = self.parse_comparison()?;
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

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_additive()?;
        loop {
            let op = if self.matches(&TokenKind::EqualEqual) {
                BinaryOp::Equal
            } else if self.matches(&TokenKind::BangEqual) {
                BinaryOp::NotEqual
            } else if self.matches(&TokenKind::Less) {
                BinaryOp::Less
            } else if self.matches(&TokenKind::LessEqual) {
                BinaryOp::LessEqual
            } else if self.matches(&TokenKind::Greater) {
                BinaryOp::Greater
            } else if self.matches(&TokenKind::GreaterEqual) {
                BinaryOp::GreaterEqual
            } else {
                break;
            };
            let right = self.parse_additive()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
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
        let mut expr = self.parse_unary()?;
        loop {
            let op = if self.matches(&TokenKind::Star) {
                BinaryOp::Mul
            } else if self.matches(&TokenKind::Slash) {
                BinaryOp::Div
            } else {
                break;
            };
            let right = self.parse_unary()?;
            expr = Expr::Binary {
                op,
                left: Box::new(expr),
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        if self.matches(&TokenKind::Plus) {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Plus,
                expr: Box::new(expr),
            });
        }
        if self.matches(&TokenKind::Minus) {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Minus,
                expr: Box::new(expr),
            });
        }
        if self.matches(&TokenKind::Bang) {
            let expr = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: Box::new(expr),
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;
        while self.matches(&TokenKind::LParen) {
            let arguments = self.parse_argument_list()?;
            self.expect(TokenKind::RParen, "expected ')' after arguments")?;
            expr = Expr::Call {
                callee: Box::new(expr),
                arguments,
            };
        }
        Ok(expr)
    }

    fn parse_argument_list(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        if self.check(&TokenKind::RParen) {
            return Ok(args);
        }
        loop {
            args.push(self.parse_expression_inner()?);
            if self.matches(&TokenKind::Comma) {
                continue;
            }
            break;
        }
        Ok(args)
    }

    fn parse_parameter_list(&mut self) -> Result<Vec<Identifier>, ParseError> {
        let mut params = Vec::new();
        if self.check(&TokenKind::RParen) {
            return Ok(params);
        }
        loop {
            params.push(Identifier(
                self.expect_identifier("expected parameter name")?,
            ));
            if self.matches(&TokenKind::Comma) {
                continue;
            }
            break;
        }
        Ok(params)
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
            TokenKind::LBrace => Err(ParseError {
                message: "unexpected '{' in expression".to_string(),
                position,
            }),
            TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Star
            | TokenKind::Slash
            | TokenKind::Bang
            | TokenKind::Equal
            | TokenKind::EqualEqual
            | TokenKind::BangEqual
            | TokenKind::Less
            | TokenKind::LessEqual
            | TokenKind::Greater
            | TokenKind::GreaterEqual
            | TokenKind::Comma => Err(ParseError {
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
            TokenKind::RBrace => Err(ParseError {
                message: "unexpected '}'".to_string(),
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

    fn check_keyword(&self, keyword: &str) -> bool {
        matches!(
            self.current().map(|token| &token.kind),
            Some(TokenKind::Identifier(name)) if name == keyword
        )
    }

    fn check(&self, expected: &TokenKind) -> bool {
        matches!(self.current(), Some(token) if &token.kind == expected)
    }

    fn matches(&mut self, expected: &TokenKind) -> bool {
        if self.check(expected) {
            self.advance();
            true
        } else {
            false
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
        self.check(&TokenKind::Eof)
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
    use ast::{
        BinaryOp, BindingKind, Expr, FunctionDeclaration, Identifier, Script, Stmt, UnaryOp,
        VariableDeclaration,
    };

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
    fn parses_call_expression() {
        let parsed = parse_expression("add(1, mul(2, 3))").expect("parser should succeed");
        let expected = Expr::Call {
            callee: Box::new(Expr::Identifier(Identifier("add".to_string()))),
            arguments: vec![
                Expr::Number(1.0),
                Expr::Call {
                    callee: Box::new(Expr::Identifier(Identifier("mul".to_string()))),
                    arguments: vec![Expr::Number(2.0), Expr::Number(3.0)],
                },
            ],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_unary_expression() {
        let parsed = parse_expression("!-x").expect("parser should succeed");
        let expected = Expr::Unary {
            op: UnaryOp::Not,
            expr: Box::new(Expr::Unary {
                op: UnaryOp::Minus,
                expr: Box::new(Expr::Identifier(Identifier("x".to_string()))),
            }),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_comparison_expression() {
        let parsed = parse_expression("1 + 2 * 3 >= 7").expect("parser should succeed");
        let expected = Expr::Binary {
            op: BinaryOp::GreaterEqual,
            left: Box::new(Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Number(1.0)),
                right: Box::new(Expr::Binary {
                    op: BinaryOp::Mul,
                    left: Box::new(Expr::Number(2.0)),
                    right: Box::new(Expr::Number(3.0)),
                }),
            }),
            right: Box::new(Expr::Number(7.0)),
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_function_declaration_and_return() {
        let parsed = parse_script("function add(a, b) { return a + b; } add(1, 2);")
            .expect("script parsing should succeed");
        let expected = Script {
            statements: vec![
                Stmt::FunctionDeclaration(FunctionDeclaration {
                    name: Identifier("add".to_string()),
                    params: vec![Identifier("a".to_string()), Identifier("b".to_string())],
                    body: vec![Stmt::Return(Some(Expr::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(Expr::Identifier(Identifier("a".to_string()))),
                        right: Box::new(Expr::Identifier(Identifier("b".to_string()))),
                    }))],
                }),
                Stmt::Expression(Expr::Call {
                    callee: Box::new(Expr::Identifier(Identifier("add".to_string()))),
                    arguments: vec![Expr::Number(1.0), Expr::Number(2.0)],
                }),
            ],
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
    fn parses_block_statement_and_shadowing_syntax() {
        let parsed = parse_script("let x = 1; { let x = 2; x = x + 1; }; x;")
            .expect("script parsing should succeed");
        assert_eq!(parsed.statements.len(), 3);
        assert!(matches!(parsed.statements[1], Stmt::Block(_)));
    }

    #[test]
    fn allows_statement_after_block_without_semicolon() {
        let parsed =
            parse_script("{ let x = 1; } let y = 2; y;").expect("script parsing should succeed");
        assert_eq!(parsed.statements.len(), 3);
    }

    #[test]
    fn parses_if_else_statement() {
        let parsed =
            parse_script("if (1 < 2) x = 1; else x = 2;").expect("script parsing should succeed");
        let expected = Script {
            statements: vec![Stmt::If {
                condition: Expr::Binary {
                    op: BinaryOp::Less,
                    left: Box::new(Expr::Number(1.0)),
                    right: Box::new(Expr::Number(2.0)),
                },
                consequent: Box::new(Stmt::Expression(Expr::Assign {
                    target: Identifier("x".to_string()),
                    value: Box::new(Expr::Number(1.0)),
                })),
                alternate: Some(Box::new(Stmt::Expression(Expr::Assign {
                    target: Identifier("x".to_string()),
                    value: Box::new(Expr::Number(2.0)),
                }))),
            }],
        };
        assert_eq!(parsed, expected);
    }

    #[test]
    fn parses_while_statement() {
        let parsed = parse_script("let x = 0; while (x < 3) x = x + 1; x;")
            .expect("script parsing should succeed");
        assert_eq!(parsed.statements.len(), 3);
        assert!(matches!(parsed.statements[1], Stmt::While { .. }));
    }

    #[test]
    fn rejects_return_outside_function() {
        let err = parse_script("return 1;").expect_err("parser should fail");
        assert_eq!(err.message, "return outside function");
    }

    #[test]
    fn rejects_const_without_initializer() {
        let err = parse_script("const x;").expect_err("parser should fail");
        assert_eq!(err.message, "const declaration requires an initializer");
    }

    #[test]
    fn rejects_invalid_assignment_target() {
        let err = parse_expression("(x + 1) = 2").expect_err("parser should fail");
        assert_eq!(err.message, "invalid assignment target");
    }

    #[test]
    fn rejects_missing_separator_in_if_consequent() {
        let err = parse_script("if (1) x = 1 y = 2;").expect_err("parser should fail");
        assert_eq!(err.message, "expected ';' between statements");
    }

    #[test]
    fn rejects_trailing_tokens() {
        let err = parse_expression("1 2").expect_err("parser should fail");
        assert_eq!(err.message, "unexpected trailing input");
    }
}
