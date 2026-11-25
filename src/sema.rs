// src/sema.rs

use crate::ast::*;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct Symbol {
    pub var_type: Type,
    pub is_const: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub name: String,
    pub parent: Option<String>,
    pub fields: Vec<FieldDecl>,
    pub methods: HashMap<String, (Vec<Type>, Option<Type>)>,
}

pub struct SemanticAnalyzer {
    scopes: Vec<HashMap<String, Symbol>>,
    functions: HashMap<String, (Vec<Type>, Option<Type>)>,
    classes: HashMap<String, ClassInfo>,
    records: HashMap<String, Vec<FieldDecl>>,
    enums: HashMap<String, Vec<String>>,
    current_return_type: Option<Type>,
    current_class: Option<String>,
}

impl SemanticAnalyzer {
    pub fn new() -> Self {
        Self {
            scopes: Vec::new(),
            functions: HashMap::new(),
            classes: HashMap::new(),
            records: HashMap::new(),
            enums: HashMap::new(),
            current_return_type: None,
            current_class: None,
        }
    }

    fn is_assignable(&self, from: &Type, to: &Type) -> bool {
        if from == to {
            return true;
        }
        if let (Type::Custom(child), Type::Custom(parent)) = (from, to) {
            let mut curr = Some(child.clone());
            let mut visited = HashSet::new();
            while let Some(c) = curr {
                if &c == parent {
                    return true;
                }
                if !visited.insert(c.clone()) {
                    break;
                }
                if let Some(c_info) = self.classes.get(&c) {
                    curr = c_info.parent.clone();
                } else {
                    break;
                }
            }
        }
        false
    }

    fn is_numeric(t: &Type) -> bool {
        matches!(t, Type::Int | Type::Float)
    }

    fn at_location(error: String, line: usize, column: usize) -> String {
        if error.contains("línea ") && error.contains("columna ") {
            error
        } else {
            format!("{} (línea {}, columna {})", error, line, column)
        }
    }

    fn at_source_location(error: String, source_file: &str, line: usize, column: usize) -> String {
        if source_file.is_empty() {
            Self::at_location(error, line, column)
        } else if error.contains("archivo: ")
            && error.contains("línea ")
            && error.contains("columna ")
        {
            error
        } else {
            format!(
                "{} (archivo: {}, línea {}, columna {})",
                error, source_file, line, column
            )
        }
    }

    fn check_unique_fields(owner: &str, fields: &[FieldDecl]) -> Result<(), String> {
        let mut names = std::collections::HashSet::new();
        for field in fields {
            if !names.insert(&field.name) {
                return Err(Self::at_source_location(
                    format!("Campo '{}' duplicado en '{}'", field.name, owner),
                    &field.source_file,
                    field.line,
                    field.column,
                ));
            }
        }
        Ok(())
    }

    fn block_guarantees_return(statements: &[Stmt]) -> bool {
        statements.iter().any(Self::statement_guarantees_return)
    }

    fn statement_guarantees_return(stmt: &Stmt) -> bool {
        match stmt {
            Stmt::Located { statement, .. } => Self::statement_guarantees_return(statement),
            Stmt::Return(_) => true,
            Stmt::If {
                then_branch,
                else_branch: Some(else_branch),
                ..
            } => {
                Self::block_guarantees_return(then_branch)
                    && Self::block_guarantees_return(else_branch)
            }
            Stmt::TryCatch {
                try_block,
                catch_block,
                ..
            } => {
                Self::block_guarantees_return(try_block)
                    && Self::block_guarantees_return(catch_block)
            }
            _ => false,
        }
    }

    pub fn analyze(&mut self, program: &Program) -> Result<(), String> {
        self.enter_scope();

        for e in &program.enums {
            if self.enums.contains_key(&e.name) {
                return Err(format!("Enum '{}' ya declarado", e.name));
            }
            self.enums.insert(e.name.clone(), e.variants.clone());
            for v in &e.variants {
                self.declare_symbol(v, Type::Int, true)?;
            }
        }

        for r in &program.records {
            if self.records.contains_key(&r.name) || self.classes.contains_key(&r.name) {
                return Err(Self::at_source_location(
                    format!("Record '{}' ya declarado", r.name),
                    &r.source_file,
                    r.line,
                    r.column,
                ));
            }
            Self::check_unique_fields(&r.name, &r.fields)?;
            self.records.insert(r.name.clone(), r.fields.clone());
        }

        for ext in &program.extern_funcs {
            let p_types = ext.params.iter().map(|(t, _)| t.clone()).collect();
            if self
                .functions
                .insert(ext.name.clone(), (p_types, ext.return_type.clone()))
                .is_some()
            {
                return Err(Self::at_source_location(
                    format!("Función externa '{}' duplicada", ext.name),
                    &ext.source_file,
                    ext.line,
                    ext.column,
                ));
            }
        }

        for class in &program.classes {
            if self.classes.contains_key(&class.name) || self.records.contains_key(&class.name) {
                return Err(Self::at_source_location(
                    format!("Clase '{}' ya declarada", class.name),
                    &class.source_file,
                    class.line,
                    class.column,
                ));
            }
            Self::check_unique_fields(&class.name, &class.fields)?;
            let mut methods = HashMap::new();
            for m in &class.methods {
                let p_types = m.params.iter().map(|(t, _)| t.clone()).collect();
                if methods
                    .insert(m.name.clone(), (p_types, m.return_type.clone()))
                    .is_some()
                {
                    return Err(Self::at_source_location(
                        format!("Método '{}' duplicado en clase '{}'", m.name, class.name),
                        &m.source_file,
                        m.line,
                        m.column,
                    ));
                }
            }
            self.classes.insert(
                class.name.clone(),
                ClassInfo {
                    name: class.name.clone(),
                    parent: class.parent.clone(),
                    fields: class.fields.clone(),
                    methods,
                },
            );
        }

        for class in &program.classes {
            for method in &class.methods {
                if matches!(method.name.as_str(), "constructor" | "destructor") {
                    continue;
                }
                let inherited = class
                    .parent
                    .as_ref()
                    .and_then(|parent| self.find_method(parent, &method.name));
                match (method.is_override, inherited) {
                    (true, None) => {
                        return Err(Self::at_source_location(
                            format!(
                                "El método '{}' de '{}' está marcado override, pero no existe en la clase padre",
                                method.name, class.name
                            ),
                            &method.source_file,
                            method.line,
                            method.column,
                        ));
                    }
                    (false, Some(_)) => {
                        return Err(Self::at_source_location(
                            format!(
                                "El método '{}' de '{}' sobrescribe uno heredado; debe declararse override",
                                method.name, class.name
                            ),
                            &method.source_file,
                            method.line,
                            method.column,
                        ));
                    }
                    (true, Some((parent_params, parent_return))) => {
                        let method_params: Vec<Type> =
                            method.params.iter().map(|(t, _)| t.clone()).collect();
                        if method_params != parent_params || method.return_type != parent_return {
                            return Err(Self::at_source_location(
                                format!(
                                    "La firma del método override '{}.{}' no coincide con la clase padre",
                                    class.name, method.name
                                ),
                                &method.source_file,
                                method.line,
                                method.column,
                            ));
                        }
                    }
                    (false, None) => {}
                }
            }
        }

        for record in &program.records {
            for field in &record.fields {
                self.validate_type(&field.field_type).map_err(|e| {
                    Self::at_source_location(
                        e,
                        &field.source_file,
                        field.type_line,
                        field.type_column,
                    )
                })?;
            }
        }
        for class in &program.classes {
            for field in &class.fields {
                self.validate_type(&field.field_type).map_err(|e| {
                    Self::at_source_location(
                        e,
                        &field.source_file,
                        field.type_line,
                        field.type_column,
                    )
                })?;
            }
            for method in &class.methods {
                for (param_type, _) in &method.params {
                    self.validate_type(param_type)?;
                }
                if let Some(return_type) = &method.return_type {
                    self.validate_type(return_type)?;
                }
            }
        }
        for ext in &program.extern_funcs {
            for (param_type, _) in &ext.params {
                self.validate_type(param_type).map_err(|e| {
                    Self::at_source_location(e, &ext.source_file, ext.line, ext.column)
                })?;
            }
            if let Some(return_type) = &ext.return_type {
                self.validate_type(return_type).map_err(|e| {
                    Self::at_source_location(e, &ext.source_file, ext.line, ext.column)
                })?;
            }
        }

        let class_names: Vec<String> = self.classes.keys().cloned().collect();
        for c_name in class_names {
            let mut current_parent = self.classes.get(&c_name).and_then(|c| c.parent.clone());
            let mut visited_parents = HashSet::new();
            let mut inherited_fields = Vec::new();
            while let Some(ref p_name) = current_parent {
                if !visited_parents.insert(p_name.clone()) {
                    let class = program.classes.iter().find(|class| class.name == c_name);
                    let (file, line, column) = class
                        .map(|class| (class.source_file.as_str(), class.line, class.column))
                        .unwrap_or(("", 0, 0));
                    return Err(Self::at_source_location(
                        format!("Herencia circular detectada en la clase '{}'", c_name),
                        file,
                        line,
                        column,
                    ));
                }
                let p_info = match self.classes.get(p_name) {
                    Some(info) => info,
                    None => {
                        let class = program.classes.iter().find(|class| class.name == c_name);
                        let (file, line, column) = class
                            .map(|class| (class.source_file.as_str(), class.line, class.column))
                            .unwrap_or(("", 0, 0));
                        return Err(Self::at_source_location(
                            format!("Clase padre '{}' no existe", p_name),
                            file,
                            line,
                            column,
                        ));
                    }
                };
                for f in p_info.fields.iter().rev() {
                    if !inherited_fields
                        .iter()
                        .any(|field: &FieldDecl| field.name == f.name)
                    {
                        inherited_fields.insert(0, f.clone());
                    }
                }
                current_parent = p_info.parent.clone();
            }
            if let Some(c_info) = self.classes.get(&c_name) {
                for field in &c_info.fields {
                    if inherited_fields
                        .iter()
                        .any(|inherited: &FieldDecl| inherited.name == field.name)
                    {
                        return Err(Self::at_source_location(
                            format!(
                                "El campo '{}' de '{}' duplica un campo heredado",
                                field.name, c_name
                            ),
                            &field.source_file,
                            field.line,
                            field.column,
                        ));
                    }
                }
            }
            if !inherited_fields.is_empty() {
                if let Some(c_info) = self.classes.get_mut(&c_name) {
                    let original_fields = c_info.fields.clone();
                    inherited_fields.extend(original_fields);
                    c_info.fields = inherited_fields;
                }
            }
        }

        for func in &program.functions {
            if self.functions.contains_key(&func.name) {
                return Err(Self::at_source_location(
                    format!("Función '{}' ya declarada", func.name),
                    &func.source_file,
                    func.line,
                    func.column,
                ));
            }
            let param_types = func.params.iter().map(|(t, _)| t.clone()).collect();
            self.functions
                .insert(func.name.clone(), (param_types, func.return_type.clone()));
        }

        for class in &program.classes {
            self.current_class = Some(class.name.clone());
            for method in &class.methods {
                self.analyze_function(method)?;
            }
            self.current_class = None;
        }

        for func in &program.functions {
            self.analyze_function(func)?;
        }
        for stmt in &program.main_block {
            self.analyze_stmt(stmt)?;
        }

        self.exit_scope();
        Ok(())
    }

    fn analyze_function(&mut self, func: &FunctionDecl) -> Result<(), String> {
        self.analyze_function_inner(func).map_err(|error| {
            Self::at_source_location(error, &func.source_file, func.line, func.column)
        })
    }

    fn analyze_function_inner(&mut self, func: &FunctionDecl) -> Result<(), String> {
        self.enter_scope();
        self.current_return_type = func.return_type.clone();
        for (p_type, p_name) in &func.params {
            self.validate_type(p_type)?;
            self.declare_symbol(p_name, p_type.clone(), false)?;
        }
        if let Some(return_type) = &func.return_type {
            self.validate_type(return_type)?;
        }
        if func.return_type.is_some() && !Self::block_guarantees_return(&func.body) {
            return Err(format!(
                "La función '{}' puede terminar sin devolver un valor",
                func.name
            ));
        }
        for stmt in &func.body {
            self.analyze_stmt(stmt)?;
        }
        self.current_return_type = None;
        self.exit_scope();
        Ok(())
    }

    fn analyze_stmt(&mut self, stmt: &Stmt) -> Result<(), String> {
        match stmt {
            Stmt::Located {
                source_file,
                line,
                column,
                statement,
            } => {
                let result = self.analyze_stmt(statement);
                result.map_err(|error| Self::at_source_location(error, source_file, *line, *column))
            }
            Stmt::VarDecl { var_type, names } => {
                self.validate_type(var_type)?;
                for name in names {
                    self.declare_symbol(name, var_type.clone(), false)?;
                }
                Ok(())
            }
            Stmt::ConstDecl {
                const_type,
                name,
                value,
            } => {
                let expr_type = self.typecheck_expr(value)?;
                if let Some(expected_t) = const_type {
                    self.validate_type(expected_t)?;
                    if !self.is_assignable(&expr_type, expected_t) {
                        return Err(format!("Constante '{}' tipo incompatible", name));
                    }
                }
                self.declare_symbol(name, expr_type, true)?;
                Ok(())
            }
            Stmt::Assign { name, value } => {
                let value_type = self.typecheck_expr(value)?;
                if let Some(symbol) = self.lookup_symbol(name) {
                    if symbol.is_const {
                        return Err(format!("No se puede reasignar constante '{}'", name));
                    }
                    if !self.is_assignable(&value_type, &symbol.var_type) {
                        return Err(format!(
                            "No se puede asignar {:?} a variable '{}' de tipo {:?}",
                            value_type, name, symbol.var_type
                        ));
                    }
                    return Ok(());
                }
                if let Some(ref class_name) = self.current_class {
                    if let Some(c_info) = self.classes.get(class_name) {
                        for field in &c_info.fields {
                            if field.name == *name {
                                if !self.is_assignable(&value_type, &field.field_type) {
                                    return Err(format!("Campo '{}' tipo incompatible", name));
                                }
                                return Ok(());
                            }
                        }
                    }
                }
                Err(format!("Variable '{}' no declarada", name))
            }
            Stmt::MemberAssign {
                object,
                member,
                value,
            } => {
                let obj_type = self.typecheck_expr(object)?;
                let val_type = self.typecheck_expr(value)?;
                match obj_type {
                    Type::Custom(name) => {
                        if let Some(c_info) = self.classes.get(&name) {
                            for field in &c_info.fields {
                                if field.name == *member {
                                    if !self.is_assignable(&val_type, &field.field_type) {
                                        return Err("Tipo incompatible en campo".to_string());
                                    }
                                    return Ok(());
                                }
                            }
                        } else if let Some(r_fields) = self.records.get(&name) {
                            for field in r_fields {
                                if field.name == *member {
                                    if !self.is_assignable(&val_type, &field.field_type) {
                                        return Err("Tipo incompatible en record".to_string());
                                    }
                                    return Ok(());
                                }
                            }
                        }
                        Err(format!("Campo '{}' no existe en '{}'", member, name))
                    }
                    _ => Err("Acceso a miembro en tipo no compuesto".to_string()),
                }
            }
            Stmt::IndexAssign {
                array,
                index,
                value,
            } => {
                let arr_type = self.typecheck_expr(array)?;
                let idx_type = self.typecheck_expr(index)?;
                let val_type = self.typecheck_expr(value)?;

                match arr_type {
                    Type::Array(inner) => {
                        if idx_type != Type::Int {
                            return Err("Índice de array debe ser int".to_string());
                        }
                        if !self.is_assignable(&val_type, &inner) {
                            return Err("Tipo incompatible para elemento de array".to_string());
                        }
                        Ok(())
                    }
                    Type::Map(k_type, v_type) => {
                        if idx_type != *k_type {
                            return Err(format!("La clave del map debe ser {:?}", k_type));
                        }
                        if !self.is_assignable(&val_type, &v_type) {
                            return Err(format!("Valor del map debe ser {:?}", v_type));
                        }
                        Ok(())
                    }
                    _ => Err("Tipo no indexable".to_string()),
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond_type = self.typecheck_expr(condition)?;
                if cond_type != Type::Bool {
                    return Err("Condición if debe ser bool".to_string());
                }
                self.enter_scope();
                for s in then_branch {
                    self.analyze_stmt(s)?;
                }
                self.exit_scope();
                if let Some(else_stmts) = else_branch {
                    self.enter_scope();
                    for s in else_stmts {
                        self.analyze_stmt(s)?;
                    }
                    self.exit_scope();
                }
                Ok(())
            }
            Stmt::While { condition, body } => {
                let cond_t = self.typecheck_expr(condition)?;
                if cond_t != Type::Bool {
                    return Err("Condición while debe ser bool".to_string());
                }
                self.enter_scope();
                for s in body {
                    self.analyze_stmt(s)?;
                }
                self.exit_scope();
                Ok(())
            }
            Stmt::ForRange {
                var_name,
                start,
                end,
                body,
            } => {
                let start_t = self.typecheck_expr(start)?;
                let end_t = self.typecheck_expr(end)?;
                if start_t != Type::Int || end_t != Type::Int {
                    return Err("Límites for deben ser int".to_string());
                }
                self.enter_scope();
                self.declare_symbol(var_name, Type::Int, false)?;
                for s in body {
                    self.analyze_stmt(s)?;
                }
                self.exit_scope();
                Ok(())
            }
            Stmt::ForIn {
                var_name,
                iterable,
                body,
            } => {
                let iter_t = self.typecheck_expr(iterable)?;
                let elem_t = match iter_t {
                    Type::Array(inner) => *inner,
                    _ => return Err("for in requiere array".to_string()),
                };
                self.enter_scope();
                self.declare_symbol(var_name, elem_t, false)?;
                for s in body {
                    self.analyze_stmt(s)?;
                }
                self.exit_scope();
                Ok(())
            }
            Stmt::Spawn { func_name } => {
                let func_info = self.functions.get(func_name).cloned();
                if let Some((param_types, _)) = func_info {
                    if !param_types.is_empty() {
                        return Err(format!(
                            "'{}' en spawn no debe recibir argumentos",
                            func_name
                        ));
                    }
                    Ok(())
                } else {
                    Err(format!("Función '{}' no encontrada para spawn", func_name))
                }
            }
            Stmt::TryCatch {
                try_block,
                catch_var,
                catch_block,
            } => {
                self.enter_scope();
                for s in try_block {
                    self.analyze_stmt(s)?;
                }
                self.exit_scope();
                self.enter_scope();
                self.declare_symbol(catch_var, Type::String, false)?;
                for s in catch_block {
                    self.analyze_stmt(s)?;
                }
                self.exit_scope();
                Ok(())
            }
            Stmt::Throw(err_expr) => {
                let t = self.typecheck_expr(err_expr)?;
                if t != Type::String {
                    return Err("throw requiere string".to_string());
                }
                Ok(())
            }
            Stmt::Return(maybe_expr) => {
                let expr_type = match maybe_expr {
                    Some(e) => Some(self.typecheck_expr(e)?),
                    None => None,
                };
                if let (Some(ret_t), Some(expected_t)) = (&expr_type, &self.current_return_type) {
                    if !self.is_assignable(ret_t, expected_t) {
                        return Err("Retorno incompatible".to_string());
                    }
                } else if expr_type != self.current_return_type {
                    return Err("Retorno incompatible".to_string());
                }
                Ok(())
            }
            Stmt::Expression(expr) => self.analyze_statement_expression(expr),
        }
    }

    fn analyze_statement_expression(&mut self, expr: &Expr) -> Result<(), String> {
        match expr {
            Expr::Call { callee, args } => {
                if callee == "write" || callee == "writeln" {
                    for arg in args {
                        self.typecheck_expr(arg)?;
                    }
                    return Ok(());
                }
                if matches!(
                    callee.as_str(),
                    "len"
                        | "toUpper"
                        | "toLower"
                        | "SliceString"
                        | "posString"
                        | "intToString"
                        | "floatToString"
                        | "readln"
                        | "readFile"
                        | "writeFile"
                        | "sqrt"
                        | "pow"
                        | "abs"
                        | "sin"
                        | "cos"
                        | "tan"
                        | "asin"
                        | "acos"
                        | "atan"
                        | "atan2"
                        | "exp"
                        | "log"
                        | "log10"
                        | "floor"
                        | "ceil"
                        | "round"
                        | "fmod"
                        | "dispose"
                        | "contains"
                        | "sleep"
                        | "jsonParse"
                ) {
                    self.typecheck_expr(expr)?;
                    return Ok(());
                }

                let func_info = self.functions.get(callee).cloned();
                if let Some((param_types, _)) = func_info {
                    if args.len() != param_types.len() {
                        return Err(format!(
                            "'{}' espera {} argumentos",
                            callee,
                            param_types.len()
                        ));
                    }
                    for (i, (arg, p_type)) in args.iter().zip(param_types.iter()).enumerate() {
                        let a_type = self.typecheck_expr(arg)?;
                        if !self.is_assignable(&a_type, p_type) {
                            return Err(format!("Arg {} de '{}' incompatible", i + 1, callee));
                        }
                    }
                    return Ok(());
                }

                let current_class = self.current_class.clone();
                if let Some(ref class_name) = current_class {
                    let method_info = self.find_method(class_name, callee);
                    if let Some((param_types, _)) = method_info {
                        if args.len() != param_types.len() {
                            return Err(format!(
                                "Método '{}' espera {} argumentos",
                                callee,
                                param_types.len()
                            ));
                        }
                        for (i, (arg, p_type)) in args.iter().zip(param_types.iter()).enumerate() {
                            let a_type = self.typecheck_expr(arg)?;
                            if !self.is_assignable(&a_type, p_type) {
                                return Err(format!("Arg {} de '{}' incompatible", i + 1, callee));
                            }
                        }
                        return Ok(());
                    }
                }

                Err(format!("Función o método '{}' no declarado", callee))
            }
            Expr::MethodCall {
                object,
                method,
                args,
            } => {
                let obj_type = self.typecheck_expr(object)?;
                match obj_type {
                    Type::Custom(class_name) => {
                        let method_info = self.find_method(&class_name, method);
                        let (param_types, _) = method_info.ok_or_else(|| {
                            format!("Método '{}' no existe en clase '{}'", method, class_name)
                        })?;
                        if args.len() != param_types.len() {
                            return Err("Argumentos incorrectos".to_string());
                        }
                        for (arg, p_type) in args.iter().zip(param_types.iter()) {
                            let a_type = self.typecheck_expr(arg)?;
                            if !self.is_assignable(&a_type, p_type) {
                                return Err("Tipo de argumento incompatible en método".to_string());
                            }
                        }
                        Ok(())
                    }
                    _ => Err("Llamada a método solo en objetos".to_string()),
                }
            }
            _ => {
                self.typecheck_expr(expr)?;
                Ok(())
            }
        }
    }

    fn find_method(&self, class_name: &str, method: &str) -> Option<(Vec<Type>, Option<Type>)> {
        let mut curr = Some(class_name.to_string());
        let mut visited = HashSet::new();
        while let Some(c) = curr {
            if !visited.insert(c.clone()) {
                return None;
            }
            if let Some(c_info) = self.classes.get(&c) {
                if let Some(m) = c_info.methods.get(method) {
                    return Some(m.clone());
                }
                curr = c_info.parent.clone();
            } else {
                break;
            }
        }
        None
    }

    fn typecheck_expr(&mut self, expr: &Expr) -> Result<Type, String> {
        match expr {
            Expr::Integer(_) => Ok(Type::Int),
            Expr::Float(_) => Ok(Type::Float),
            Expr::StringLit(_) => Ok(Type::String),
            Expr::Boolean(_) => Ok(Type::Bool),
            Expr::Variable(name) => {
                if let Some(s) = self.lookup_symbol(name) {
                    return Ok(s.var_type.clone());
                }
                if let Some(ref class_name) = self.current_class {
                    if let Some(c_info) = self.classes.get(class_name) {
                        for field in &c_info.fields {
                            if field.name == *name {
                                return Ok(field.field_type.clone());
                            }
                        }
                    }
                }
                Err(format!("Variable '{}' no declarada", name))
            }
            Expr::ArrayLiteral(elements) => {
                if elements.is_empty() {
                    return Err("Arreglo literal vacío requiere tipo explícito".to_string());
                }
                let first_t = self.typecheck_expr(&elements[0])?;
                for (i, el) in elements.iter().enumerate().skip(1) {
                    let el_t = self.typecheck_expr(el)?;
                    if el_t != first_t {
                        return Err(format!(
                            "Elemento {} del arreglo tiene tipo {:?}, pero se esperaba {:?}",
                            i + 1,
                            el_t,
                            first_t
                        ));
                    }
                }
                Ok(Type::Array(Box::new(first_t)))
            }
            Expr::NewArray { element_type, size } => {
                self.validate_type(element_type)?;
                let sz_t = self.typecheck_expr(size)?;
                if sz_t != Type::Int {
                    return Err("Tamaño debe ser int".to_string());
                }
                Ok(Type::Array(Box::new(element_type.clone())))
            }
            Expr::NewMap { key_type, val_type } => {
                self.validate_type(key_type)?;
                self.validate_type(val_type)?;
                Ok(Type::Map(
                    Box::new(key_type.clone()),
                    Box::new(val_type.clone()),
                ))
            }
            Expr::IndexAccess { array, index } => {
                let arr_t = self.typecheck_expr(array)?;
                let idx_t = self.typecheck_expr(index)?;
                match arr_t {
                    Type::Array(inner) => {
                        if idx_t != Type::Int {
                            return Err("Índice de array es int".to_string());
                        }
                        Ok(*inner)
                    }
                    Type::Map(k, v) => {
                        if idx_t != *k {
                            return Err("Clave de map incorrecta".to_string());
                        }
                        Ok(*v)
                    }
                    Type::String => Ok(Type::String),
                    _ => Err("Tipo no indexable".to_string()),
                }
            }
            Expr::New { class_name, args } => {
                if !self.classes.contains_key(class_name) {
                    return Err(format!("Clase '{}' no declarada", class_name));
                }
                if let Some((param_types, _)) = self.find_method(class_name, "constructor") {
                    if args.len() != param_types.len() {
                        return Err(format!(
                            "El constructor de '{}' espera {} argumentos",
                            class_name,
                            param_types.len()
                        ));
                    }
                    for (arg, p_type) in args.iter().zip(param_types.iter()) {
                        let a_type = self.typecheck_expr(arg)?;
                        if !self.is_assignable(&a_type, p_type) {
                            return Err("Arg constructor incompatible".to_string());
                        }
                    }
                } else if !args.is_empty() {
                    return Err(format!(
                        "La clase '{}' no tiene constructor con argumentos",
                        class_name
                    ));
                }
                Ok(Type::Custom(class_name.clone()))
            }
            Expr::MemberAccess { object, member } => {
                let obj_type = self.typecheck_expr(object)?;
                if let Type::Custom(name) = obj_type {
                    if let Some(c_info) = self.classes.get(&name) {
                        for field in &c_info.fields {
                            if field.name == *member {
                                return Ok(field.field_type.clone());
                            }
                        }
                    } else if let Some(r_fields) = self.records.get(&name) {
                        for field in r_fields {
                            if field.name == *member {
                                return Ok(field.field_type.clone());
                            }
                        }
                    }
                    Err(format!("Campo '{}' no existe en '{}'", member, name))
                } else {
                    Err("Acceso a miembro solo en clases o records".to_string())
                }
            }
            Expr::MethodCall {
                object,
                method,
                args,
            } => {
                let obj_type = self.typecheck_expr(object)?;
                match obj_type {
                    Type::Custom(class_name) => {
                        let method_info = self.find_method(&class_name, method);
                        let (param_types, ret_type) = method_info.ok_or_else(|| {
                            format!("Método '{}' no existe en clase '{}'", method, class_name)
                        })?;
                        if args.len() != param_types.len() {
                            return Err(format!(
                                "Método '{}' espera {} argumentos",
                                method,
                                param_types.len()
                            ));
                        }
                        for (arg, p_type) in args.iter().zip(param_types.iter()) {
                            let a_type = self.typecheck_expr(arg)?;
                            if !self.is_assignable(&a_type, p_type) {
                                return Err("Tipo incorrecto en método".to_string());
                            }
                        }
                        match ret_type {
                            Some(t) => Ok(t.clone()),
                            None => Err("Método no retorna valor".to_string()),
                        }
                    }
                    _ => Err("Llamada a método solo en objetos".to_string()),
                }
            }
            Expr::Unary { op, right } => {
                let r_type = self.typecheck_expr(right)?;
                match op {
                    UnaryOp::Neg if Self::is_numeric(&r_type) => Ok(r_type),
                    UnaryOp::Neg => Err(format!(
                        "El operador unario '-' requiere int o float, recibió {:?}",
                        r_type
                    )),
                    UnaryOp::Not if r_type == Type::Bool => Ok(Type::Bool),
                    UnaryOp::Not => Err(format!("'not' requiere bool, recibió {:?}", r_type)),
                }
            }
            Expr::Binary {
                op: BinaryOp::Add, ..
            } => self.typecheck_add_chain(expr),
            Expr::Binary { left, op, right } => {
                let l_type = self.typecheck_expr(left)?;
                let r_type = self.typecheck_expr(right)?;
                match op {
                    BinaryOp::Add
                        if l_type == r_type
                            && (Self::is_numeric(&l_type) || l_type == Type::String) =>
                    {
                        Ok(l_type)
                    }
                    BinaryOp::Add => Err(format!(
                        "'+' no admite operandos {:?} y {:?}",
                        l_type, r_type
                    )),
                    BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div
                        if l_type == r_type && Self::is_numeric(&l_type) =>
                    {
                        Ok(l_type)
                    }
                    BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => Err(format!(
                        "El operador {:?} requiere dos operandos int o float del mismo tipo; recibió {:?} y {:?}",
                        op, l_type, r_type
                    )),
                    BinaryOp::Shl
                    | BinaryOp::Shr
                    | BinaryOp::Xor
                    | BinaryOp::BitAnd
                    | BinaryOp::BitOr => {
                        if l_type == Type::Int && r_type == Type::Int {
                            Ok(Type::Int)
                        } else {
                            Err("Los operadores bitwise requieren tipo int".to_string())
                        }
                    }
                    BinaryOp::And | BinaryOp::Or
                        if l_type == Type::Bool && r_type == Type::Bool =>
                    {
                        Ok(Type::Bool)
                    }
                    BinaryOp::And | BinaryOp::Or => Err(format!(
                        "Los operadores lógicos requieren dos bool; recibieron {:?} y {:?}",
                        l_type, r_type
                    )),
                    BinaryOp::Equal | BinaryOp::NotEqual
                        if l_type == r_type
                            && matches!(
                                l_type,
                                Type::Int
                                    | Type::Float
                                    | Type::Bool
                                    | Type::String
                                    | Type::Char
                                    | Type::Custom(_)
                            ) =>
                    {
                        Ok(Type::Bool)
                    }
                    BinaryOp::Equal | BinaryOp::NotEqual => Err(format!(
                        "No se pueden comparar {:?} y {:?} con igualdad",
                        l_type, r_type
                    )),
                    BinaryOp::Less | BinaryOp::Greater | BinaryOp::LessEq | BinaryOp::GreaterEq
                        if l_type == r_type && Self::is_numeric(&l_type) =>
                    {
                        Ok(Type::Bool)
                    }
                    BinaryOp::Less | BinaryOp::Greater | BinaryOp::LessEq | BinaryOp::GreaterEq => {
                        Err(format!(
                            "La comparación de orden requiere dos int o dos float; recibió {:?} y {:?}",
                            l_type, r_type
                        ))
                    }
                }
            }
            Expr::Call { callee, args } => {
                if callee == "len" {
                    if args.len() != 1 {
                        return Err("'len' requiere exactamente 1 argumento".to_string());
                    }
                    let arg_type = self.typecheck_expr(&args[0])?;
                    if !matches!(arg_type, Type::String | Type::Array(_) | Type::Map(_, _)) {
                        return Err("'len' requiere string, array o map".to_string());
                    }
                    return Ok(Type::Int);
                }
                if callee == "contains" {
                    if args.len() != 2 {
                        return Err("'contains' requiere exactamente 2 argumentos".to_string());
                    }
                    let map_t = self.typecheck_expr(&args[0])?;
                    let key_t = self.typecheck_expr(&args[1])?;
                    if let Type::Map(k, _) = map_t {
                        if *k != key_t {
                            return Err("Tipo de clave incorrecto en contains".to_string());
                        }
                        return Ok(Type::Bool);
                    }
                    return Err("contains requiere un map".to_string());
                }
                if callee == "jsonParse" {
                    if args.len() != 1 {
                        return Err("'jsonParse' requiere 1 argumento (string)".to_string());
                    }
                    let t = self.typecheck_expr(&args[0])?;
                    if t != Type::String {
                        return Err("'jsonParse' requiere tipo string".to_string());
                    }
                    return Ok(Type::Map(Box::new(Type::String), Box::new(Type::String)));
                }
                if callee == "intToString" || callee == "floatToString" {
                    if args.len() != 1 {
                        return Err(format!("'{}' requiere 1 argumento", callee));
                    }
                    let arg_type = self.typecheck_expr(&args[0])?;
                    if !Self::is_numeric(&arg_type) {
                        return Err(format!("'{}' requiere int o float", callee));
                    }
                    return Ok(Type::String);
                }
                if callee == "toUpper" || callee == "toLower" {
                    if args.len() != 1 || self.typecheck_expr(&args[0])? != Type::String {
                        return Err(format!("'{}' requiere 1 argumento string", callee));
                    }
                    return Ok(Type::String);
                }
                if callee == "SliceString" {
                    if args.len() != 2 && args.len() != 3 {
                        return Err("'SliceString' requiere 2 o 3 argumentos".to_string());
                    }
                    if self.typecheck_expr(&args[0])? != Type::String
                        || self.typecheck_expr(&args[1])? != Type::Int
                        || (args.len() == 3 && self.typecheck_expr(&args[2])? != Type::Int)
                    {
                        return Err("'SliceString' requiere (string, int[, int])".to_string());
                    }
                    return Ok(Type::String);
                }
                if callee == "posString" {
                    if args.len() != 2
                        || self.typecheck_expr(&args[0])? != Type::String
                        || self.typecheck_expr(&args[1])? != Type::String
                    {
                        return Err("'posString' requiere 2 argumentos string".to_string());
                    }
                    return Ok(Type::Int);
                }
                if callee == "readFile" {
                    if args.len() != 1 || self.typecheck_expr(&args[0])? != Type::String {
                        return Err("'readFile' requiere una ruta string".to_string());
                    }
                    return Ok(Type::String);
                }
                if callee == "writeFile" {
                    if args.len() != 2
                        || self.typecheck_expr(&args[0])? != Type::String
                        || self.typecheck_expr(&args[1])? != Type::String
                    {
                        return Err("'writeFile' requiere ruta y contenido string".to_string());
                    }
                    return Ok(Type::Bool);
                }
                if callee == "readln" {
                    if args.is_empty() {
                        return Ok(Type::String);
                    }
                    if args.len() == 1
                        && matches!(args[0], Expr::Variable(_))
                        && matches!(
                            self.typecheck_expr(&args[0])?,
                            Type::Int | Type::Float | Type::String
                        )
                    {
                        return Ok(Type::String);
                    }
                    return Err(
                        "'readln' acepta cero argumentos o una variable int, float o string"
                            .to_string(),
                    );
                }
                if callee == "dispose" {
                    if args.len() != 1 || !matches!(args[0], Expr::Variable(_)) {
                        return Err("'dispose' requiere una variable administrada".to_string());
                    }
                    let t = self.typecheck_expr(&args[0])?;
                    if !matches!(t, Type::Array(_) | Type::Map(_, _) | Type::Custom(_)) {
                        return Err("'dispose' solo acepta objetos, arrays o maps".to_string());
                    }
                    return Ok(Type::Bool);
                }
                if callee == "sleep" {
                    if args.len() != 1 {
                        return Err("'sleep' requiere 1 argumento (int)".to_string());
                    }
                    let t = self.typecheck_expr(&args[0])?;
                    if t != Type::Int {
                        return Err("'sleep' requiere int (milisegundos)".to_string());
                    }
                    return Ok(Type::Int);
                }
                if callee == "sqrt" || callee == "abs" {
                    if args.len() != 1 {
                        return Err(format!("'{}' requiere 1 argumento", callee));
                    }
                    let t = self.typecheck_expr(&args[0])?;
                    if !Self::is_numeric(&t) {
                        return Err(format!("'{}' requiere int o float", callee));
                    }
                    return Ok(if callee == "abs" { t } else { Type::Float });
                }
                if matches!(
                    callee.as_str(),
                    "sin"
                        | "cos"
                        | "tan"
                        | "asin"
                        | "acos"
                        | "atan"
                        | "exp"
                        | "log"
                        | "log10"
                        | "floor"
                        | "ceil"
                        | "round"
                ) {
                    if args.len() != 1 || !Self::is_numeric(&self.typecheck_expr(&args[0])?) {
                        return Err(format!("'{}' requiere 1 argumento numérico", callee));
                    }
                    return Ok(Type::Float);
                }
                if callee == "atan2" || callee == "fmod" {
                    if args.len() != 2
                        || !Self::is_numeric(&self.typecheck_expr(&args[0])?)
                        || !Self::is_numeric(&self.typecheck_expr(&args[1])?)
                    {
                        return Err(format!("'{}' requiere 2 argumentos numéricos", callee));
                    }
                    return Ok(Type::Float);
                }
                if callee == "pow" {
                    if args.len() != 2
                        || !Self::is_numeric(&self.typecheck_expr(&args[0])?)
                        || !Self::is_numeric(&self.typecheck_expr(&args[1])?)
                    {
                        return Err("'pow' requiere 2 argumentos numéricos".to_string());
                    }
                    return Ok(Type::Float);
                }

                let func_info = self.functions.get(callee).cloned();
                if let Some((param_types, ret_type)) = func_info {
                    if args.len() != param_types.len() {
                        return Err(format!(
                            "'{}' espera {} argumentos",
                            callee,
                            param_types.len()
                        ));
                    }
                    for (i, (arg, p_type)) in args.iter().zip(param_types.iter()).enumerate() {
                        let a_type = self.typecheck_expr(arg)?;
                        if !self.is_assignable(&a_type, p_type) {
                            return Err(format!("Arg {} en '{}' incompatible", i + 1, callee));
                        }
                    }
                    return match ret_type {
                        Some(t) => Ok(t),
                        None => Err(format!("'{}' no retorna valor", callee)),
                    };
                }

                let current_class = self.current_class.clone();
                if let Some(ref class_name) = current_class {
                    let method_info = self.find_method(class_name, callee);
                    if let Some((param_types, ret_type)) = method_info {
                        if args.len() != param_types.len() {
                            return Err(format!(
                                "Método '{}' espera {} argumentos",
                                callee,
                                param_types.len()
                            ));
                        }
                        for (i, (arg, p_type)) in args.iter().zip(param_types.iter()).enumerate() {
                            let a_type = self.typecheck_expr(arg)?;
                            if !self.is_assignable(&a_type, p_type) {
                                return Err(format!(
                                    "Tipo incorrecto en arg {} de método '{}'",
                                    i + 1,
                                    callee
                                ));
                            }
                        }
                        return match ret_type {
                            Some(t) => Ok(t),
                            None => Err(format!("Método '{}' no retorna valor", callee)),
                        };
                    }
                }

                Err(format!("Función o método '{}' no declarado", callee))
            }
        }
    }

    fn typecheck_add_chain(&mut self, expr: &Expr) -> Result<Type, String> {
        let mut pending = vec![expr];
        let mut operands = Vec::new();
        while let Some(node) = pending.pop() {
            if let Expr::Binary {
                left,
                op: BinaryOp::Add,
                right,
            } = node
            {
                pending.push(right);
                pending.push(left);
            } else {
                operands.push(node);
            }
        }

        let mut common_type: Option<Type> = None;
        for operand in operands {
            let operand_type = self.typecheck_expr(operand)?;
            if !matches!(operand_type, Type::Int | Type::Float | Type::String) {
                return Err(format!(
                    "'+' no admite operandos de tipo {:?}",
                    operand_type
                ));
            }
            if let Some(expected) = &common_type {
                if *expected != operand_type {
                    return Err(format!(
                        "'+' no admite operandos {:?} y {:?}",
                        expected, operand_type
                    ));
                }
            } else {
                common_type = Some(operand_type);
            }
        }
        common_type.ok_or_else(|| "expresión '+' sin operandos".to_string())
    }

    fn validate_type(&self, t: &Type) -> Result<(), String> {
        match t {
            Type::Custom(name)
                if name != "ptr"
                    && !self.classes.contains_key(name)
                    && !self.records.contains_key(name) =>
            {
                Err(format!("Tipo '{}' no declarado", name))
            }
            Type::Array(inner) => self.validate_type(inner),
            Type::Map(key, value) => {
                self.validate_type(key)?;
                self.validate_type(value)
            }
            _ => Ok(()),
        }
    }
    fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }
    fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare_symbol(&mut self, name: &str, var_type: Type, is_const: bool) -> Result<(), String> {
        let current_scope = self
            .scopes
            .last_mut()
            .ok_or_else(|| "Error interno: no hay un ámbito activo".to_string())?;
        if current_scope.contains_key(name) {
            return Err(format!(
                "El nombre '{}' ya fue declarado en este ámbito",
                name
            ));
        }
        current_scope.insert(name.to_string(), Symbol { var_type, is_const });
        Ok(())
    }

    fn lookup_symbol(&self, name: &str) -> Option<&Symbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(symbol) = scope.get(name) {
                return Some(symbol);
            }
        }
        None
    }
}
