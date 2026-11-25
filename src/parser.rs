// src/parser.rs

use crate::ast::*;
use crate::token::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, current: 0 }
    }

    pub fn parse_program(&mut self) -> Result<Program, String> {
        self.parse_program_inner().map_err(|error| {
            if error.contains("columna") || error.contains("col ") {
                error
            } else {
                let token = self.peek();
                format!("{} (línea {}, columna {})", error, token.line, token.column)
            }
        })
    }

    fn parse_program_inner(&mut self) -> Result<Program, String> {
        let mut imports = Vec::new();
        let mut enums = Vec::new();
        let mut records = Vec::new();
        let mut extern_funcs = Vec::new();
        let mut classes = Vec::new();
        let mut functions = Vec::new();
        let mut main_block = Vec::new();

        self.skip_newlines();

        while self.check(&TokenKind::Import) || self.check(&TokenKind::From) {
            if self.match_token(&[TokenKind::From]) {
                let mut module_path =
                    self.consume_ident("Se esperaba nombre de módulo tras 'from'")?;
                while self.match_token(&[TokenKind::Dot]) {
                    let sub = self.consume_ident("Se esperaba sub-módulo tras '.'")?;
                    module_path.push('.');
                    module_path.push_str(&sub);
                }

                self.consume(
                    &TokenKind::Import,
                    "Se esperaba 'import' tras nombre de módulo",
                )?;
                let mut symbols = Vec::new();
                symbols.push(self.consume_ident("Se esperaba nombre de símbolo a importar")?);
                while self.match_token(&[TokenKind::Comma]) {
                    symbols.push(self.consume_ident("Se esperaba nombre de símbolo tras ','")?);
                }
                imports.push(ImportDecl {
                    module_path,
                    symbols,
                });
            } else if self.match_token(&[TokenKind::Import]) {
                let mut module_path =
                    self.consume_ident("Se esperaba nombre de módulo tras 'import'")?;
                while self.match_token(&[TokenKind::Dot]) {
                    let sub = self.consume_ident("Se esperaba sub-módulo tras '.'")?;
                    module_path.push('.');
                    module_path.push_str(&sub);
                }
                imports.push(ImportDecl {
                    module_path,
                    symbols: Vec::new(),
                });
            }
            self.skip_newlines();
        }

        while !self.is_at_end() {
            self.skip_newlines();

            if self.check(&TokenKind::Enum) {
                enums.push(self.parse_enum()?);
            } else if self.check(&TokenKind::Record) {
                records.push(self.parse_record()?);
            } else if self.check(&TokenKind::Extern) {
                extern_funcs.push(self.parse_extern_func()?);
            } else if self.check(&TokenKind::Class) {
                classes.push(self.parse_class()?);
            } else if self.check(&TokenKind::Func) {
                functions.push(self.parse_function(false)?);
            } else if self.check(&TokenKind::Begin) {
                main_block = self.parse_main_block()?;
                break;
            } else if self.is_at_end() {
                break;
            } else {
                return Err(format!(
                    "Token inesperado '{:?}' en línea {}",
                    self.peek().kind,
                    self.peek().line
                ));
            }
        }

        Ok(Program {
            imports,
            enums,
            records,
            extern_funcs,
            classes,
            functions,
            main_block,
        })
    }

    fn parse_record(&mut self) -> Result<RecordDecl, String> {
        let start = self.peek().clone();
        self.consume(&TokenKind::Record, "Se esperaba 'record'")?;
        let name = self.consume_ident("Se esperaba nombre de record")?;
        self.skip_newlines();
        self.consume(&TokenKind::Begin, "Se esperaba 'begin' al inicio de record")?;
        self.skip_newlines();

        let mut fields = Vec::new();
        while !self.check(&TokenKind::End) && !self.is_at_end() {
            self.skip_newlines();
            let type_token = self.peek().clone();
            let f_type = self.parse_type()?;
            let field_token = self.peek().clone();
            let f_name = self.consume_ident("Se esperaba nombre de campo")?;
            fields.push(FieldDecl {
                field_type: f_type.clone(),
                name: f_name,
                line: field_token.line,
                column: field_token.column,
                type_line: type_token.line,
                type_column: type_token.column,
                source_file: String::new(),
            });

            while self.match_token(&[TokenKind::Comma]) {
                let field_token = self.peek().clone();
                let next_name = self.consume_ident("Se esperaba nombre tras ','")?;
                fields.push(FieldDecl {
                    field_type: f_type.clone(),
                    name: next_name,
                    line: field_token.line,
                    column: field_token.column,
                    type_line: type_token.line,
                    type_column: type_token.column,
                    source_file: String::new(),
                });
            }
            self.skip_newlines();
        }

        self.consume(&TokenKind::End, "Se esperaba 'end' al cerrar record")?;
        Ok(RecordDecl {
            name,
            line: start.line,
            column: start.column,
            source_file: String::new(),
            fields,
        })
    }

    fn parse_enum(&mut self) -> Result<EnumDecl, String> {
        self.consume(&TokenKind::Enum, "Se esperaba 'enum'")?;
        let name = self.consume_ident("Se esperaba nombre de enum")?;
        self.skip_newlines();
        self.consume(&TokenKind::Begin, "Se esperaba 'begin' al inicio de enum")?;
        self.skip_newlines();

        let mut variants = Vec::new();
        variants.push(self.consume_ident("Se esperaba al menos una variante de enum")?);

        while self.match_token(&[TokenKind::Comma]) {
            self.skip_newlines();
            variants.push(self.consume_ident("Se esperaba nombre de variante tras ','")?);
        }

        self.skip_newlines();
        self.consume(&TokenKind::End, "Se esperaba 'end' al cerrar enum")?;
        Ok(EnumDecl { name, variants })
    }

    fn parse_extern_func(&mut self) -> Result<ExternFuncDecl, String> {
        let start = self.peek().clone();
        self.consume(&TokenKind::Extern, "Se esperaba 'extern'")?;
        self.consume(&TokenKind::Func, "Se esperaba 'func' tras 'extern'")?;
        let name = self.consume_ident("Se esperaba nombre de función externa")?;

        self.consume(&TokenKind::LParen, "Se esperaba '('")?;
        let mut params = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                let p_type = self.parse_type()?;
                let p_name = self.consume_ident("Se esperaba nombre de parámetro")?;
                params.push((p_type, p_name));
                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }
        }
        self.consume(&TokenKind::RParen, "Se esperaba ')'")?;

        let return_type = if self.match_token(&[TokenKind::Colon]) {
            Some(self.parse_type()?)
        } else {
            None
        };
        Ok(ExternFuncDecl {
            name,
            line: start.line,
            column: start.column,
            source_file: String::new(),
            params,
            return_type,
        })
    }

    fn parse_class(&mut self) -> Result<ClassDecl, String> {
        let start = self.peek().clone();
        self.consume(&TokenKind::Class, "Se esperaba 'class'")?;
        let name = self.consume_ident("Se esperaba nombre de clase")?;

        let parent = if self.match_token(&[TokenKind::Extends]) {
            Some(self.consume_ident("Se esperaba nombre de clase padre tras 'extends'")?)
        } else {
            None
        };

        self.skip_newlines();
        self.consume(&TokenKind::Begin, "Se esperaba 'begin' al inicio de clase")?;
        self.skip_newlines();

        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while !self.check(&TokenKind::End) && !self.is_at_end() {
            self.skip_newlines();

            // Detección de 'override func' o 'func' normal
            let is_override = self.match_token(&[TokenKind::Override]);

            if self.check(&TokenKind::Func) {
                methods.push(self.parse_function(is_override)?);
            } else if self.is_type_token(&self.peek().kind) || self.is_custom_type_start() {
                let type_token = self.peek().clone();
                let f_type = self.parse_type()?;
                let field_token = self.peek().clone();
                let f_name = self.consume_ident("Se esperaba nombre de atributo")?;
                fields.push(FieldDecl {
                    field_type: f_type.clone(),
                    name: f_name,
                    line: field_token.line,
                    column: field_token.column,
                    type_line: type_token.line,
                    type_column: type_token.column,
                    source_file: String::new(),
                });

                while self.match_token(&[TokenKind::Comma]) {
                    let field_token = self.peek().clone();
                    let next_name =
                        self.consume_ident("Se esperaba nombre de atributo tras ','")?;
                    fields.push(FieldDecl {
                        field_type: f_type.clone(),
                        name: next_name,
                        line: field_token.line,
                        column: field_token.column,
                        type_line: type_token.line,
                        type_column: type_token.column,
                        source_file: String::new(),
                    });
                }
            } else {
                return Err(format!(
                    "Declaración no válida dentro de la clase '{}'",
                    name
                ));
            }
            self.skip_newlines();
        }

        self.consume(&TokenKind::End, "Se esperaba 'end' al cerrar la clase")?;
        Ok(ClassDecl {
            name,
            line: start.line,
            column: start.column,
            source_file: String::new(),
            parent,
            fields,
            methods,
        })
    }

    fn parse_main_block(&mut self) -> Result<Vec<Stmt>, String> {
        self.consume(
            &TokenKind::Begin,
            "Se esperaba 'begin' al inicio del programa",
        )?;
        self.skip_newlines();

        let mut stmts = Vec::new();
        while !self.check_end_dot() && !self.is_at_end() {
            stmts.push(self.parse_stmt()?);
            self.skip_newlines();
        }

        self.consume(
            &TokenKind::End,
            "Se esperaba 'end' al final del programa principal",
        )?;
        self.consume(
            &TokenKind::Dot,
            "Se esperaba '.' tras 'end' para cerrar el programa",
        )?;
        Ok(stmts)
    }

    fn check_end_dot(&self) -> bool {
        if self.check(&TokenKind::End) {
            if let Some(next) = self.peek_next() {
                return next.kind == TokenKind::Dot;
            }
        }
        false
    }

    fn parse_function(&mut self, is_override: bool) -> Result<FunctionDecl, String> {
        let start = self.peek().clone();
        self.consume(&TokenKind::Func, "Se esperaba 'func'")?;
        let name = self.consume_ident("Se esperaba nombre de función")?;

        self.consume(&TokenKind::LParen, "Se esperaba '('")?;
        let mut params = Vec::new();

        if !self.check(&TokenKind::RParen) {
            loop {
                let p_type = self.parse_type()?;
                let p_name = self.consume_ident("Se esperaba nombre de parámetro")?;
                params.push((p_type, p_name));
                if !self.match_token(&[TokenKind::Comma]) {
                    break;
                }
            }
        }
        self.consume(&TokenKind::RParen, "Se esperaba ')'")?;

        let return_type = if self.match_token(&[TokenKind::Colon]) {
            Some(self.parse_type()?)
        } else {
            None
        };

        self.skip_newlines();
        self.consume(&TokenKind::Begin, "Se esperaba 'begin' en función")?;
        self.skip_newlines();

        let mut body = Vec::new();
        while !self.check(&TokenKind::End) && !self.is_at_end() {
            body.push(self.parse_stmt()?);
            self.skip_newlines();
        }
        self.consume(&TokenKind::End, "Se esperaba 'end' para cerrar función")?;

        Ok(FunctionDecl {
            name,
            is_override,
            line: start.line,
            column: start.column,
            source_file: String::new(),
            params,
            return_type,
            body,
        })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        self.skip_newlines();
        let start = self.peek().clone();
        let statement = self.parse_stmt_inner()?;
        Ok(Stmt::Located {
            source_file: String::new(),
            line: start.line,
            column: start.column,
            statement: Box::new(statement),
        })
    }

    fn parse_stmt_inner(&mut self) -> Result<Stmt, String> {
        self.skip_newlines();

        if self.match_token(&[TokenKind::Const]) {
            let (const_type, name) =
                if self.is_type_token(&self.peek().kind) || self.is_custom_type_start() {
                    let t = self.parse_type()?;
                    let n = self.consume_ident("Se esperaba nombre de constante")?;
                    (Some(t), n)
                } else {
                    let n = self.consume_ident("Se esperaba nombre de constante")?;
                    (None, n)
                };
            self.consume(&TokenKind::Assign, "Se esperaba '=' en constante")?;
            let value = self.parse_expr()?;
            return Ok(Stmt::ConstDecl {
                const_type,
                name,
                value,
            });
        }

        if self.is_type_token(&self.peek().kind) || self.is_custom_type_start() {
            let var_type = self.parse_type()?;
            let mut names = Vec::new();
            names.push(self.consume_ident("Se esperaba nombre de variable")?);
            while self.match_token(&[TokenKind::Comma]) {
                names.push(self.consume_ident("Se esperaba nombre de variable tras ','")?);
            }
            return Ok(Stmt::VarDecl { var_type, names });
        }

        if self.match_token(&[TokenKind::If]) {
            let condition = self.parse_expr()?;
            self.consume(&TokenKind::Then, "Se esperaba 'then'")?;
            self.skip_newlines();
            self.consume(&TokenKind::Begin, "Se esperaba 'begin' en bloque then")?;
            self.skip_newlines();
            let mut then_branch = Vec::new();
            while !self.check(&TokenKind::End) && !self.is_at_end() {
                then_branch.push(self.parse_stmt()?);
                self.skip_newlines();
            }
            self.consume(&TokenKind::End, "Se esperaba 'end' para cerrar bloque then")?;

            let mut else_branch = None;
            self.skip_newlines();
            if self.match_token(&[TokenKind::Else]) {
                self.skip_newlines();
                self.consume(&TokenKind::Begin, "Se esperaba 'begin' en bloque else")?;
                self.skip_newlines();
                let mut e_stmts = Vec::new();
                while !self.check(&TokenKind::End) && !self.is_at_end() {
                    e_stmts.push(self.parse_stmt()?);
                    self.skip_newlines();
                }
                self.consume(&TokenKind::End, "Se esperaba 'end' para cerrar bloque else")?;
                else_branch = Some(e_stmts);
            }
            return Ok(Stmt::If {
                condition,
                then_branch,
                else_branch,
            });
        }

        if self.match_token(&[TokenKind::While]) {
            let condition = self.parse_expr()?;
            self.consume(&TokenKind::Do, "Se esperaba 'do'")?;
            self.skip_newlines();
            self.consume(&TokenKind::Begin, "Se esperaba 'begin' en 'while'")?;
            self.skip_newlines();
            let mut body = Vec::new();
            while !self.check(&TokenKind::End) && !self.is_at_end() {
                body.push(self.parse_stmt()?);
                self.skip_newlines();
            }
            self.consume(&TokenKind::End, "Se esperaba 'end' para cerrar 'while'")?;
            return Ok(Stmt::While { condition, body });
        }

        if self.match_token(&[TokenKind::For]) {
            let var_name = self.consume_ident("Se esperaba nombre de variable tras 'for'")?;

            if self.match_token(&[TokenKind::In]) {
                let iterable = self.parse_expr()?;
                self.consume(&TokenKind::Do, "Se esperaba 'do'")?;
                self.skip_newlines();
                self.consume(&TokenKind::Begin, "Se esperaba 'begin' en 'for in'")?;
                self.skip_newlines();
                let mut body = Vec::new();
                while !self.check(&TokenKind::End) && !self.is_at_end() {
                    body.push(self.parse_stmt()?);
                    self.skip_newlines();
                }
                self.consume(&TokenKind::End, "Se esperaba 'end' para cerrar 'for in'")?;
                return Ok(Stmt::ForIn {
                    var_name,
                    iterable,
                    body,
                });
            }

            if self.match_token(&[TokenKind::Assign]) {
                let start = self.parse_expr()?;
                self.consume(&TokenKind::To, "Se esperaba 'to'")?;
                let end = self.parse_expr()?;
                self.consume(&TokenKind::Do, "Se esperaba 'do'")?;
                self.skip_newlines();
                self.consume(&TokenKind::Begin, "Se esperaba 'begin'")?;
                self.skip_newlines();
                let mut body = Vec::new();
                while !self.check(&TokenKind::End) && !self.is_at_end() {
                    body.push(self.parse_stmt()?);
                    self.skip_newlines();
                }
                self.consume(&TokenKind::End, "Se esperaba 'end' para cerrar 'for'")?;
                return Ok(Stmt::ForRange {
                    var_name,
                    start,
                    end,
                    body,
                });
            }

            return Err("Se esperaba '=' o 'in' en 'for'".to_string());
        }

        if self.match_token(&[TokenKind::Spawn]) {
            let func_name = self.consume_ident("Se esperaba nombre de función tras 'spawn'")?;
            self.consume(&TokenKind::LParen, "Se esperaba '('")?;
            self.consume(&TokenKind::RParen, "Se esperaba ')'")?;
            return Ok(Stmt::Spawn { func_name });
        }

        if self.match_token(&[TokenKind::Try]) {
            self.skip_newlines();
            self.consume(&TokenKind::Begin, "Se esperaba 'begin' tras 'try'")?;
            self.skip_newlines();
            let mut try_block = Vec::new();
            while !self.check(&TokenKind::End) && !self.is_at_end() {
                try_block.push(self.parse_stmt()?);
                self.skip_newlines();
            }
            self.consume(&TokenKind::End, "Se esperaba 'end' en 'try'")?;
            self.skip_newlines();

            self.consume(&TokenKind::Catch, "Se esperaba 'catch'")?;
            self.consume(&TokenKind::LParen, "Se esperaba '('")?;
            let catch_var = self.consume_ident("Se esperaba variable en 'catch'")?;
            self.consume(&TokenKind::RParen, "Se esperaba ')'")?;
            self.skip_newlines();

            self.consume(&TokenKind::Begin, "Se esperaba 'begin' en 'catch'")?;
            self.skip_newlines();
            let mut catch_block = Vec::new();
            while !self.check(&TokenKind::End) && !self.is_at_end() {
                catch_block.push(self.parse_stmt()?);
                self.skip_newlines();
            }
            self.consume(&TokenKind::End, "Se esperaba 'end' en 'catch'")?;
            return Ok(Stmt::TryCatch {
                try_block,
                catch_var,
                catch_block,
            });
        }

        if self.match_token(&[TokenKind::Throw]) {
            self.consume(&TokenKind::LParen, "Se esperaba '('")?;
            let err_expr = self.parse_expr()?;
            self.consume(&TokenKind::RParen, "Se esperaba ')'")?;
            return Ok(Stmt::Throw(err_expr));
        }

        if self.match_token(&[TokenKind::Return]) {
            if self.check(&TokenKind::Newline) || self.check(&TokenKind::End) {
                return Ok(Stmt::Return(None));
            }
            let val = self.parse_expr()?;
            return Ok(Stmt::Return(Some(val)));
        }

        let expr = self.parse_expr()?;

        if self.match_token(&[TokenKind::Assign]) {
            let value = self.parse_expr()?;
            match expr {
                Expr::Variable(name) => Ok(Stmt::Assign { name, value }),
                Expr::MemberAccess { object, member } => Ok(Stmt::MemberAssign {
                    object,
                    member,
                    value,
                }),
                Expr::IndexAccess { array, index } => Ok(Stmt::IndexAssign {
                    array,
                    index,
                    value,
                }),
                _ => Err("Destino de asignación no válido".to_string()),
            }
        } else {
            Ok(Stmt::Expression(expr))
        }
    }

    fn is_type_token(&self, kind: &TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::TypeInt
                | TokenKind::TypeFloat
                | TokenKind::TypeBool
                | TokenKind::TypeString
                | TokenKind::TypeChar
                | TokenKind::Array
                | TokenKind::Map
        )
    }

    fn is_custom_type_start(&self) -> bool {
        if let TokenKind::Ident(_) = &self.peek().kind {
            if let Some(next) = self.peek_next() {
                matches!(next.kind, TokenKind::Ident(_))
            } else {
                false
            }
        } else {
            false
        }
    }

    fn parse_type(&mut self) -> Result<Type, String> {
        let token = self.advance();
        match token.kind {
            TokenKind::TypeInt => Ok(Type::Int),
            TokenKind::TypeFloat => Ok(Type::Float),
            TokenKind::TypeBool => Ok(Type::Bool),
            TokenKind::TypeString => Ok(Type::String),
            TokenKind::TypeChar => Ok(Type::Char),
            TokenKind::Array => {
                self.consume(&TokenKind::LBracket, "Se esperaba '['")?;
                let inner = self.parse_type()?;
                self.consume(&TokenKind::RBracket, "Se esperaba ']'")?;
                Ok(Type::Array(Box::new(inner)))
            }
            TokenKind::Map => {
                self.consume(&TokenKind::LBracket, "Se esperaba '[' tras 'map'")?;
                let k_type = self.parse_type()?;
                self.consume(&TokenKind::Comma, "Se esperaba ',' separando clave y valor")?;
                let v_type = self.parse_type()?;
                self.consume(&TokenKind::RBracket, "Se esperaba ']' al cerrar 'map'")?;
                Ok(Type::Map(Box::new(k_type), Box::new(v_type)))
            }
            TokenKind::Ident(class_name) => Ok(Type::Custom(class_name)),
            _ => Err(format!("Se esperaba tipo de dato en línea {}", token.line)),
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_and()?;
        while self.match_token(&[TokenKind::Or]) {
            let right = self.parse_and()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::Or,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_bitor()?;
        while self.match_token(&[TokenKind::And]) {
            let right = self.parse_bitor()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::And,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_bitor(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_xor()?;
        while self.match_token(&[TokenKind::BitOr]) {
            let right = self.parse_xor()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::BitOr,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_xor(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_bitand()?;
        while self.match_token(&[TokenKind::Xor]) {
            let right = self.parse_bitand()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::Xor,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_bitand(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_equality()?;
        while self.match_token(&[TokenKind::BitAnd]) {
            let right = self.parse_equality()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op: BinaryOp::BitAnd,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_comparison()?;
        while let Some(op) = self.match_equality_op() {
            let right = self.parse_comparison()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_shift()?;
        while let Some(op) = self.match_comparison_op() {
            let right = self.parse_shift()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_shift(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_term()?;
        while let Some(op) = self.match_shift_op() {
            let right = self.parse_term()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_term(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_factor()?;
        while let Some(op) = self.match_term_op() {
            let right = self.parse_factor()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_factor(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_unary()?;
        while let Some(op) = self.match_factor_op() {
            let right = self.parse_unary()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                op,
                right: Box::new(right),
            };
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if self.match_token(&[TokenKind::Minus]) {
            let right = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Neg,
                right: Box::new(right),
            });
        }
        if self.match_token(&[TokenKind::Not]) {
            let right = self.parse_unary()?;
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                right: Box::new(right),
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primary()?;

        loop {
            if self.match_token(&[TokenKind::Dot]) {
                let member = self.consume_ident("Se esperaba nombre tras '.'")?;
                if self.match_token(&[TokenKind::LParen]) {
                    let mut args = Vec::new();
                    if !self.check(&TokenKind::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if !self.match_token(&[TokenKind::Comma]) {
                                break;
                            }
                        }
                    }
                    self.consume(&TokenKind::RParen, "Se esperaba ')' tras argumentos")?;
                    expr = Expr::MethodCall {
                        object: Box::new(expr),
                        method: member,
                        args,
                    };
                } else {
                    expr = Expr::MemberAccess {
                        object: Box::new(expr),
                        member,
                    };
                }
            } else if self.match_token(&[TokenKind::LBracket]) {
                let index = self.parse_expr()?;
                self.consume(&TokenKind::RBracket, "Se esperaba ']' tras índice")?;
                expr = Expr::IndexAccess {
                    array: Box::new(expr),
                    index: Box::new(index),
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        if self.match_token(&[TokenKind::New]) {
            if self.match_token(&[TokenKind::Array]) {
                self.consume(&TokenKind::LBracket, "Se esperaba '['")?;
                let element_type = self.parse_type()?;
                self.consume(&TokenKind::RBracket, "Se esperaba ']'")?;
                self.consume(&TokenKind::LParen, "Se esperaba '('")?;
                let size = self.parse_expr()?;
                self.consume(&TokenKind::RParen, "Se esperaba ')' tras tamaño")?;
                return Ok(Expr::NewArray {
                    element_type,
                    size: Box::new(size),
                });
            }

            if self.match_token(&[TokenKind::Map]) {
                self.consume(&TokenKind::LBracket, "Se esperaba '['")?;
                let key_type = self.parse_type()?;
                self.consume(&TokenKind::Comma, "Se esperaba ','")?;
                let val_type = self.parse_type()?;
                self.consume(&TokenKind::RBracket, "Se esperaba ']'")?;
                self.consume(&TokenKind::LParen, "Se esperaba '('")?;
                self.consume(&TokenKind::RParen, "Se esperaba ')'")?;
                return Ok(Expr::NewMap { key_type, val_type });
            }

            let class_name = self.consume_ident("Se esperaba nombre de clase tras 'new'")?;
            self.consume(&TokenKind::LParen, "Se esperaba '('")?;
            let mut args = Vec::new();
            if !self.check(&TokenKind::RParen) {
                loop {
                    args.push(self.parse_expr()?);
                    if !self.match_token(&[TokenKind::Comma]) {
                        break;
                    }
                }
            }
            self.consume(&TokenKind::RParen, "Se esperaba ')' tras argumentos")?;
            return Ok(Expr::New { class_name, args });
        }

        if self.match_token(&[TokenKind::LBracket]) {
            let mut elements = Vec::new();
            if !self.check(&TokenKind::RBracket) {
                loop {
                    elements.push(self.parse_expr()?);
                    if !self.match_token(&[TokenKind::Comma]) {
                        break;
                    }
                }
            }
            self.consume(&TokenKind::RBracket, "Se esperaba ']'")?;
            return Ok(Expr::ArrayLiteral(elements));
        }

        let token = self.advance();
        match token.kind {
            TokenKind::IntLiteral(val) => Ok(Expr::Integer(val)),
            TokenKind::FloatLiteral(val) => Ok(Expr::Float(val)),
            TokenKind::StringLiteral(val) => Ok(Expr::StringLit(val)),
            TokenKind::BoolLiteral(val) => Ok(Expr::Boolean(val)),
            TokenKind::Contains => {
                self.consume(&TokenKind::LParen, "Se esperaba '(' tras contains")?;
                let mut args = Vec::new();
                if !self.check(&TokenKind::RParen) {
                    loop {
                        args.push(self.parse_expr()?);
                        if !self.match_token(&[TokenKind::Comma]) {
                            break;
                        }
                    }
                }
                self.consume(&TokenKind::RParen, "Se esperaba ')' tras args de contains")?;
                Ok(Expr::Call {
                    callee: "contains".to_string(),
                    args,
                })
            }
            TokenKind::Ident(name) => {
                if self.match_token(&[TokenKind::LParen]) {
                    let mut args = Vec::new();
                    if !self.check(&TokenKind::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if !self.match_token(&[TokenKind::Comma]) {
                                break;
                            }
                        }
                    }
                    self.consume(&TokenKind::RParen, "Se esperaba ')' tras argumentos")?;
                    Ok(Expr::Call { callee: name, args })
                } else {
                    Ok(Expr::Variable(name))
                }
            }
            TokenKind::LParen => {
                let expr = self.parse_expr()?;
                self.consume(&TokenKind::RParen, "Se esperaba ')' tras expresión")?;
                Ok(expr)
            }
            _ => Err(format!("Expresión no válida con '{:?}'", token.kind)),
        }
    }

    fn match_equality_op(&mut self) -> Option<BinaryOp> {
        if self.match_token(&[TokenKind::EqualEqual]) {
            Some(BinaryOp::Equal)
        } else if self.match_token(&[TokenKind::NotEqual]) {
            Some(BinaryOp::NotEqual)
        } else {
            None
        }
    }

    fn match_comparison_op(&mut self) -> Option<BinaryOp> {
        if self.match_token(&[TokenKind::Less]) {
            Some(BinaryOp::Less)
        } else if self.match_token(&[TokenKind::Greater]) {
            Some(BinaryOp::Greater)
        } else if self.match_token(&[TokenKind::LessEqual]) {
            Some(BinaryOp::LessEq)
        } else if self.match_token(&[TokenKind::GreaterEqual]) {
            Some(BinaryOp::GreaterEq)
        } else {
            None
        }
    }

    fn match_shift_op(&mut self) -> Option<BinaryOp> {
        if self.match_token(&[TokenKind::Shl]) {
            Some(BinaryOp::Shl)
        } else if self.match_token(&[TokenKind::Shr]) {
            Some(BinaryOp::Shr)
        } else {
            None
        }
    }

    fn match_term_op(&mut self) -> Option<BinaryOp> {
        if self.match_token(&[TokenKind::Plus]) {
            Some(BinaryOp::Add)
        } else if self.match_token(&[TokenKind::Minus]) {
            Some(BinaryOp::Sub)
        } else {
            None
        }
    }

    fn match_factor_op(&mut self) -> Option<BinaryOp> {
        if self.match_token(&[TokenKind::Star]) {
            Some(BinaryOp::Mul)
        } else if self.match_token(&[TokenKind::Slash]) {
            Some(BinaryOp::Div)
        } else {
            None
        }
    }

    fn consume_ident(&mut self, err_msg: &str) -> Result<String, String> {
        let token = self.peek();
        if let TokenKind::Ident(name) = &token.kind {
            let name = name.clone();
            self.advance();
            Ok(name)
        } else {
            Err(format!(
                "{} en línea {}, columna {}",
                err_msg, token.line, token.column
            ))
        }
    }

    fn consume(&mut self, expected: &TokenKind, err_msg: &str) -> Result<Token, String> {
        if self.check(expected) {
            Ok(self.advance())
        } else {
            Err(format!(
                "{} (encontrado '{:?}') en línea {}",
                err_msg,
                self.peek().kind,
                self.peek().line
            ))
        }
    }

    fn match_token(&mut self, kinds: &[TokenKind]) -> bool {
        for kind in kinds {
            if self.check(kind) {
                self.advance();
                return true;
            }
        }
        false
    }

    fn check(&self, kind: &TokenKind) -> bool {
        if self.is_at_end() {
            return false;
        }
        &self.peek().kind == kind
    }

    fn advance(&mut self) -> Token {
        if !self.is_at_end() {
            self.current += 1;
        }
        self.previous()
    }

    fn is_at_end(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }
    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }
    fn peek_next(&self) -> Option<&Token> {
        if self.current + 1 < self.tokens.len() {
            Some(&self.tokens[self.current + 1])
        } else {
            None
        }
    }
    fn previous(&self) -> Token {
        self.tokens[self.current - 1].clone()
    }
    fn skip_newlines(&mut self) {
        while self.check(&TokenKind::Newline) {
            self.advance();
        }
    }
}
