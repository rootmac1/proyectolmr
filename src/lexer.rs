// src/lexer.rs

use crate::token::{Token, TokenKind, lookup_keyword};

pub struct Lexer {
    source: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
    tokens: Vec<Token>,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Self {
            source: source.chars().collect(),
            pos: 0,
            line: 1,
            column: 1,
            tokens: Vec::new(),
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        while !self.is_at_end() {
            let ch = self.current_char();

            match ch {
                // Espacios en blanco horizontales
                ' ' | '\t' | '\r' => {
                    self.advance();
                }

                // Saltos de línea (Regla estilo Go para fin de sentencia)
                '\n' => {
                    if self.should_insert_newline_token() {
                        self.tokens.push(Token::new(
                            TokenKind::Newline,
                            "\n",
                            self.line,
                            self.column,
                        ));
                    }
                    self.line += 1;
                    self.column = 1;
                    self.pos += 1;
                }

                // Comentarios o división
                '/' => {
                    if self.peek_next() == Some('/') {
                        // Comentario de una línea: saltar hasta el salto de línea
                        while !self.is_at_end() && self.current_char() != '\n' {
                            self.advance();
                        }
                    } else if self.peek_next() == Some('*') {
                        // Comentario multilínea /* ... */
                        self.advance(); // salta '/'
                        self.advance(); // salta '*'
                        while !self.is_at_end() {
                            if self.current_char() == '*' && self.peek_next() == Some('/') {
                                self.advance(); // salta '*'
                                self.advance(); // salta '/'
                                break;
                            }
                            if self.current_char() == '\n' {
                                self.line += 1;
                                self.column = 1;
                                self.pos += 1;
                            } else {
                                self.advance();
                            }
                        }
                    } else {
                        self.add_token(TokenKind::Slash, "/");
                    }
                }

                // Delimitadores y operadores de un caracter
                '+' => self.add_token(TokenKind::Plus, "+"),
                '-' => self.add_token(TokenKind::Minus, "-"),
                '*' => self.add_token(TokenKind::Star, "*"),
                '(' => self.add_token(TokenKind::LParen, "("),
                ')' => self.add_token(TokenKind::RParen, ")"),
                '[' => self.add_token(TokenKind::LBracket, "["),
                ']' => self.add_token(TokenKind::RBracket, "]"),
                ',' => self.add_token(TokenKind::Comma, ","),
                ':' => self.add_token(TokenKind::Colon, ":"),

                // Puntos (para números, módulos y 'end.')
                '.' => self.add_token(TokenKind::Dot, "."),

                // Asignación o igualdad
                '=' => {
                    if self.peek_next() == Some('=') {
                        let line = self.line;
                        let col = self.column;
                        self.advance();
                        self.advance();
                        self.tokens
                            .push(Token::new(TokenKind::EqualEqual, "==", line, col));
                    } else {
                        self.add_token(TokenKind::Assign, "=");
                    }
                }

                // Comparaciones: !=, <, <=, >, >=
                '!' => {
                    if self.peek_next() == Some('=') {
                        let line = self.line;
                        let col = self.column;
                        self.advance();
                        self.advance();
                        self.tokens
                            .push(Token::new(TokenKind::NotEqual, "!=", line, col));
                    } else {
                        return Err(format!(
                            "Carácter inesperado '!' en línea {}, col {}",
                            self.line, self.column
                        ));
                    }
                }
                '<' => {
                    if self.peek_next() == Some('=') {
                        let line = self.line;
                        let col = self.column;
                        self.advance();
                        self.advance();
                        self.tokens
                            .push(Token::new(TokenKind::LessEqual, "<=", line, col));
                    } else {
                        self.add_token(TokenKind::Less, "<");
                    }
                }
                '>' => {
                    if self.peek_next() == Some('=') {
                        let line = self.line;
                        let col = self.column;
                        self.advance();
                        self.advance();
                        self.tokens
                            .push(Token::new(TokenKind::GreaterEqual, ">=", line, col));
                    } else {
                        self.add_token(TokenKind::Greater, ">");
                    }
                }

                // Cadenas de texto
                '"' => self.scan_string()?,

                // Identificadores y palabras reservadas
                'a'..='z' | 'A'..='Z' | '_' => self.scan_identifier(),

                // Números enteros y flotantes
                '0'..='9' => self.scan_number()?,

                _ => {
                    return Err(format!(
                        "Carácter no reconocido '{}' en línea {}, col {}",
                        ch, self.line, self.column
                    ));
                }
            }
        }

        // Si el archivo no termina con salto de línea pero puede cerrarse, agregamos uno
        if self.should_insert_newline_token() {
            self.tokens
                .push(Token::new(TokenKind::Newline, "\n", self.line, self.column));
        }

        self.tokens
            .push(Token::new(TokenKind::Eof, "", self.line, self.column));
        Ok(self.tokens.clone())
    }

    // Regla de Go: inserta salto de línea solo si el último token cierra una expresión
    fn should_insert_newline_token(&self) -> bool {
        match self.tokens.last().map(|t| &t.kind) {
            Some(TokenKind::Ident(_))
            | Some(TokenKind::IntLiteral(_))
            | Some(TokenKind::FloatLiteral(_))
            | Some(TokenKind::StringLiteral(_))
            | Some(TokenKind::BoolLiteral(_))
            | Some(TokenKind::Return)
            | Some(TokenKind::End)
            | Some(TokenKind::Dot)
            | Some(TokenKind::RParen)
            | Some(TokenKind::RBracket) => true,
            _ => false,
        }
    }

    fn scan_string(&mut self) -> Result<(), String> {
        let start_line = self.line;
        let start_col = self.column;
        self.advance(); // salta comilla inicial

        let mut content = String::new();
        while !self.is_at_end() && self.current_char() != '"' {
            if self.current_char() == '\n' {
                return Err(format!(
                    "Cadena sin cerrar en línea {}, col {}",
                    start_line, start_col
                ));
            }

            // --- DETECTAR SECUENCIAS DE ESCAPE ---
            if self.current_char() == '\\' {
                self.advance(); // salta el '\'
                match self.current_char() {
                    'n' => content.push('\n'),
                    't' => content.push('\t'),
                    'r' => content.push('\r'),
                    '\"' => content.push('\"'),
                    '\\' => content.push('\\'),
                    'e' => content.push('\x1B'),
                    c => {
                        content.push('\\');
                        content.push(c);
                    }
                }
            } else {
                content.push(self.current_char());
            }
            self.advance();
        }

        if self.is_at_end() {
            return Err(format!(
                "Cadena sin cerrar en línea {}, col {}",
                start_line, start_col
            ));
        }

        self.advance(); // salta comilla de cierre
        self.tokens.push(Token::new(
            TokenKind::StringLiteral(content.clone()),
            format!("\"{}\"", content),
            start_line,
            start_col,
        ));
        Ok(())
    }

    fn scan_number(&mut self) -> Result<(), String> {
        let start_line = self.line;
        let start_col = self.column;
        let mut num_str = String::new();
        let mut is_float = false;

        while !self.is_at_end()
            && (self.current_char().is_ascii_digit() || self.current_char() == '.')
        {
            if self.current_char() == '.' {
                if is_float {
                    break;
                }
                // Si el siguiente carácter no es dígito, el punto es para otra cosa (como "end.")
                if let Some(next) = self.peek_next() {
                    if !next.is_ascii_digit() {
                        break;
                    }
                } else {
                    break;
                }
                is_float = true;
            }
            num_str.push(self.current_char());
            self.advance();
        }

        if is_float {
            let val = num_str.parse::<f64>().map_err(|e| e.to_string())?;
            self.tokens.push(Token::new(
                TokenKind::FloatLiteral(val),
                num_str,
                start_line,
                start_col,
            ));
        } else {
            let val = num_str.parse::<i64>().map_err(|e| e.to_string())?;
            self.tokens.push(Token::new(
                TokenKind::IntLiteral(val),
                num_str,
                start_line,
                start_col,
            ));
        }

        Ok(())
    }

    fn scan_identifier(&mut self) {
        let start_line = self.line;
        let start_col = self.column;
        let mut ident = String::new();

        while !self.is_at_end()
            && (self.current_char().is_ascii_alphanumeric() || self.current_char() == '_')
        {
            ident.push(self.current_char());
            self.advance();
        }

        // Revisamos si es palabra reservada
        let kind = match lookup_keyword(&ident) {
            Some(kw) => kw,
            None => TokenKind::Ident(ident.clone()),
        };

        self.tokens
            .push(Token::new(kind, ident, start_line, start_col));
    }

    fn add_token(&mut self, kind: TokenKind, lexeme: &str) {
        self.tokens
            .push(Token::new(kind, lexeme, self.line, self.column));
        self.advance();
    }

    fn current_char(&self) -> char {
        self.source[self.pos]
    }

    fn peek_next(&self) -> Option<char> {
        if self.pos + 1 < self.source.len() {
            Some(self.source[self.pos + 1])
        } else {
            None
        }
    }

    fn advance(&mut self) {
        self.pos += 1;
        self.column += 1;
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.source.len()
    }
}
