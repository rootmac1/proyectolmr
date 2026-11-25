use crate::ast::*;
use std::collections::HashMap;

#[derive(Clone)]
struct VarInfo {
    llvm_ptr: String,
    var_type: Type,
    is_local: bool,
}

pub struct CodeGenerator {
    output: String,
    string_constants: Vec<(String, String, usize)>,
    temp_counter: usize,
    label_counter: usize,
    scopes: Vec<HashMap<String, VarInfo>>,
    scope_roots: Vec<Vec<String>>,
    functions: HashMap<String, Option<Type>>,
    classes: HashMap<String, ClassDecl>,
    records: HashMap<String, RecordDecl>,
    class_vtables: HashMap<String, Vec<(String, String, Option<Type>, Vec<Type>)>>,
    current_class: Option<String>,
    enum_constants: HashMap<String, i64>,
    in_main: bool,
    codegen_error: Option<String>,
}

impl CodeGenerator {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            string_constants: Vec::new(),
            temp_counter: 0,
            label_counter: 0,
            scopes: Vec::new(),
            scope_roots: Vec::new(),
            functions: HashMap::new(),
            classes: HashMap::new(),
            records: HashMap::new(),
            class_vtables: HashMap::new(),
            current_class: None,
            enum_constants: HashMap::new(),
            in_main: false,
            codegen_error: None,
        }
    }

    pub fn generate(&mut self, program: &Program) -> Result<String, String> {
        for e in &program.enums {
            for (idx, variant) in e.variants.iter().enumerate() {
                self.enum_constants.insert(variant.clone(), idx as i64);
            }
        }

        for r in &program.records {
            self.records.insert(r.name.clone(), r.clone());
        }

        for class in &program.classes {
            self.classes.insert(class.name.clone(), class.clone());
        }

        let class_names: Vec<String> = self.classes.keys().cloned().collect();
        for c_name in &class_names {
            let mut current_parent = self.classes.get(c_name).and_then(|c| c.parent.clone());
            let mut inherited_fields = Vec::new();
            while let Some(ref p_name) = current_parent {
                if let Some(p_decl) = self.classes.get(p_name) {
                    for f in p_decl.fields.iter().rev() {
                        if !inherited_fields
                            .iter()
                            .any(|field: &FieldDecl| field.name == f.name)
                        {
                            inherited_fields.insert(0, f.clone());
                        }
                    }
                    current_parent = p_decl.parent.clone();
                } else {
                    break;
                }
            }
            if !inherited_fields.is_empty() {
                if let Some(c_decl) = self.classes.get_mut(c_name) {
                    let original = c_decl.fields.clone();
                    inherited_fields.extend(original);
                    c_decl.fields = inherited_fields;
                }
            }
        }

        for c_name in &class_names {
            let mut slots: Vec<(String, String, Option<Type>, Vec<Type>)> = Vec::new();
            let mut hierarchy = Vec::new();
            let mut curr = Some(c_name.clone());
            while let Some(c) = curr {
                hierarchy.insert(0, c.clone());
                curr = self.classes.get(&c).and_then(|cd| cd.parent.clone());
            }

            for cls in hierarchy {
                let Some(c_decl) = self.classes.get(&cls) else {
                    continue;
                };
                for m in &c_decl.methods {
                    if m.name == "constructor" || m.name == "destructor" {
                        continue;
                    }
                    let p_types = m.params.iter().map(|(t, _)| t.clone()).collect();
                    if let Some(pos) = slots.iter().position(|(m_name, _, _, _)| m_name == &m.name)
                    {
                        if let Some(slot) = slots.get_mut(pos) {
                            *slot = (m.name.clone(), cls.clone(), m.return_type.clone(), p_types);
                        }
                    } else {
                        slots.push((m.name.clone(), cls.clone(), m.return_type.clone(), p_types));
                    }
                }
            }
            self.class_vtables.insert(c_name.clone(), slots);
        }

        for func in &program.functions {
            self.functions
                .insert(func.name.clone(), func.return_type.clone());
        }
        for ext in &program.extern_funcs {
            self.functions
                .insert(ext.name.clone(), ext.return_type.clone());
        }

        let mut body_ir = String::new();

        for class in &program.classes {
            self.current_class = Some(class.name.clone());
            for method in &class.methods {
                body_ir.push_str(&self.gen_method(&class.name, method));
                body_ir.push('\n');
            }
            self.current_class = None;
        }

        for func in &program.functions {
            body_ir.push_str(&self.gen_function(func));
            body_ir.push('\n');
        }

        body_ir.push_str(&self.gen_main(&program.main_block));

        let mut header = String::new();
        header.push_str("; --- Código LLVM IR generado por el Compilador LMR ---\n");
        header.push_str("declare i32 @printf(ptr, ...)\n");
        header.push_str("declare i32 @scanf(ptr, ...)\n");
        header.push_str("declare i32 @getchar()\n");
        header.push_str("declare void @lmr_sleep(i32)\n");
        header.push_str("declare void @lmr_spawn_thread(ptr)\n");
        header.push_str("declare ptr @lmr_json_parse(ptr)\n");
        header.push_str("declare void @lmr_check_array_index(i64, i64)\n");
        header.push_str("declare void @lmr_check_array_size(i64)\n");
        header.push_str("declare i64 @lmr_checked_array_bytes(i64)\n");
        header.push_str("declare void @lmr_check_integer_division(i64, i64)\n");
        header.push_str("declare void @lmr_check_float_division(double)\n");
        header.push_str("declare void @lmr_check_map_capacity(i64)\n");
        header.push_str("declare void @lmr_missing_map_key()\n");
        header.push_str("declare ptr @lmr_malloc(i64)\n");
        header.push_str("declare void @lmr_free(ptr)\n");
        header.push_str("declare void @lmr_register_root(ptr)\n");
        header.push_str("declare void @lmr_unregister_root(ptr)\n");
        header.push_str("declare void @lmr_collect()\n");
        header.push_str("declare void @lmr_enter_function()\n");
        header.push_str("declare void @lmr_leave_function()\n");
        header.push_str("declare void @lmr_register_finalizer(ptr, ptr)\n");
        header.push_str("declare i64 @strlen(ptr)\n");
        header.push_str("declare i32 @strcmp(ptr, ptr)\n");
        header.push_str("declare ptr @strcpy(ptr, ptr)\n");
        header.push_str("declare ptr @strcat(ptr, ptr)\n");
        header.push_str("declare ptr @strstr(ptr, ptr)\n");
        header.push_str("declare ptr @memcpy(ptr, ptr, i64)\n");
        header.push_str("declare i32 @toupper(i32)\n");
        header.push_str("declare i32 @tolower(i32)\n");
        header.push_str("declare i32 @snprintf(ptr, i64, ptr, ...)\n");
        header.push_str("declare ptr @fopen(ptr, ptr)\n");
        header.push_str("declare i32 @fclose(ptr)\n");
        header.push_str("declare i64 @fread(ptr, i64, i64, ptr)\n");
        header.push_str("declare i64 @fwrite(ptr, i64, i64, ptr)\n");
        header.push_str("declare i32 @fseek(ptr, i64, i32)\n");
        header.push_str("declare i64 @ftell(ptr)\n");
        header.push_str("declare double @sqrt(double)\n");
        header.push_str("declare double @pow(double, double)\n");
        header.push_str("declare double @fabs(double)\n\n");
        header.push_str("declare double @sin(double)\n");
        header.push_str("declare double @cos(double)\n");
        header.push_str("declare double @tan(double)\n");
        header.push_str("declare double @asin(double)\n");
        header.push_str("declare double @acos(double)\n");
        header.push_str("declare double @atan(double)\n");
        header.push_str("declare double @atan2(double, double)\n");
        header.push_str("declare double @exp(double)\n");
        header.push_str("declare double @log(double)\n");
        header.push_str("declare double @log10(double)\n");
        header.push_str("declare double @floor(double)\n");
        header.push_str("declare double @ceil(double)\n");
        header.push_str("declare double @round(double)\n");
        header.push_str("declare double @fmod(double, double)\n\n");

        for ext in &program.extern_funcs {
            let ret_llvm = match &ext.return_type {
                Some(t) => self.llvm_type(t),
                None => "void",
            };
            let p_types: Vec<&str> = ext.params.iter().map(|(t, _)| self.llvm_type(t)).collect();
            header.push_str(&format!(
                "declare {} @{}({})\n",
                ret_llvm,
                ext.name,
                p_types.join(", ")
            ));
        }
        header.push('\n');

        header.push_str("@.fmt_int = private unnamed_addr constant [5 x i8] c\"%lld\\00\"\n");
        header.push_str("@.fmt_float = private unnamed_addr constant [7 x i8] c\"%-8.2f\\00\"\n");
        header.push_str("@.fmt_str = private unnamed_addr constant [3 x i8] c\"%s\\00\"\n");
        header.push_str("@.fmt_nl = private unnamed_addr constant [2 x i8] c\"\\0A\\00\"\n");
        header.push_str(
            "@.fmt_scan_str = private unnamed_addr constant [11 x i8] c\" %1023[^\\0A]\\00\"\n",
        );
        header.push_str("@.fmt_scan_int = private unnamed_addr constant [6 x i8] c\" %lld\\00\"\n");
        header
            .push_str("@.fmt_scan_float = private unnamed_addr constant [5 x i8] c\" %lf\\00\"\n");
        header.push_str("@.mode_rb = private unnamed_addr constant [3 x i8] c\"rb\\00\"\n");
        header.push_str("@.mode_wb = private unnamed_addr constant [3 x i8] c\"wb\\00\"\n");

        header.push_str("@__lmr_has_error = internal global i1 0\n");
        header.push_str("@__lmr_error_msg = internal global ptr null\n");

        header.push_str("%struct.Array = type { i64, ptr }\n");
        header.push_str("%struct.Map = type { i64, ptr, ptr }\n");

        for (c_name, slots) in &self.class_vtables {
            if slots.is_empty() {
                header.push_str(&format!(
                    "@vtable_{} = internal constant [1 x ptr] [ptr null]\n",
                    c_name
                ));
            } else {
                let fns: Vec<String> = slots
                    .iter()
                    .map(|(m_name, target_c, _, _)| format!("ptr @{}_{}", target_c, m_name))
                    .collect();
                header.push_str(&format!(
                    "@vtable_{} = internal constant [{} x ptr] [{}]\n",
                    c_name,
                    slots.len(),
                    fns.join(", ")
                ));
            }
        }

        for (c_name, c_decl) in &self.classes {
            let mut fields_llvm: Vec<&str> = vec!["ptr"];
            for field in &c_decl.fields {
                fields_llvm.push(self.llvm_type(&field.field_type));
            }
            header.push_str(&format!(
                "%struct.{} = type {{ {} }}\n",
                c_name,
                fields_llvm.join(", ")
            ));
        }

        for (r_name, r_decl) in &self.records {
            let fields_llvm: Vec<&str> = r_decl
                .fields
                .iter()
                .map(|field| self.llvm_type(&field.field_type))
                .collect();
            header.push_str(&format!(
                "%struct.{} = type {{ {} }}\n",
                r_name,
                fields_llvm.join(", ")
            ));
        }

        for (name, val, len) in &self.string_constants {
            let mut escaped_val = String::new();
            for b in val.bytes() {
                if b >= 32 && b <= 126 && b != b'"' && b != b'\\' {
                    escaped_val.push(b as char);
                } else {
                    escaped_val.push_str(&format!("\\{:02X}", b));
                }
            }
            header.push_str(&format!(
                "@{} = private unnamed_addr constant [{} x i8] c\"{}\\00\"\n",
                name, len, escaped_val
            ));
        }
        header.push('\n');

        header.push_str(&body_ir);
        match self.codegen_error.take() {
            Some(error) => Err(error),
            None => Ok(header),
        }
    }

    fn emit_cleanup_current_scope(&mut self) {
        let scope = match self.scopes.last() {
            Some(scope) => scope.clone(),
            None => {
                self.fail_codegen("intento de liberar variables sin un ámbito activo");
                return;
            }
        };
        for (name, info) in scope {
            if !info.is_local {
                continue;
            }
            if matches!(
                info.var_type,
                Type::Custom(_) | Type::Array(_) | Type::Map(_, _)
            ) {
                if let Type::Custom(ref type_name) = info.var_type {
                    if type_name == "ptr" || self.records.contains_key(type_name) {
                        continue;
                    }
                }

                let ptr_reg = self.next_temp();
                self.emit(&format!("  {} = load ptr, ptr {}", ptr_reg, info.llvm_ptr));
                let is_not_null = self.next_temp();
                self.emit(&format!(
                    "  {} = icmp ne ptr {}, null",
                    is_not_null, ptr_reg
                ));

                let id = self.next_label_id();
                let free_lbl = format!("raii_free_{}_{}", name, id);
                let cont_lbl = format!("raii_cont_{}_{}", name, id);

                self.emit(&format!(
                    "  br i1 {}, label %{}, label %{}",
                    is_not_null, free_lbl, cont_lbl
                ));
                self.emit(&format!("{}:", free_lbl));

                match &info.var_type {
                    Type::Custom(_) => {
                        self.emit(&format!("  call void @lmr_free(ptr {})", ptr_reg));
                    }
                    Type::Array(_) => {
                        let data_gep = self.next_temp();
                        self.emit(&format!(
                            "  {} = getelementptr inbounds %struct.Array, ptr {}, i32 0, i32 1",
                            data_gep, ptr_reg
                        ));
                        let data_ptr = self.next_temp();
                        self.emit(&format!("  {} = load ptr, ptr {}", data_ptr, data_gep));
                        self.emit(&format!("  call void @lmr_free(ptr {})", data_ptr));
                        self.emit(&format!("  call void @lmr_free(ptr {})", ptr_reg));
                    }
                    Type::Map(_, _) => {
                        let keys_gep = self.next_temp();
                        self.emit(&format!(
                            "  {} = getelementptr inbounds %struct.Map, ptr {}, i32 0, i32 1",
                            keys_gep, ptr_reg
                        ));
                        let keys_ptr = self.next_temp();
                        self.emit(&format!("  {} = load ptr, ptr {}", keys_ptr, keys_gep));
                        self.emit(&format!("  call void @lmr_free(ptr {})", keys_ptr));

                        let vals_gep = self.next_temp();
                        self.emit(&format!(
                            "  {} = getelementptr inbounds %struct.Map, ptr {}, i32 0, i32 2",
                            vals_gep, ptr_reg
                        ));
                        let vals_ptr = self.next_temp();
                        self.emit(&format!("  {} = load ptr, ptr {}", vals_ptr, vals_gep));
                        self.emit(&format!("  call void @lmr_free(ptr {})", vals_ptr));

                        self.emit(&format!("  call void @lmr_free(ptr {})", ptr_reg));
                    }
                    _ => {}
                }
                self.emit(&format!("  br label %{}", cont_lbl));
                self.emit(&format!("{}:", cont_lbl));
            }
        }
        if let Some(roots) = self.scope_roots.last().cloned() {
            for root in roots.iter().rev() {
                self.emit(&format!("  call void @lmr_unregister_root(ptr {})", root));
            }
        }
    }

    fn emit_unregister_all_roots(&mut self) {
        let roots = self
            .scope_roots
            .iter()
            .rev()
            .flat_map(|scope| scope.iter().rev())
            .cloned()
            .collect::<Vec<_>>();
        for root in roots {
            self.emit(&format!("  call void @lmr_unregister_root(ptr {})", root));
        }
    }

    fn gen_method(&mut self, class_name: &str, method: &FunctionDecl) -> String {
        self.output.clear();
        self.temp_counter = 0;
        self.enter_scope();

        let ret_llvm = match &method.return_type {
            Some(t) => self.llvm_type(t),
            None => "void",
        };
        let mut params_str = vec!["ptr %param_this".to_string()];
        for (p_type, p_name) in &method.params {
            params_str.push(format!("{} %param_{}", self.llvm_type(p_type), p_name));
        }

        self.emit(&format!(
            "define {} @{}_{}({}) {{",
            ret_llvm,
            class_name,
            method.name,
            params_str.join(", ")
        ));
        self.emit("entry:");
        self.emit("  call void @lmr_enter_function()");

        self.emit("  %ptr_this = alloca ptr");
        self.emit("  store ptr %param_this, ptr %ptr_this");
        self.insert_var(
            "this",
            "%ptr_this",
            Type::Custom(class_name.to_string()),
            false,
        );

        for (p_type, p_name) in &method.params {
            let ptr = format!("%ptr_{}", p_name);
            let llvm_t = self.llvm_type(p_type);
            self.emit(&format!("  {} = alloca {}", ptr, llvm_t));
            self.emit(&format!(
                "  store {} %param_{}, ptr {}",
                llvm_t, p_name, ptr
            ));
            self.insert_var(p_name, &ptr, p_type.clone(), false);
        }

        for stmt in &method.body {
            self.gen_stmt(stmt);
        }

        if !self.current_block_terminated() {
            self.emit_cleanup_current_scope();
            self.emit("  call void @lmr_leave_function()");
            if method.return_type.is_none() {
                self.emit("  ret void");
            } else {
                let default_ret = match method.return_type.as_ref() {
                    None => "ret void",
                    Some(Type::Int) => "ret i64 0",
                    Some(Type::Float) => "ret double 0.0",
                    Some(Type::Bool) => "ret i1 0",
                    Some(_) => "ret ptr null",
                };
                self.emit(&format!("  {}", default_ret));
            }
        }

        self.emit("}");
        self.exit_scope();
        self.output.clone()
    }

    fn gen_function(&mut self, func: &FunctionDecl) -> String {
        self.output.clear();
        self.temp_counter = 0;
        self.enter_scope();

        let ret_llvm = match &func.return_type {
            Some(t) => self.llvm_type(t),
            None => "void",
        };
        let mut params_str = Vec::new();
        for (p_type, p_name) in &func.params {
            params_str.push(format!("{} %param_{}", self.llvm_type(p_type), p_name));
        }

        self.emit(&format!(
            "define {} @{}({}) {{",
            ret_llvm,
            func.name,
            params_str.join(", ")
        ));
        self.emit("entry:");
        self.emit("  call void @lmr_enter_function()");

        for (p_type, p_name) in &func.params {
            let ptr = format!("%ptr_{}", p_name);
            let llvm_t = self.llvm_type(p_type);
            self.emit(&format!("  {} = alloca {}", ptr, llvm_t));
            self.emit(&format!(
                "  store {} %param_{}, ptr {}",
                llvm_t, p_name, ptr
            ));
            self.insert_var(p_name, &ptr, p_type.clone(), false);
        }

        for stmt in &func.body {
            self.gen_stmt(stmt);
        }

        if !self.current_block_terminated() {
            self.emit_cleanup_current_scope();
            self.emit("  call void @lmr_leave_function()");
            if func.return_type.is_none() {
                self.emit("  ret void");
            } else {
                let default_ret = match func.return_type.as_ref() {
                    None => "ret void",
                    Some(Type::Int) => "ret i64 0",
                    Some(Type::Float) => "ret double 0.0",
                    Some(Type::Bool) => "ret i1 0",
                    Some(_) => "ret ptr null",
                };
                self.emit(&format!("  {}", default_ret));
            }
        }

        self.emit("}");
        self.exit_scope();
        self.output.clone()
    }

    fn gen_main(&mut self, stmts: &[Stmt]) -> String {
        self.output.clear();
        self.temp_counter = 0;
        self.enter_scope();
        self.in_main = true;

        self.emit("define i32 @main() {");
        self.emit("entry:");
        self.emit("  call void @lmr_enter_function()");

        for stmt in stmts {
            self.gen_stmt(stmt);
        }

        if !self.current_block_terminated() {
            self.emit_cleanup_current_scope();
            self.emit("  call void @lmr_collect()");
            self.emit("  call void @lmr_leave_function()");
            self.emit("  ret i32 0");
        }
        self.emit("}");
        self.in_main = false;
        self.exit_scope();
        self.output.clone()
    }

    fn current_block_terminated(&self) -> bool {
        let last = self
            .output
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("")
            .trim();
        last.starts_with("ret ") || last.starts_with("br ") || last == "unreachable"
    }

    fn find_class_method(&self, class_name: &str, method: &str) -> Option<(String, FunctionDecl)> {
        let mut curr = Some(class_name.to_string());
        while let Some(c) = curr {
            if let Some(c_decl) = self.classes.get(&c) {
                if let Some(m) = c_decl.methods.iter().find(|m| &m.name == method) {
                    return Some((c.clone(), m.clone()));
                }
                curr = c_decl.parent.clone();
            } else {
                break;
            }
        }
        None
    }

    fn gen_stmt(&mut self, stmt: &Stmt) {
        let mut stmt = stmt;
        while let Stmt::Located { statement, .. } = stmt {
            stmt = statement;
        }
        match stmt {
            Stmt::Located { .. } => {}
            Stmt::VarDecl { var_type, names } => {
                let is_record = if let Type::Custom(name) = var_type {
                    self.records.contains_key(name)
                } else {
                    false
                };
                let is_raw_ptr = if let Type::Custom(name) = var_type {
                    name == "ptr"
                } else {
                    false
                };
                let llvm_t = if is_record {
                    if let Type::Custom(name) = var_type {
                        format!("%struct.{}", name)
                    } else {
                        self.llvm_type(var_type).to_string()
                    }
                } else {
                    self.llvm_type(var_type).to_string()
                };

                for name in names {
                    let ptr = format!("%{}_{}", name, self.next_temp_id());
                    self.emit(&format!("  {} = alloca {}", ptr, llvm_t));
                    if matches!(
                        var_type,
                        Type::String | Type::Custom(_) | Type::Array(_) | Type::Map(_, _)
                    ) && !is_record
                        && !is_raw_ptr
                    {
                        self.emit(&format!("  store ptr null, ptr {}", ptr));
                    }
                    self.insert_var(name, &ptr, var_type.clone(), !is_record && !is_raw_ptr);
                }
            }

            Stmt::ConstDecl { name, value, .. } => {
                let (val_reg, val_type) = self.gen_expr(value);
                let llvm_t = self.llvm_type(&val_type);
                let ptr = format!("%const_{}_{}", name, self.next_temp_id());
                self.emit(&format!("  {} = alloca {}", ptr, llvm_t));
                self.emit(&format!("  store {} {}, ptr {}", llvm_t, val_reg, ptr));
                self.insert_var(name, &ptr, val_type, true);
            }

            Stmt::Assign { name, value } => {
                let (val_reg, val_type) = self.gen_expr(value);
                let llvm_t = self.llvm_type(&val_type);

                if let Some(info) = self.lookup_var(name) {
                    self.emit(&format!(
                        "  store {} {}, ptr {}",
                        llvm_t, val_reg, info.llvm_ptr
                    ));
                    return;
                }

                if let Some(c_name) = self.current_class.clone() {
                    let c_decl = self.required_class(&c_name);
                    if let Some(idx) = c_decl.fields.iter().position(|field| field.name == *name) {
                        let this_info = self.required_var("this");
                        let this_reg = self.next_temp();
                        self.emit(&format!(
                            "  {} = load ptr, ptr {}",
                            this_reg, this_info.llvm_ptr
                        ));
                        let gep = self.next_temp();
                        self.emit(&format!(
                            "  {} = getelementptr inbounds %struct.{}, ptr {}, i32 0, i32 {}",
                            gep,
                            c_name,
                            this_reg,
                            idx + 1
                        ));
                        self.emit(&format!("  store {} {}, ptr {}", llvm_t, val_reg, gep));
                    }
                }
            }

            Stmt::MemberAssign {
                object,
                member,
                value,
            } => {
                let (val_reg, val_type) = self.gen_expr(value);
                let llvm_t = self.llvm_type(&val_type);

                if let Expr::Variable(v_name) = object.as_ref() {
                    let info = self.required_var(v_name);
                    if let Type::Custom(name) = &info.var_type {
                        if let Some(r_decl) = self.records.get(name).cloned() {
                            let idx = self.required_field_index(name, &r_decl.fields, member);
                            let gep = self.next_temp();
                            self.emit(&format!(
                                "  {} = getelementptr inbounds %struct.{}, ptr {}, i32 0, i32 {}",
                                gep, name, info.llvm_ptr, idx
                            ));
                            self.emit(&format!("  store {} {}, ptr {}", llvm_t, val_reg, gep));
                            return;
                        } else if let Some(c_decl) = self.classes.get(name).cloned() {
                            let idx = self.required_field_index(name, &c_decl.fields, member);
                            let obj_ptr = self.next_temp();
                            self.emit(&format!("  {} = load ptr, ptr {}", obj_ptr, info.llvm_ptr));
                            let gep = self.next_temp();
                            self.emit(&format!(
                                "  {} = getelementptr inbounds %struct.{}, ptr {}, i32 0, i32 {}",
                                gep,
                                name,
                                obj_ptr,
                                idx + 1
                            ));
                            self.emit(&format!("  store {} {}, ptr {}", llvm_t, val_reg, gep));
                            return;
                        }
                    }
                }

                let (obj_reg, obj_type) = self.gen_expr(object);
                if let Type::Custom(ref c_name) = obj_type {
                    let c_decl = self.required_class(c_name);
                    let idx = self.required_field_index(c_name, &c_decl.fields, member);
                    let gep = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds %struct.{}, ptr {}, i32 0, i32 {}",
                        gep,
                        c_name,
                        obj_reg,
                        idx + 1
                    ));
                    self.emit(&format!("  store {} {}, ptr {}", llvm_t, val_reg, gep));
                }
            }

            Stmt::IndexAssign {
                array,
                index,
                value,
            } => {
                let (arr_reg, arr_type) = self.gen_expr(array);
                let (idx_reg, _) = self.gen_expr(index);
                let (val_reg, _) = self.gen_expr(value);

                if let Type::Array(inner) = arr_type {
                    let inner_llvm = self.llvm_type(&inner);
                    let gep_len = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds %struct.Array, ptr {}, i32 0, i32 0",
                        gep_len, arr_reg
                    ));
                    let len_reg = self.next_temp();
                    self.emit(&format!("  {} = load i64, ptr {}", len_reg, gep_len));
                    self.emit(&format!(
                        "  call void @lmr_check_array_index(i64 {}, i64 {})",
                        idx_reg, len_reg
                    ));
                    let gep_data = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds %struct.Array, ptr {}, i32 0, i32 1",
                        gep_data, arr_reg
                    ));
                    let data_ptr = self.next_temp();
                    self.emit(&format!("  {} = load ptr, ptr {}", data_ptr, gep_data));
                    let elem_gep = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds {}, ptr {}, i64 {}",
                        elem_gep, inner_llvm, data_ptr, idx_reg
                    ));
                    self.emit(&format!(
                        "  store {} {}, ptr {}",
                        inner_llvm, val_reg, elem_gep
                    ));
                } else if let Type::Map(k_type, v_type) = arr_type {
                    let v_llvm = self.llvm_type(&v_type);
                    let id = self.next_label_id();

                    let gep_sz = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds %struct.Map, ptr {}, i32 0, i32 0",
                        gep_sz, arr_reg
                    ));
                    let sz = self.next_temp();
                    self.emit(&format!("  {} = load i64, ptr {}", sz, gep_sz));

                    let gep_k = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds %struct.Map, ptr {}, i32 0, i32 1",
                        gep_k, arr_reg
                    ));
                    let k_arr = self.next_temp();
                    self.emit(&format!("  {} = load ptr, ptr {}", k_arr, gep_k));

                    let gep_v = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds %struct.Map, ptr {}, i32 0, i32 2",
                        gep_v, arr_reg
                    ));
                    let v_arr = self.next_temp();
                    self.emit(&format!("  {} = load ptr, ptr {}", v_arr, gep_v));

                    let idx_ptr = format!("%map_i_{}", id);
                    self.emit(&format!("  {} = alloca i64", idx_ptr));
                    self.emit(&format!("  store i64 0, ptr {}", idx_ptr));

                    let cond_lbl = format!("map_cond_{}", id);
                    let body_lbl = format!("map_body_{}", id);
                    let match_lbl = format!("map_match_{}", id);
                    let inc_lbl = format!("map_inc_{}", id);
                    let not_found_lbl = format!("map_nf_{}", id);
                    let end_lbl = format!("map_end_{}", id);

                    self.emit(&format!("  br label %{}", cond_lbl));
                    self.emit(&format!("{}:", cond_lbl));
                    let curr_i = self.next_temp();
                    self.emit(&format!("  {} = load i64, ptr {}", curr_i, idx_ptr));
                    let cmp_sz = self.next_temp();
                    self.emit(&format!("  {} = icmp slt i64 {}, {}", cmp_sz, curr_i, sz));
                    self.emit(&format!(
                        "  br i1 {}, label %{}, label %{}",
                        cmp_sz, body_lbl, not_found_lbl
                    ));

                    self.emit(&format!("{}:", body_lbl));
                    let curr_k_gep = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds {}, ptr {}, i64 {}",
                        curr_k_gep,
                        self.llvm_type(&k_type),
                        k_arr,
                        curr_i
                    ));
                    let curr_k = self.next_temp();
                    self.emit(&format!(
                        "  {} = load {}, ptr {}",
                        curr_k,
                        self.llvm_type(&k_type),
                        curr_k_gep
                    ));

                    let cmp_key = self.next_temp();
                    if *k_type == Type::String {
                        let strcmp_res = self.next_temp();
                        self.emit(&format!(
                            "  {} = call i32 @strcmp(ptr {}, ptr {})",
                            strcmp_res, curr_k, idx_reg
                        ));
                        self.emit(&format!("  {} = icmp eq i32 {}, 0", cmp_key, strcmp_res));
                    } else {
                        self.emit(&format!(
                            "  {} = icmp eq {} {}, {}",
                            cmp_key,
                            self.llvm_type(&k_type),
                            curr_k,
                            idx_reg
                        ));
                    }

                    self.emit(&format!(
                        "  br i1 {}, label %{}, label %{}",
                        cmp_key, match_lbl, inc_lbl
                    ));

                    self.emit(&format!("{}:", match_lbl));
                    let curr_v_gep = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds {}, ptr {}, i64 {}",
                        curr_v_gep, v_llvm, v_arr, curr_i
                    ));
                    self.emit(&format!(
                        "  store {} {}, ptr {}",
                        v_llvm, val_reg, curr_v_gep
                    ));
                    self.emit(&format!("  br label %{}", end_lbl));

                    self.emit(&format!("{}:", inc_lbl));
                    let next_i = self.next_temp();
                    self.emit(&format!("  {} = add i64 {}, 1", next_i, curr_i));
                    self.emit(&format!("  store i64 {}, ptr {}", next_i, idx_ptr));
                    self.emit(&format!("  br label %{}", cond_lbl));

                    self.emit(&format!("{}:", not_found_lbl));
                    self.emit(&format!("  call void @lmr_check_map_capacity(i64 {})", sz));
                    let new_k_gep = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds {}, ptr {}, i64 {}",
                        new_k_gep,
                        self.llvm_type(&k_type),
                        k_arr,
                        sz
                    ));
                    self.emit(&format!(
                        "  store {} {}, ptr {}",
                        self.llvm_type(&k_type),
                        idx_reg,
                        new_k_gep
                    ));
                    let new_v_gep = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds {}, ptr {}, i64 {}",
                        new_v_gep, v_llvm, v_arr, sz
                    ));
                    self.emit(&format!(
                        "  store {} {}, ptr {}",
                        v_llvm, val_reg, new_v_gep
                    ));
                    let new_sz = self.next_temp();
                    self.emit(&format!("  {} = add i64 {}, 1", new_sz, sz));
                    self.emit(&format!("  store i64 {}, ptr {}", new_sz, gep_sz));
                    self.emit(&format!("  br label %{}", end_lbl));

                    self.emit(&format!("{}:", end_lbl));
                }
            }

            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let (cond_reg, _) = self.gen_expr(condition);
                let id = self.next_label_id();
                let then_lbl = format!("then_{}", id);
                let else_lbl = format!("else_{}", id);
                let end_lbl = format!("endif_{}", id);

                if else_branch.is_some() {
                    self.emit(&format!(
                        "  br i1 {}, label %{}, label %{}",
                        cond_reg, then_lbl, else_lbl
                    ));
                } else {
                    self.emit(&format!(
                        "  br i1 {}, label %{}, label %{}",
                        cond_reg, then_lbl, end_lbl
                    ));
                }

                self.emit(&format!("{}:", then_lbl));
                self.enter_scope();
                for s in then_branch {
                    self.gen_stmt(s);
                }
                self.emit_cleanup_current_scope();
                self.exit_scope();
                let then_terminated = self.current_block_terminated();
                if !then_terminated {
                    self.emit("  call void @lmr_collect()");
                    self.emit(&format!("  br label %{}", end_lbl));
                }

                let mut else_terminated = false;
                if let Some(else_stmts) = else_branch {
                    self.emit(&format!("{}:", else_lbl));
                    self.enter_scope();
                    for s in else_stmts {
                        self.gen_stmt(s);
                    }
                    self.emit_cleanup_current_scope();
                    self.exit_scope();
                    else_terminated = self.current_block_terminated();
                    if !else_terminated {
                        self.emit("  call void @lmr_collect()");
                        self.emit(&format!("  br label %{}", end_lbl));
                    }
                }

                self.emit(&format!("{}:", end_lbl));
                if then_terminated && else_terminated {
                    self.emit("  unreachable");
                    self.emit(&format!("dead_after_if_{}:", id));
                }
            }

            Stmt::While { condition, body } => {
                let id = self.next_label_id();
                let cond_lbl = format!("while_cond_{}", id);
                let body_lbl = format!("while_body_{}", id);
                let end_lbl = format!("while_end_{}", id);

                self.emit(&format!("  br label %{}", cond_lbl));
                self.emit(&format!("{}:", cond_lbl));
                let (cond_reg, _) = self.gen_expr(condition);
                self.emit(&format!(
                    "  br i1 {}, label %{}, label %{}",
                    cond_reg, body_lbl, end_lbl
                ));

                self.emit(&format!("{}:", body_lbl));
                self.enter_scope();
                for s in body {
                    self.gen_stmt(s);
                }
                self.emit_cleanup_current_scope();
                self.exit_scope();
                if !self.current_block_terminated() {
                    self.emit("  call void @lmr_collect()");
                    self.emit(&format!("  br label %{}", cond_lbl));
                }

                self.emit(&format!("{}:", end_lbl));
            }

            Stmt::ForRange {
                var_name,
                start,
                end,
                body,
            } => {
                let (start_reg, _) = self.gen_expr(start);
                let (end_reg, _) = self.gen_expr(end);
                let id = self.next_label_id();
                let var_ptr = format!("%for_var_{}_{}", var_name, id);
                self.emit(&format!("  {} = alloca i64", var_ptr));
                self.emit(&format!("  store i64 {}, ptr {}", start_reg, var_ptr));

                let cond_lbl = format!("for_cond_{}", id);
                let body_lbl = format!("for_body_{}", id);
                let inc_lbl = format!("for_inc_{}", id);
                let end_lbl = format!("for_end_{}", id);

                self.emit(&format!("  br label %{}", cond_lbl));
                self.emit(&format!("{}:", cond_lbl));
                let curr_val = self.next_temp();
                self.emit(&format!("  {} = load i64, ptr {}", curr_val, var_ptr));
                let cmp_reg = self.next_temp();
                self.emit(&format!(
                    "  {} = icmp sle i64 {}, {}",
                    cmp_reg, curr_val, end_reg
                ));
                self.emit(&format!(
                    "  br i1 {}, label %{}, label %{}",
                    cmp_reg, body_lbl, end_lbl
                ));

                self.emit(&format!("{}:", body_lbl));
                self.enter_scope();
                self.insert_var(var_name, &var_ptr, Type::Int, false);
                for s in body {
                    self.gen_stmt(s);
                }
                self.emit_cleanup_current_scope();
                self.exit_scope();
                if !self.current_block_terminated() {
                    self.emit("  call void @lmr_collect()");
                    self.emit(&format!("  br label %{}", inc_lbl));
                }

                self.emit(&format!("{}:", inc_lbl));
                let prev_v = self.next_temp();
                self.emit(&format!("  {} = load i64, ptr {}", prev_v, var_ptr));
                let next_v = self.next_temp();
                self.emit(&format!("  {} = add i64 {}, 1", next_v, prev_v));
                self.emit(&format!("  store i64 {}, ptr {}", next_v, var_ptr));
                self.emit(&format!("  br label %{}", cond_lbl));

                self.emit(&format!("{}:", end_lbl));
            }

            Stmt::ForIn {
                var_name,
                iterable,
                body,
            } => {
                let (arr_reg, arr_type) = self.gen_expr(iterable);
                let inner_type = match arr_type {
                    Type::Array(inner) => *inner,
                    _ => {
                        self.fail_codegen("for-in recibió un valor que no es array");
                        Type::Int
                    }
                };
                let inner_llvm = self.llvm_type(&inner_type);
                let id = self.next_label_id();

                let gep_len = self.next_temp();
                self.emit(&format!(
                    "  {} = getelementptr inbounds %struct.Array, ptr {}, i32 0, i32 0",
                    gep_len, arr_reg
                ));
                let len_reg = self.next_temp();
                self.emit(&format!("  {} = load i64, ptr {}", len_reg, gep_len));

                let gep_data = self.next_temp();
                self.emit(&format!(
                    "  {} = getelementptr inbounds %struct.Array, ptr {}, i32 0, i32 1",
                    gep_data, arr_reg
                ));
                let data_ptr = self.next_temp();
                self.emit(&format!("  {} = load ptr, ptr {}", data_ptr, gep_data));
                let iterable_root = format!("%forin_root_{}", id);
                self.emit(&format!("  {} = alloca ptr", iterable_root));
                self.emit(&format!("  store ptr {}, ptr {}", arr_reg, iterable_root));
                self.emit(&format!(
                    "  call void @lmr_register_root(ptr {})",
                    iterable_root
                ));
                if let Some(roots) = self.scope_roots.last_mut() {
                    roots.push(iterable_root.clone());
                }

                let idx_ptr = format!("%forin_idx_{}", id);
                self.emit(&format!("  {} = alloca i64", idx_ptr));
                self.emit(&format!("  store i64 0, ptr {}", idx_ptr));

                let elem_ptr = format!("%forin_elem_{}_{}", var_name, id);
                self.emit(&format!("  {} = alloca {}", elem_ptr, inner_llvm));

                let cond_lbl = format!("forin_cond_{}", id);
                let body_lbl = format!("forin_body_{}", id);
                let inc_lbl = format!("forin_inc_{}", id);
                let end_lbl = format!("forin_end_{}", id);

                self.emit(&format!("  br label %{}", cond_lbl));
                self.emit(&format!("{}:", cond_lbl));
                let curr_idx = self.next_temp();
                self.emit(&format!("  {} = load i64, ptr {}", curr_idx, idx_ptr));
                let cmp_reg = self.next_temp();
                self.emit(&format!(
                    "  {} = icmp slt i64 {}, {}",
                    cmp_reg, curr_idx, len_reg
                ));
                self.emit(&format!(
                    "  br i1 {}, label %{}, label %{}",
                    cmp_reg, body_lbl, end_lbl
                ));

                self.emit(&format!("{}:", body_lbl));
                let elem_gep = self.next_temp();
                self.emit(&format!(
                    "  {} = getelementptr inbounds {}, ptr {}, i64 {}",
                    elem_gep, inner_llvm, data_ptr, curr_idx
                ));
                let curr_val = self.next_temp();
                self.emit(&format!(
                    "  {} = load {}, ptr {}",
                    curr_val, inner_llvm, elem_gep
                ));
                self.emit(&format!(
                    "  store {} {}, ptr {}",
                    inner_llvm, curr_val, elem_ptr
                ));

                self.enter_scope();
                self.insert_var(var_name, &elem_ptr, inner_type, false);
                for s in body {
                    self.gen_stmt(s);
                }
                self.emit_cleanup_current_scope();
                self.exit_scope();
                if !self.current_block_terminated() {
                    self.emit("  call void @lmr_collect()");
                    self.emit(&format!("  br label %{}", inc_lbl));
                }

                self.emit(&format!("{}:", inc_lbl));
                let prev_i = self.next_temp();
                self.emit(&format!("  {} = load i64, ptr {}", prev_i, idx_ptr));
                let next_i = self.next_temp();
                self.emit(&format!("  {} = add i64 {}, 1", next_i, prev_i));
                self.emit(&format!("  store i64 {}, ptr {}", next_i, idx_ptr));
                self.emit(&format!("  br label %{}", cond_lbl));

                self.emit(&format!("{}:", end_lbl));
                self.emit(&format!(
                    "  call void @lmr_unregister_root(ptr {})",
                    iterable_root
                ));
            }

            Stmt::Spawn { func_name } => {
                self.emit(&format!(
                    "  call void @lmr_spawn_thread(ptr @{})",
                    func_name
                ));
            }

            Stmt::TryCatch {
                try_block,
                catch_var,
                catch_block,
            } => {
                let id = self.next_label_id();
                let catch_lbl = format!("catch_{}", id);
                let end_lbl = format!("try_end_{}", id);

                self.enter_scope();
                for s in try_block {
                    self.gen_stmt(s);
                    let has_err = self.next_temp();
                    self.emit(&format!("  {} = load i1, ptr @__lmr_has_error", has_err));
                    let next_stmt_lbl = self.next_label_id();
                    let cont_lbl = format!("try_cont_{}", next_stmt_lbl);
                    self.emit(&format!(
                        "  br i1 {}, label %{}, label %{}",
                        has_err, catch_lbl, cont_lbl
                    ));
                    self.emit(&format!("{}:", cont_lbl));
                }
                self.emit_cleanup_current_scope();
                self.exit_scope();
                self.emit(&format!("  br label %{}", end_lbl));

                self.emit(&format!("{}:", catch_lbl));
                self.emit("  store i1 0, ptr @__lmr_has_error");
                let err_msg_ptr = self.next_temp();
                self.emit(&format!(
                    "  {} = load ptr, ptr @__lmr_error_msg",
                    err_msg_ptr
                ));

                let catch_var_ptr = format!("%catch_var_{}_{}", catch_var, id);
                self.emit(&format!("  {} = alloca ptr", catch_var_ptr));
                self.emit(&format!(
                    "  store ptr {}, ptr {}",
                    err_msg_ptr, catch_var_ptr
                ));

                self.enter_scope();
                self.insert_var(catch_var, &catch_var_ptr, Type::String, false);
                for s in catch_block {
                    self.gen_stmt(s);
                }
                self.emit_cleanup_current_scope();
                self.exit_scope();
                self.emit(&format!("  br label %{}", end_lbl));

                self.emit(&format!("{}:", end_lbl));
            }

            Stmt::Throw(err_expr) => {
                let (msg_reg, _) = self.gen_expr(err_expr);
                self.emit("  store i1 1, ptr @__lmr_has_error");
                self.emit(&format!("  store ptr {}, ptr @__lmr_error_msg", msg_reg));
            }

            Stmt::Return(maybe_expr) => {
                if let Some(expr) = maybe_expr {
                    let (val_reg, val_type) = self.gen_expr(expr);
                    let llvm_t = self.llvm_type(&val_type);
                    self.emit_cleanup_current_scope();
                    self.emit_unregister_all_roots();
                    self.emit("  call void @lmr_leave_function()");
                    self.emit(&format!("  ret {} {}", llvm_t, val_reg));
                } else {
                    self.emit_cleanup_current_scope();
                    self.emit_unregister_all_roots();
                    self.emit("  call void @lmr_leave_function()");
                    if self.in_main {
                        self.emit("  ret i32 0");
                    } else {
                        self.emit("  ret void");
                    }
                }
            }

            Stmt::Expression(expr) => {
                if let Expr::Call { callee, args } = expr {
                    if callee == "write" || callee == "writeln" {
                        for arg in args {
                            let (val_reg, val_type) = self.gen_expr(arg);
                            match val_type {
                                Type::Int | Type::Bool => {
                                    let tmp = self.next_temp();
                                    if val_type == Type::Bool {
                                        let ext = self.next_temp();
                                        self.emit(&format!(
                                            "  {} = zext i1 {} to i64",
                                            ext, val_reg
                                        ));
                                        self.emit(&format!("  {} = call i32 (ptr, ...) @printf(ptr @.fmt_int, i64 {})", tmp, ext));
                                    } else {
                                        self.emit(&format!("  {} = call i32 (ptr, ...) @printf(ptr @.fmt_int, i64 {})", tmp, val_reg));
                                    }
                                }
                                Type::Float => {
                                    let tmp = self.next_temp();
                                    self.emit(&format!("  {} = call i32 (ptr, ...) @printf(ptr @.fmt_float, double {})", tmp, val_reg));
                                }
                                Type::String => {
                                    let tmp = self.next_temp();
                                    self.emit(&format!(
                                        "  {} = call i32 (ptr, ...) @printf(ptr @.fmt_str, ptr {})",
                                        tmp, val_reg
                                    ));
                                }
                                _ => {}
                            }
                        }
                        if callee == "writeln" {
                            let tmp = self.next_temp();
                            self.emit(&format!(
                                "  {} = call i32 (ptr, ...) @printf(ptr @.fmt_nl)",
                                tmp
                            ));
                        }
                        return;
                    }

                    if callee == "readln" && args.len() == 1 {
                        if let Some(Expr::Variable(v_name)) = args.first() {
                            let info = self.required_var(v_name);
                            let d = self.next_temp();
                            match info.var_type {
                                Type::Int => {
                                    self.emit(&format!("  {} = call i32 (ptr, ...) @scanf(ptr @.fmt_scan_int, ptr {})", d, info.llvm_ptr));
                                }
                                Type::Float => {
                                    self.emit(&format!("  {} = call i32 (ptr, ...) @scanf(ptr @.fmt_scan_float, ptr {})", d, info.llvm_ptr));
                                }
                                Type::String => {
                                    let buf = self.next_temp();
                                    self.emit(&format!(
                                        "  {} = call ptr @lmr_malloc(i64 1024)",
                                        buf
                                    ));
                                    self.emit(&format!("  {} = call i32 (ptr, ...) @scanf(ptr @.fmt_scan_str, ptr {})", d, buf));
                                    self.emit(&format!(
                                        "  store ptr {}, ptr {}",
                                        buf, info.llvm_ptr
                                    ));
                                }
                                _ => {}
                            }
                            return;
                        }
                    }

                    if callee == "dispose" && args.len() == 1 {
                        if let Some(Expr::Variable(v_name)) = args.first() {
                            if let Some(info) = self.lookup_var(v_name) {
                                let target_ptr = self.next_temp();
                                self.emit(&format!(
                                    "  {} = load ptr, ptr {}",
                                    target_ptr, info.llvm_ptr
                                ));

                                if let Type::Array(_) = info.var_type {
                                    let gep_data = self.next_temp();
                                    self.emit(&format!("  {} = getelementptr inbounds %struct.Array, ptr {}, i32 0, i32 1", gep_data, target_ptr));
                                    let data_ptr = self.next_temp();
                                    self.emit(&format!(
                                        "  {} = load ptr, ptr {}",
                                        data_ptr, gep_data
                                    ));
                                    self.emit(&format!("  call void @lmr_free(ptr {})", data_ptr));
                                    self.emit(&format!(
                                        "  call void @lmr_free(ptr {})",
                                        target_ptr
                                    ));
                                } else if let Type::Map(_, _) = info.var_type {
                                    let keys_gep = self.next_temp();
                                    self.emit(&format!("  {} = getelementptr inbounds %struct.Map, ptr {}, i32 0, i32 1", keys_gep, target_ptr));
                                    let keys_ptr = self.next_temp();
                                    self.emit(&format!(
                                        "  {} = load ptr, ptr {}",
                                        keys_ptr, keys_gep
                                    ));
                                    self.emit(&format!("  call void @lmr_free(ptr {})", keys_ptr));

                                    let vals_gep = self.next_temp();
                                    self.emit(&format!("  {} = getelementptr inbounds %struct.Map, ptr {}, i32 0, i32 2", vals_gep, target_ptr));
                                    let vals_ptr = self.next_temp();
                                    self.emit(&format!(
                                        "  {} = load ptr, ptr {}",
                                        vals_ptr, vals_gep
                                    ));
                                    self.emit(&format!("  call void @lmr_free(ptr {})", vals_ptr));

                                    self.emit(&format!(
                                        "  call void @lmr_free(ptr {})",
                                        target_ptr
                                    ));
                                } else {
                                    self.emit(&format!(
                                        "  call void @lmr_free(ptr {})",
                                        target_ptr
                                    ));
                                }
                                self.emit(&format!("  store ptr null, ptr {}", info.llvm_ptr));
                                return;
                            }
                        }
                    }
                }
                self.gen_expr(expr);
            }
        }
    }

    fn gen_add_chain(&mut self, expr: &Expr) -> (String, Type) {
        // String/number concatenations are parsed as left-deep binary trees.
        // Walk those trees iteratively so long SQL/UI strings cannot exhaust
        // the native thread stack during code generation.
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

        let mut values = operands.into_iter();
        let Some(first) = values.next() else {
            return ("0".to_string(), Type::Int);
        };
        let mut result = self.gen_expr(first);
        for operand in values {
            let right = self.gen_expr(operand);
            let dest = self.next_temp();
            match result.1 {
                Type::Int => self.emit(&format!("  {} = add i64 {}, {}", dest, result.0, right.0)),
                Type::Float => self.emit(&format!(
                    "  {} = fadd double {}, {}",
                    dest, result.0, right.0
                )),
                Type::String => {
                    let left_len = self.next_temp();
                    self.emit(&format!(
                        "  {} = call i64 @strlen(ptr {})",
                        left_len, result.0
                    ));
                    let right_len = self.next_temp();
                    self.emit(&format!(
                        "  {} = call i64 @strlen(ptr {})",
                        right_len, right.0
                    ));
                    let total_len = self.next_temp();
                    self.emit(&format!(
                        "  {} = add i64 {}, {}",
                        total_len, left_len, right_len
                    ));
                    let alloc_size = self.next_temp();
                    self.emit(&format!("  {} = add i64 {}, 1", alloc_size, total_len));
                    let buffer = self.next_temp();
                    self.emit(&format!(
                        "  {} = call ptr @lmr_malloc(i64 {})",
                        buffer, alloc_size
                    ));
                    let copy = self.next_temp();
                    self.emit(&format!(
                        "  {} = call ptr @strcpy(ptr {}, ptr {})",
                        copy, buffer, result.0
                    ));
                    let append = self.next_temp();
                    self.emit(&format!(
                        "  {} = call ptr @strcat(ptr {}, ptr {})",
                        append, buffer, right.0
                    ));
                    result = (buffer, Type::String);
                    continue;
                }
                other => {
                    self.fail_codegen(format!("operador + no soportado para tipo {:?}", other));
                    return ("undef".to_string(), Type::Int);
                }
            }
            result = (dest, result.1);
        }
        result
    }

    fn gen_expr(&mut self, expr: &Expr) -> (String, Type) {
        match expr {
            Expr::Integer(val) => (val.to_string(), Type::Int),
            Expr::Float(val) => {
                let s = if val.fract() == 0.0 {
                    format!("{:.1}", val)
                } else {
                    val.to_string()
                };
                (s, Type::Float)
            }
            Expr::Boolean(val) => (
                if *val {
                    "1".to_string()
                } else {
                    "0".to_string()
                },
                Type::Bool,
            ),

            Expr::StringLit(s) => {
                let id = self.string_constants.len();
                let name = format!(".str_{}", id);
                let len = s.len() + 1;
                self.string_constants.push((name.clone(), s.clone(), len));
                (format!("@{}", name), Type::String)
            }

            Expr::Variable(name) => {
                if let Some(info) = self.lookup_var(name) {
                    let is_record = if let Type::Custom(c_name) = &info.var_type {
                        self.records.contains_key(c_name)
                    } else {
                        false
                    };
                    if is_record {
                        return (info.llvm_ptr.clone(), info.var_type.clone());
                    }
                    let reg = self.next_temp();
                    let llvm_t = self.llvm_type(&info.var_type);
                    self.emit(&format!(
                        "  {} = load {}, ptr {}",
                        reg, llvm_t, info.llvm_ptr
                    ));
                    return (reg, info.var_type);
                }

                if let Some(&val) = self.enum_constants.get(name) {
                    return (val.to_string(), Type::Int);
                }

                if let Some(c_name) = self.current_class.clone() {
                    let c_decl = self.required_class(&c_name);
                    if let Some(idx) = c_decl.fields.iter().position(|field| field.name == *name) {
                        let f_type = self.required_field_type(&c_decl.fields, idx, &c_name);
                        let this_info = self.required_var("this");
                        let this_reg = self.next_temp();
                        self.emit(&format!(
                            "  {} = load ptr, ptr {}",
                            this_reg, this_info.llvm_ptr
                        ));
                        let gep = self.next_temp();
                        self.emit(&format!(
                            "  {} = getelementptr inbounds %struct.{}, ptr {}, i32 0, i32 {}",
                            gep,
                            c_name,
                            this_reg,
                            idx + 1
                        ));
                        let reg = self.next_temp();
                        let llvm_t = self.llvm_type(&f_type);
                        self.emit(&format!("  {} = load {}, ptr {}", reg, llvm_t, gep));
                        return (reg, f_type);
                    }
                }

                self.fail_codegen(format!(
                    "variable '{}' no disponible durante la generación",
                    name
                ));
                ("0".to_string(), Type::Int)
            }

            Expr::ArrayLiteral(elements) => {
                let len = elements.len();
                let mut comp_elements = Vec::new();
                for el in elements {
                    comp_elements.push(self.gen_expr(el));
                }
                let elem_type = match comp_elements.first() {
                    Some((_, t)) => t.clone(),
                    None => {
                        self.fail_codegen("literal de array vacío llegó al generador");
                        Type::Int
                    }
                };
                let elem_llvm = self.llvm_type(&elem_type);

                let desc_ptr = self.next_temp();
                self.emit(&format!("  {} = call ptr @lmr_malloc(i64 16)", desc_ptr));

                let data_ptr = self.next_temp();
                self.emit(&format!(
                    "  {} = call ptr @lmr_malloc(i64 {})",
                    data_ptr,
                    (len * 8).max(8)
                ));

                for (i, (val_reg, _)) in comp_elements.iter().enumerate() {
                    let elem_gep = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds {}, ptr {}, i64 {}",
                        elem_gep, elem_llvm, data_ptr, i
                    ));
                    self.emit(&format!(
                        "  store {} {}, ptr {}",
                        elem_llvm, val_reg, elem_gep
                    ));
                }

                let len_gep = self.next_temp();
                self.emit(&format!(
                    "  {} = getelementptr inbounds %struct.Array, ptr {}, i32 0, i32 0",
                    len_gep, desc_ptr
                ));
                self.emit(&format!("  store i64 {}, ptr {}", len, len_gep));

                let data_gep = self.next_temp();
                self.emit(&format!(
                    "  {} = getelementptr inbounds %struct.Array, ptr {}, i32 0, i32 1",
                    data_gep, desc_ptr
                ));
                self.emit(&format!("  store ptr {}, ptr {}", data_ptr, data_gep));

                (desc_ptr, Type::Array(Box::new(elem_type)))
            }

            Expr::NewArray { element_type, size } => {
                let (sz_reg, _) = self.gen_expr(size);
                let desc_ptr = self.next_temp();
                self.emit(&format!("  {} = call ptr @lmr_malloc(i64 16)", desc_ptr));

                let bytes_reg = self.next_temp();
                self.emit(&format!(
                    "  {} = call i64 @lmr_checked_array_bytes(i64 {})",
                    bytes_reg, sz_reg
                ));
                let data_ptr = self.next_temp();
                self.emit(&format!(
                    "  {} = call ptr @lmr_malloc(i64 {})",
                    data_ptr, bytes_reg
                ));

                let len_gep = self.next_temp();
                self.emit(&format!(
                    "  {} = getelementptr inbounds %struct.Array, ptr {}, i32 0, i32 0",
                    len_gep, desc_ptr
                ));
                self.emit(&format!("  store i64 {}, ptr {}", sz_reg, len_gep));

                let data_gep = self.next_temp();
                self.emit(&format!(
                    "  {} = getelementptr inbounds %struct.Array, ptr {}, i32 0, i32 1",
                    data_gep, desc_ptr
                ));
                self.emit(&format!("  store ptr {}, ptr {}", data_ptr, data_gep));

                (desc_ptr, Type::Array(Box::new(element_type.clone())))
            }

            Expr::NewMap { key_type, val_type } => {
                let desc_ptr = self.next_temp();
                self.emit(&format!("  {} = call ptr @lmr_malloc(i64 24)", desc_ptr));

                let keys_ptr = self.next_temp();
                self.emit(&format!("  {} = call ptr @lmr_malloc(i64 800)", keys_ptr));
                let vals_ptr = self.next_temp();
                self.emit(&format!("  {} = call ptr @lmr_malloc(i64 800)", vals_ptr));

                let size_gep = self.next_temp();
                self.emit(&format!(
                    "  {} = getelementptr inbounds %struct.Map, ptr {}, i32 0, i32 0",
                    size_gep, desc_ptr
                ));
                self.emit(&format!("  store i64 0, ptr {}", size_gep));

                let keys_gep = self.next_temp();
                self.emit(&format!(
                    "  {} = getelementptr inbounds %struct.Map, ptr {}, i32 0, i32 1",
                    keys_gep, desc_ptr
                ));
                self.emit(&format!("  store ptr {}, ptr {}", keys_ptr, keys_gep));

                let vals_gep = self.next_temp();
                self.emit(&format!(
                    "  {} = getelementptr inbounds %struct.Map, ptr {}, i32 0, i32 2",
                    vals_gep, desc_ptr
                ));
                self.emit(&format!("  store ptr {}, ptr {}", vals_ptr, vals_gep));

                (
                    desc_ptr,
                    Type::Map(Box::new(key_type.clone()), Box::new(val_type.clone())),
                )
            }

            Expr::IndexAccess { array, index } => {
                let (arr_reg, arr_type) = self.gen_expr(array);
                let (idx_reg, _) = self.gen_expr(index);

                if let Type::Array(inner) = arr_type {
                    let inner_llvm = self.llvm_type(&inner);
                    let gep_len = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds %struct.Array, ptr {}, i32 0, i32 0",
                        gep_len, arr_reg
                    ));
                    let len_reg = self.next_temp();
                    self.emit(&format!("  {} = load i64, ptr {}", len_reg, gep_len));
                    self.emit(&format!(
                        "  call void @lmr_check_array_index(i64 {}, i64 {})",
                        idx_reg, len_reg
                    ));
                    let gep_data = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds %struct.Array, ptr {}, i32 0, i32 1",
                        gep_data, arr_reg
                    ));
                    let data_ptr = self.next_temp();
                    self.emit(&format!("  {} = load ptr, ptr {}", data_ptr, gep_data));

                    let elem_gep = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds {}, ptr {}, i64 {}",
                        elem_gep, inner_llvm, data_ptr, idx_reg
                    ));
                    let reg = self.next_temp();
                    self.emit(&format!(
                        "  {} = load {}, ptr {}",
                        reg, inner_llvm, elem_gep
                    ));
                    (reg, *inner)
                } else if let Type::Map(k_type, v_type) = arr_type {
                    let v_llvm = self.llvm_type(&v_type);
                    let id = self.next_label_id();

                    let gep_sz = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds %struct.Map, ptr {}, i32 0, i32 0",
                        gep_sz, arr_reg
                    ));
                    let sz = self.next_temp();
                    self.emit(&format!("  {} = load i64, ptr {}", sz, gep_sz));

                    let gep_k = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds %struct.Map, ptr {}, i32 0, i32 1",
                        gep_k, arr_reg
                    ));
                    let k_arr = self.next_temp();
                    self.emit(&format!("  {} = load ptr, ptr {}", k_arr, gep_k));

                    let gep_v = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds %struct.Map, ptr {}, i32 0, i32 2",
                        gep_v, arr_reg
                    ));
                    let v_arr = self.next_temp();
                    self.emit(&format!("  {} = load ptr, ptr {}", v_arr, gep_v));

                    let idx_ptr = format!("%map_i_{}", id);
                    self.emit(&format!("  {} = alloca i64", idx_ptr));
                    self.emit(&format!("  store i64 0, ptr {}", idx_ptr));

                    let res_ptr = format!("%map_res_{}", id);
                    self.emit(&format!("  {} = alloca {}", res_ptr, v_llvm));

                    let cond_lbl = format!("map_cond_{}", id);
                    let body_lbl = format!("map_body_{}", id);
                    let match_lbl = format!("map_match_{}", id);
                    let inc_lbl = format!("map_inc_{}", id);
                    let missing_lbl = format!("map_missing_{}", id);
                    let end_lbl = format!("map_end_{}", id);

                    self.emit(&format!("  br label %{}", cond_lbl));
                    self.emit(&format!("{}:", cond_lbl));
                    let curr_i = self.next_temp();
                    self.emit(&format!("  {} = load i64, ptr {}", curr_i, idx_ptr));
                    let cmp_sz = self.next_temp();
                    self.emit(&format!("  {} = icmp slt i64 {}, {}", cmp_sz, curr_i, sz));
                    self.emit(&format!(
                        "  br i1 {}, label %{}, label %{}",
                        cmp_sz, body_lbl, missing_lbl
                    ));

                    self.emit(&format!("{}:", body_lbl));
                    let curr_k_gep = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds {}, ptr {}, i64 {}",
                        curr_k_gep,
                        self.llvm_type(&k_type),
                        k_arr,
                        curr_i
                    ));
                    let curr_k = self.next_temp();
                    self.emit(&format!(
                        "  {} = load {}, ptr {}",
                        curr_k,
                        self.llvm_type(&k_type),
                        curr_k_gep
                    ));

                    let cmp_key = self.next_temp();
                    if *k_type == Type::String {
                        let strcmp_res = self.next_temp();
                        self.emit(&format!(
                            "  {} = call i32 @strcmp(ptr {}, ptr {})",
                            strcmp_res, curr_k, idx_reg
                        ));
                        self.emit(&format!("  {} = icmp eq i32 {}, 0", cmp_key, strcmp_res));
                    } else {
                        self.emit(&format!(
                            "  {} = icmp eq {} {}, {}",
                            cmp_key,
                            self.llvm_type(&k_type),
                            curr_k,
                            idx_reg
                        ));
                    }

                    self.emit(&format!(
                        "  br i1 {}, label %{}, label %{}",
                        cmp_key, match_lbl, inc_lbl
                    ));

                    self.emit(&format!("{}:", match_lbl));
                    let curr_v_gep = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds {}, ptr {}, i64 {}",
                        curr_v_gep, v_llvm, v_arr, curr_i
                    ));
                    let curr_v = self.next_temp();
                    self.emit(&format!(
                        "  {} = load {}, ptr {}",
                        curr_v, v_llvm, curr_v_gep
                    ));
                    self.emit(&format!("  store {} {}, ptr {}", v_llvm, curr_v, res_ptr));
                    self.emit(&format!("  br label %{}", end_lbl));

                    self.emit(&format!("{}:", inc_lbl));
                    let next_i = self.next_temp();
                    self.emit(&format!("  {} = add i64 {}, 1", next_i, curr_i));
                    self.emit(&format!("  store i64 {}, ptr {}", next_i, idx_ptr));
                    self.emit(&format!("  br label %{}", cond_lbl));

                    self.emit(&format!("{}:", end_lbl));
                    let final_val = self.next_temp();
                    self.emit(&format!(
                        "  {} = load {}, ptr {}",
                        final_val, v_llvm, res_ptr
                    ));
                    let done_lbl = format!("map_done_{}", id);
                    self.emit(&format!("  br label %{}", done_lbl));
                    self.emit(&format!("{}:", missing_lbl));
                    self.emit("  call void @lmr_missing_map_key()");
                    self.emit("  unreachable");
                    self.emit(&format!("{}:", done_lbl));
                    (final_val, *v_type)
                } else if arr_type == Type::String {
                    let str_len = self.next_temp();
                    self.emit(&format!(
                        "  {} = call i64 @strlen(ptr {})",
                        str_len, arr_reg
                    ));
                    self.emit(&format!(
                        "  call void @lmr_check_array_index(i64 {}, i64 {})",
                        idx_reg, str_len
                    ));
                    let char_ptr = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds i8, ptr {}, i64 {}",
                        char_ptr, arr_reg, idx_reg
                    ));
                    let char_val = self.next_temp();
                    self.emit(&format!("  {} = load i8, ptr {}", char_val, char_ptr));

                    let new_str = self.next_temp();
                    self.emit(&format!("  {} = call ptr @lmr_malloc(i64 2)", new_str));
                    let slot0 = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds i8, ptr {}, i64 0",
                        slot0, new_str
                    ));
                    self.emit(&format!("  store i8 {}, ptr {}", char_val, slot0));
                    let slot1 = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds i8, ptr {}, i64 1",
                        slot1, new_str
                    ));
                    self.emit(&format!("  store i8 0, ptr {}", slot1));

                    (new_str, Type::String)
                } else {
                    self.fail_codegen(format!("indexación no soportada para tipo {:?}", arr_type));
                    ("undef".to_string(), Type::Int)
                }
            }

            Expr::New { class_name, args } => {
                let c_decl = self.required_class(class_name);
                let size_in_bytes = ((c_decl.fields.len() + 1) * 8).max(16);
                let ptr_reg = self.next_temp();
                self.emit(&format!(
                    "  {} = call ptr @lmr_malloc(i64 {})",
                    ptr_reg, size_in_bytes
                ));

                let vptr_gep = self.next_temp();
                self.emit(&format!(
                    "  {} = getelementptr inbounds %struct.{}, ptr {}, i32 0, i32 0",
                    vptr_gep, class_name, ptr_reg
                ));
                self.emit(&format!(
                    "  store ptr @vtable_{}, ptr {}",
                    class_name, vptr_gep
                ));

                if let Some((constructor_class, _)) =
                    self.find_class_method(class_name, "constructor")
                {
                    let mut comp_args = vec![format!("ptr {}", ptr_reg)];
                    for arg in args {
                        let (r, t) = self.gen_expr(arg);
                        comp_args.push(format!("{} {}", self.llvm_type(&t), r));
                    }
                    self.emit(&format!(
                        "  call void @{}_constructor({})",
                        constructor_class,
                        comp_args.join(", ")
                    ));
                }
                if let Some((destructor_class, _)) =
                    self.find_class_method(class_name, "destructor")
                {
                    self.emit(&format!(
                        "  call void @lmr_register_finalizer(ptr {}, ptr @{}_destructor)",
                        ptr_reg, destructor_class
                    ));
                }

                (ptr_reg, Type::Custom(class_name.clone()))
            }

            Expr::MemberAccess { object, member } => {
                if let Expr::Variable(v_name) = object.as_ref() {
                    let info = self.required_var(v_name);
                    if let Type::Custom(name) = &info.var_type {
                        if let Some(r_decl) = self.records.get(name).cloned() {
                            let idx = self.required_field_index(name, &r_decl.fields, member);
                            let f_type = self.required_field_type(&r_decl.fields, idx, name);
                            let gep = self.next_temp();
                            self.emit(&format!(
                                "  {} = getelementptr inbounds %struct.{}, ptr {}, i32 0, i32 {}",
                                gep, name, info.llvm_ptr, idx
                            ));
                            let reg = self.next_temp();
                            let llvm_t = self.llvm_type(&f_type);
                            self.emit(&format!("  {} = load {}, ptr {}", reg, llvm_t, gep));
                            return (reg, f_type);
                        } else if let Some(c_decl) = self.classes.get(name).cloned() {
                            let idx = self.required_field_index(name, &c_decl.fields, member);
                            let f_type = self.required_field_type(&c_decl.fields, idx, name);
                            let obj_ptr = self.next_temp();
                            self.emit(&format!("  {} = load ptr, ptr {}", obj_ptr, info.llvm_ptr));
                            let gep = self.next_temp();
                            self.emit(&format!(
                                "  {} = getelementptr inbounds %struct.{}, ptr {}, i32 0, i32 {}",
                                gep,
                                name,
                                obj_ptr,
                                idx + 1
                            ));
                            let reg = self.next_temp();
                            let llvm_t = self.llvm_type(&f_type);
                            self.emit(&format!("  {} = load {}, ptr {}", reg, llvm_t, gep));
                            return (reg, f_type);
                        }
                    }
                }

                let (obj_reg, obj_type) = self.gen_expr(object);
                if let Type::Custom(ref c_name) = obj_type {
                    let c_decl = self.required_class(c_name);
                    let idx = self.required_field_index(c_name, &c_decl.fields, member);
                    let f_type = self.required_field_type(&c_decl.fields, idx, c_name);
                    let gep = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds %struct.{}, ptr {}, i32 0, i32 {}",
                        gep,
                        c_name,
                        obj_reg,
                        idx + 1
                    ));
                    let reg = self.next_temp();
                    let llvm_t = self.llvm_type(&f_type);
                    self.emit(&format!("  {} = load {}, ptr {}", reg, llvm_t, gep));
                    (reg, f_type)
                } else {
                    self.fail_codegen(format!(
                        "acceso a miembro sobre tipo no compuesto: {:?}",
                        obj_type
                    ));
                    ("undef".to_string(), Type::Int)
                }
            }

            Expr::MethodCall {
                object,
                method,
                args,
            } => {
                let (obj_reg, obj_type) = self.gen_expr(object);
                if let Type::Custom(ref c_name) = obj_type {
                    let slots = self.class_vtables.get(c_name).cloned().unwrap_or_default();
                    let slot_idx = match slots.iter().position(|(m_name, _, _, _)| m_name == method)
                    {
                        Some(index) => index,
                        None => {
                            self.fail_codegen(format!(
                                "método '{}' no encontrado en la tabla virtual de '{}'",
                                method, c_name
                            ));
                            return ("undef".to_string(), Type::Int);
                        }
                    };
                    let ret_t = match slots.get(slot_idx) {
                        Some(slot) => slot.2.clone(),
                        None => {
                            self.fail_codegen(format!(
                                "índice de método inválido en la tabla virtual de '{}'",
                                c_name
                            ));
                            None
                        }
                    };
                    let ret_llvm = match &ret_t {
                        Some(t) => self.llvm_type(t),
                        None => "void",
                    };

                    let vtable_ptr = self.next_temp();
                    self.emit(&format!("  {} = load ptr, ptr {}", vtable_ptr, obj_reg));

                    let method_slot_gep = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds ptr, ptr {}, i64 {}",
                        method_slot_gep, vtable_ptr, slot_idx
                    ));
                    let fn_ptr = self.next_temp();
                    self.emit(&format!("  {} = load ptr, ptr {}", fn_ptr, method_slot_gep));

                    let mut comp_args = vec![format!("ptr {}", obj_reg)];
                    for arg in args {
                        let (r, t) = self.gen_expr(arg);
                        comp_args.push(format!("{} {}", self.llvm_type(&t), r));
                    }

                    if let Some(r_t) = ret_t {
                        let dest = self.next_temp();
                        self.emit(&format!(
                            "  {} = call {} {}({})",
                            dest,
                            ret_llvm,
                            fn_ptr,
                            comp_args.join(", ")
                        ));
                        return (dest, r_t);
                    } else {
                        self.emit(&format!("  call void {}({})", fn_ptr, comp_args.join(", ")));
                        return ("0".to_string(), Type::Int);
                    }
                } else {
                    self.fail_codegen(format!(
                        "llamada a método sobre tipo no compuesto: {:?}",
                        obj_type
                    ));
                    ("undef".to_string(), Type::Int)
                }
            }

            Expr::Unary { op, right } => {
                let (r_reg, r_type) = self.gen_expr(right);
                let dest = self.next_temp();
                match op {
                    UnaryOp::Neg => {
                        if r_type == Type::Int {
                            self.emit(&format!("  {} = sub i64 0, {}", dest, r_reg));
                        } else {
                            self.emit(&format!("  {} = fneg double {}", dest, r_reg));
                        }
                        (dest, r_type)
                    }
                    UnaryOp::Not => {
                        self.emit(&format!("  {} = xor i1 {}, true", dest, r_reg));
                        (dest, Type::Bool)
                    }
                }
            }

            Expr::Binary {
                op: BinaryOp::Add, ..
            } => self.gen_add_chain(expr),

            Expr::Binary { left, op, right } => {
                let (l_reg, l_type) = self.gen_expr(left);
                let (r_reg, _) = self.gen_expr(right);
                let dest = self.next_temp();

                match op {
                    BinaryOp::Add => {
                        if l_type == Type::Int {
                            self.emit(&format!("  {} = add i64 {}, {}", dest, l_reg, r_reg));
                            (dest, l_type)
                        } else if l_type == Type::Float {
                            self.emit(&format!("  {} = fadd double {}, {}", dest, l_reg, r_reg));
                            (dest, l_type)
                        } else if l_type == Type::String {
                            let l_len = self.next_temp();
                            self.emit(&format!("  {} = call i64 @strlen(ptr {})", l_len, l_reg));
                            let r_len = self.next_temp();
                            self.emit(&format!("  {} = call i64 @strlen(ptr {})", r_len, r_reg));
                            let tot_len = self.next_temp();
                            self.emit(&format!("  {} = add i64 {}, {}", tot_len, l_len, r_len));
                            let alloc_sz = self.next_temp();
                            self.emit(&format!("  {} = add i64 {}, 1", alloc_sz, tot_len));
                            let new_ptr = self.next_temp();
                            self.emit(&format!(
                                "  {} = call ptr @lmr_malloc(i64 {})",
                                new_ptr, alloc_sz
                            ));
                            let d1 = self.next_temp();
                            self.emit(&format!(
                                "  {} = call ptr @strcpy(ptr {}, ptr {})",
                                d1, new_ptr, l_reg
                            ));
                            let d2 = self.next_temp();
                            self.emit(&format!(
                                "  {} = call ptr @strcat(ptr {}, ptr {})",
                                d2, new_ptr, r_reg
                            ));
                            (new_ptr, Type::String)
                        } else {
                            self.fail_codegen(format!(
                                "operador + no soportado para tipo {:?}",
                                l_type
                            ));
                            ("undef".to_string(), Type::Int)
                        }
                    }
                    BinaryOp::Sub => {
                        if l_type == Type::Int {
                            self.emit(&format!("  {} = sub i64 {}, {}", dest, l_reg, r_reg));
                        } else {
                            self.emit(&format!("  {} = fsub double {}, {}", dest, l_reg, r_reg));
                        }
                        (dest, l_type)
                    }
                    BinaryOp::Mul => {
                        if l_type == Type::Int {
                            self.emit(&format!("  {} = mul i64 {}, {}", dest, l_reg, r_reg));
                        } else {
                            self.emit(&format!("  {} = fmul double {}, {}", dest, l_reg, r_reg));
                        }
                        (dest, l_type)
                    }
                    BinaryOp::Div => {
                        if l_type == Type::Int {
                            self.emit(&format!(
                                "  call void @lmr_check_integer_division(i64 {}, i64 {})",
                                l_reg, r_reg
                            ));
                            self.emit(&format!("  {} = sdiv i64 {}, {}", dest, l_reg, r_reg));
                        } else {
                            self.emit(&format!(
                                "  call void @lmr_check_float_division(double {})",
                                r_reg
                            ));
                            self.emit(&format!("  {} = fdiv double {}, {}", dest, l_reg, r_reg));
                        }
                        (dest, l_type)
                    }
                    BinaryOp::Shl => {
                        self.emit(&format!("  {} = shl i64 {}, {}", dest, l_reg, r_reg));
                        (dest, Type::Int)
                    }
                    BinaryOp::Shr => {
                        self.emit(&format!("  {} = ashr i64 {}, {}", dest, l_reg, r_reg));
                        (dest, Type::Int)
                    }
                    BinaryOp::Xor => {
                        self.emit(&format!("  {} = xor i64 {}, {}", dest, l_reg, r_reg));
                        (dest, Type::Int)
                    }
                    BinaryOp::BitAnd => {
                        self.emit(&format!("  {} = and i64 {}, {}", dest, l_reg, r_reg));
                        (dest, Type::Int)
                    }
                    BinaryOp::BitOr => {
                        self.emit(&format!("  {} = or i64 {}, {}", dest, l_reg, r_reg));
                        (dest, Type::Int)
                    }
                    BinaryOp::Equal => {
                        if l_type == Type::String {
                            let cmp_res = self.next_temp();
                            self.emit(&format!(
                                "  {} = call i32 @strcmp(ptr {}, ptr {})",
                                cmp_res, l_reg, r_reg
                            ));
                            self.emit(&format!("  {} = icmp eq i32 {}, 0", dest, cmp_res));
                        } else if l_type == Type::Float {
                            self.emit(&format!(
                                "  {} = fcmp oeq double {}, {}",
                                dest, l_reg, r_reg
                            ));
                        } else {
                            let llvm_t = self.llvm_type(&l_type);
                            self.emit(&format!(
                                "  {} = icmp eq {} {}, {}",
                                dest, llvm_t, l_reg, r_reg
                            ));
                        }
                        (dest, Type::Bool)
                    }
                    BinaryOp::NotEqual => {
                        if l_type == Type::String {
                            let cmp_res = self.next_temp();
                            self.emit(&format!(
                                "  {} = call i32 @strcmp(ptr {}, ptr {})",
                                cmp_res, l_reg, r_reg
                            ));
                            self.emit(&format!("  {} = icmp ne i32 {}, 0", dest, cmp_res));
                        } else if l_type == Type::Float {
                            self.emit(&format!(
                                "  {} = fcmp one double {}, {}",
                                dest, l_reg, r_reg
                            ));
                        } else {
                            let llvm_t = self.llvm_type(&l_type);
                            self.emit(&format!(
                                "  {} = icmp ne {} {}, {}",
                                dest, llvm_t, l_reg, r_reg
                            ));
                        }
                        (dest, Type::Bool)
                    }
                    BinaryOp::Less => {
                        if l_type == Type::Float {
                            self.emit(&format!(
                                "  {} = fcmp olt double {}, {}",
                                dest, l_reg, r_reg
                            ));
                        } else {
                            self.emit(&format!("  {} = icmp slt i64 {}, {}", dest, l_reg, r_reg));
                        }
                        (dest, Type::Bool)
                    }
                    BinaryOp::Greater => {
                        if l_type == Type::Float {
                            self.emit(&format!(
                                "  {} = fcmp ogt double {}, {}",
                                dest, l_reg, r_reg
                            ));
                        } else {
                            self.emit(&format!("  {} = icmp sgt i64 {}, {}", dest, l_reg, r_reg));
                        }
                        (dest, Type::Bool)
                    }
                    BinaryOp::LessEq => {
                        if l_type == Type::Float {
                            self.emit(&format!(
                                "  {} = fcmp ole double {}, {}",
                                dest, l_reg, r_reg
                            ));
                        } else {
                            self.emit(&format!("  {} = icmp sle i64 {}, {}", dest, l_reg, r_reg));
                        }
                        (dest, Type::Bool)
                    }
                    BinaryOp::GreaterEq => {
                        if l_type == Type::Float {
                            self.emit(&format!(
                                "  {} = fcmp oge double {}, {}",
                                dest, l_reg, r_reg
                            ));
                        } else {
                            self.emit(&format!("  {} = icmp sge i64 {}, {}", dest, l_reg, r_reg));
                        }
                        (dest, Type::Bool)
                    }
                    BinaryOp::And => {
                        self.emit(&format!("  {} = and i1 {}, {}", dest, l_reg, r_reg));
                        (dest, Type::Bool)
                    }
                    BinaryOp::Or => {
                        self.emit(&format!("  {} = or i1 {}, {}", dest, l_reg, r_reg));
                        (dest, Type::Bool)
                    }
                }
            }

            Expr::Call { callee, args } => {
                if callee == "contains" {
                    let (arr_reg, arr_type) = self.gen_arg(args, 0);
                    let (key_reg, _) = self.gen_arg(args, 1);

                    if let Type::Map(k_type, _) = arr_type {
                        let id = self.next_label_id();
                        let gep_sz = self.next_temp();
                        self.emit(&format!(
                            "  {} = getelementptr inbounds %struct.Map, ptr {}, i32 0, i32 0",
                            gep_sz, arr_reg
                        ));
                        let sz = self.next_temp();
                        self.emit(&format!("  {} = load i64, ptr {}", sz, gep_sz));

                        let gep_k = self.next_temp();
                        self.emit(&format!(
                            "  {} = getelementptr inbounds %struct.Map, ptr {}, i32 0, i32 1",
                            gep_k, arr_reg
                        ));
                        let k_arr = self.next_temp();
                        self.emit(&format!("  {} = load ptr, ptr {}", k_arr, gep_k));

                        let idx_ptr = format!("%map_i_{}", id);
                        self.emit(&format!("  {} = alloca i64", idx_ptr));
                        self.emit(&format!("  store i64 0, ptr {}", idx_ptr));

                        let res_ptr = format!("%map_res_{}", id);
                        self.emit(&format!("  {} = alloca i1", res_ptr));
                        self.emit(&format!("  store i1 0, ptr {}", res_ptr));

                        let cond_lbl = format!("map_cond_{}", id);
                        let body_lbl = format!("map_body_{}", id);
                        let match_lbl = format!("map_match_{}", id);
                        let inc_lbl = format!("map_inc_{}", id);
                        let end_lbl = format!("map_end_{}", id);

                        self.emit(&format!("  br label %{}", cond_lbl));
                        self.emit(&format!("{}:", cond_lbl));
                        let curr_i = self.next_temp();
                        self.emit(&format!("  {} = load i64, ptr {}", curr_i, idx_ptr));
                        let cmp_sz = self.next_temp();
                        self.emit(&format!("  {} = icmp slt i64 {}, {}", cmp_sz, curr_i, sz));
                        self.emit(&format!(
                            "  br i1 {}, label %{}, label %{}",
                            cmp_sz, body_lbl, end_lbl
                        ));

                        self.emit(&format!("{}:", body_lbl));
                        let curr_k_gep = self.next_temp();
                        self.emit(&format!(
                            "  {} = getelementptr inbounds {}, ptr {}, i64 {}",
                            curr_k_gep,
                            self.llvm_type(&k_type),
                            k_arr,
                            curr_i
                        ));
                        let curr_k = self.next_temp();
                        self.emit(&format!(
                            "  {} = load {}, ptr {}",
                            curr_k,
                            self.llvm_type(&k_type),
                            curr_k_gep
                        ));

                        let cmp_key = self.next_temp();
                        if *k_type == Type::String {
                            let strcmp_res = self.next_temp();
                            self.emit(&format!(
                                "  {} = call i32 @strcmp(ptr {}, ptr {})",
                                strcmp_res, curr_k, key_reg
                            ));
                            self.emit(&format!("  {} = icmp eq i32 {}, 0", cmp_key, strcmp_res));
                        } else {
                            self.emit(&format!(
                                "  {} = icmp eq {} {}, {}",
                                cmp_key,
                                self.llvm_type(&k_type),
                                curr_k,
                                key_reg
                            ));
                        }

                        self.emit(&format!(
                            "  br i1 {}, label %{}, label %{}",
                            cmp_key, match_lbl, inc_lbl
                        ));

                        self.emit(&format!("{}:", match_lbl));
                        self.emit(&format!("  store i1 1, ptr {}", res_ptr));
                        self.emit(&format!("  br label %{}", end_lbl));

                        self.emit(&format!("{}:", inc_lbl));
                        let next_i = self.next_temp();
                        self.emit(&format!("  {} = add i64 {}, 1", next_i, curr_i));
                        self.emit(&format!("  store i64 {}, ptr {}", next_i, idx_ptr));
                        self.emit(&format!("  br label %{}", cond_lbl));

                        self.emit(&format!("{}:", end_lbl));
                        let final_val = self.next_temp();
                        self.emit(&format!("  {} = load i1, ptr {}", final_val, res_ptr));
                        return (final_val, Type::Bool);
                    }
                }

                if callee == "jsonParse" {
                    let (v, _) = self.gen_arg(args, 0);
                    let res = self.next_temp();
                    self.emit(&format!("  {} = call ptr @lmr_json_parse(ptr {})", res, v));
                    return (
                        res,
                        Type::Map(Box::new(Type::String), Box::new(Type::String)),
                    );
                }

                if callee == "len" {
                    let (val_reg, val_type) = self.gen_arg(args, 0);
                    if val_type == Type::String {
                        let len_reg = self.next_temp();
                        self.emit(&format!(
                            "  {} = call i64 @strlen(ptr {})",
                            len_reg, val_reg
                        ));
                        return (len_reg, Type::Int);
                    } else {
                        let len_gep = self.next_temp();
                        self.emit(&format!(
                            "  {} = getelementptr inbounds %struct.Array, ptr {}, i32 0, i32 0",
                            len_gep, val_reg
                        ));
                        let len_reg = self.next_temp();
                        self.emit(&format!("  {} = load i64, ptr {}", len_reg, len_gep));
                        return (len_reg, Type::Int);
                    }
                }

                if callee == "toUpper" || callee == "toLower" {
                    let (s_reg, _) = self.gen_arg(args, 0);
                    let len_reg = self.next_temp();
                    self.emit(&format!("  {} = call i64 @strlen(ptr {})", len_reg, s_reg));
                    let alloc_sz = self.next_temp();
                    self.emit(&format!("  {} = add i64 {}, 1", alloc_sz, len_reg));
                    let new_ptr = self.next_temp();
                    self.emit(&format!(
                        "  {} = call ptr @lmr_malloc(i64 {})",
                        new_ptr, alloc_sz
                    ));

                    let id = self.next_label_id();
                    let cond_lbl = format!("case_cond_{}", id);
                    let body_lbl = format!("case_body_{}", id);
                    let end_lbl = format!("case_end_{}", id);
                    let i_ptr = format!("%case_i_{}", id);
                    self.emit(&format!("  {} = alloca i64", i_ptr));
                    self.emit(&format!("  store i64 0, ptr {}", i_ptr));

                    self.emit(&format!("  br label %{}", cond_lbl));
                    self.emit(&format!("{}:", cond_lbl));
                    let i_val = self.next_temp();
                    self.emit(&format!("  {} = load i64, ptr {}", i_val, i_ptr));
                    let cmp = self.next_temp();
                    self.emit(&format!("  {} = icmp slt i64 {}, {}", cmp, i_val, len_reg));
                    self.emit(&format!(
                        "  br i1 {}, label %{}, label %{}",
                        cmp, body_lbl, end_lbl
                    ));

                    self.emit(&format!("{}:", body_lbl));
                    let src_gep = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds i8, ptr {}, i64 {}",
                        src_gep, s_reg, i_val
                    ));
                    let char_byte = self.next_temp();
                    self.emit(&format!("  {} = load i8, ptr {}", char_byte, src_gep));
                    let char_i32 = self.next_temp();
                    self.emit(&format!("  {} = sext i8 {} to i32", char_i32, char_byte));

                    let conv_fn = if callee == "toUpper" {
                        "@toupper"
                    } else {
                        "@tolower"
                    };
                    let conv_i32 = self.next_temp();
                    self.emit(&format!(
                        "  {} = call i32 {}(i32 {})",
                        conv_i32, conv_fn, char_i32
                    ));
                    let conv_byte = self.next_temp();
                    self.emit(&format!("  {} = trunc i32 {} to i8", conv_byte, conv_i32));

                    let dst_gep = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds i8, ptr {}, i64 {}",
                        dst_gep, new_ptr, i_val
                    ));
                    self.emit(&format!("  store i8 {}, ptr {}", conv_byte, dst_gep));

                    let next_i = self.next_temp();
                    self.emit(&format!("  {} = add i64 {}, 1", next_i, i_val));
                    self.emit(&format!("  store i64 {}, ptr {}", next_i, i_ptr));
                    self.emit(&format!("  br label %{}", cond_lbl));

                    self.emit(&format!("{}:", end_lbl));
                    let term_gep = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds i8, ptr {}, i64 {}",
                        term_gep, new_ptr, len_reg
                    ));
                    self.emit(&format!("  store i8 0, ptr {}", term_gep));
                    return (new_ptr, Type::String);
                }

                if callee == "intToString" {
                    let (n_reg, n_type) = self.gen_arg(args, 0);
                    let final_reg = if n_type == Type::Float {
                        let conv = self.next_temp();
                        self.emit(&format!("  {} = fptosi double {} to i64", conv, n_reg));
                        conv
                    } else {
                        n_reg
                    };
                    let buf_ptr = self.next_temp();
                    self.emit(&format!("  {} = call ptr @lmr_malloc(i64 32)", buf_ptr));
                    let d = self.next_temp();
                    self.emit(&format!("  {} = call i32 (ptr, i64, ptr, ...) @snprintf(ptr {}, i64 32, ptr @.fmt_int, i64 {})", d, buf_ptr, final_reg));
                    return (buf_ptr, Type::String);
                }

                if callee == "floatToString" {
                    let (n_reg, n_type) = self.gen_arg(args, 0);
                    let float_reg = if n_type == Type::Int {
                        let converted = self.next_temp();
                        self.emit(&format!("  {} = sitofp i64 {} to double", converted, n_reg));
                        converted
                    } else {
                        n_reg
                    };
                    let buf_ptr = self.next_temp();
                    self.emit(&format!("  {} = call ptr @lmr_malloc(i64 32)", buf_ptr));
                    let d = self.next_temp();
                    self.emit(&format!("  {} = call i32 (ptr, i64, ptr, ...) @snprintf(ptr {}, i64 32, ptr @.fmt_float, double {})", d, buf_ptr, float_reg));
                    return (buf_ptr, Type::String);
                }

                if callee == "posString" {
                    let (sub_reg, _) = self.gen_arg(args, 0);
                    let (txt_reg, _) = self.gen_arg(args, 1);
                    let found_ptr = self.next_temp();
                    self.emit(&format!(
                        "  {} = call ptr @strstr(ptr {}, ptr {})",
                        found_ptr, txt_reg, sub_reg
                    ));

                    let is_null = self.next_temp();
                    self.emit(&format!("  {} = icmp eq ptr {}, null", is_null, found_ptr));

                    let id = self.next_label_id();
                    let not_found_lbl = format!("pos_nf_{}", id);
                    let found_lbl = format!("pos_f_{}", id);
                    let end_lbl = format!("pos_end_{}", id);
                    let res_ptr = format!("%pos_res_{}", id);
                    self.emit(&format!("  {} = alloca i64", res_ptr));

                    self.emit(&format!(
                        "  br i1 {}, label %{}, label %{}",
                        is_null, not_found_lbl, found_lbl
                    ));
                    self.emit(&format!("{}:", not_found_lbl));
                    self.emit(&format!("  store i64 -1, ptr {}", res_ptr));
                    self.emit(&format!("  br label %{}", end_lbl));

                    self.emit(&format!("{}:", found_lbl));
                    let p1 = self.next_temp();
                    self.emit(&format!("  {} = ptrtoint ptr {} to i64", p1, found_ptr));
                    let p2 = self.next_temp();
                    self.emit(&format!("  {} = ptrtoint ptr {} to i64", p2, txt_reg));
                    let diff = self.next_temp();
                    self.emit(&format!("  {} = sub i64 {}, {}", diff, p1, p2));
                    self.emit(&format!("  store i64 {}, ptr {}", diff, res_ptr));
                    self.emit(&format!("  br label %{}", end_lbl));

                    self.emit(&format!("{}:", end_lbl));
                    let out_reg = self.next_temp();
                    self.emit(&format!("  {} = load i64, ptr {}", out_reg, res_ptr));
                    return (out_reg, Type::Int);
                }

                if callee == "SliceString" {
                    let (txt_reg, _) = self.gen_arg(args, 0);
                    let (start_reg, _) = self.gen_arg(args, 1);
                    let full_len = self.next_temp();
                    self.emit(&format!(
                        "  {} = call i64 @strlen(ptr {})",
                        full_len, txt_reg
                    ));

                    let count_reg = if args.len() == 3 {
                        let (user_count, _) = self.gen_arg(args, 2);
                        user_count
                    } else {
                        let rem = self.next_temp();
                        self.emit(&format!("  {} = sub i64 {}, {}", rem, full_len, start_reg));
                        rem
                    };

                    let alloc_sz = self.next_temp();
                    self.emit(&format!("  {} = add i64 {}, 1", alloc_sz, count_reg));
                    let new_ptr = self.next_temp();
                    self.emit(&format!(
                        "  {} = call ptr @lmr_malloc(i64 {})",
                        new_ptr, alloc_sz
                    ));
                    let src_gep = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds i8, ptr {}, i64 {}",
                        src_gep, txt_reg, start_reg
                    ));
                    let d = self.next_temp();
                    self.emit(&format!(
                        "  {} = call ptr @memcpy(ptr {}, ptr {}, i64 {})",
                        d, new_ptr, src_gep, count_reg
                    ));
                    let term_gep = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds i8, ptr {}, i64 {}",
                        term_gep, new_ptr, count_reg
                    ));
                    self.emit(&format!("  store i8 0, ptr {}", term_gep));
                    return (new_ptr, Type::String);
                }

                if callee == "readln" && args.is_empty() {
                    let id = self.next_label_id();
                    let cond_lbl = format!("rl_cond_{}", id);
                    let body_lbl = format!("rl_body_{}", id);
                    let end_lbl = format!("rl_end_{}", id);

                    self.emit(&format!("  br label %{}", cond_lbl));
                    self.emit(&format!("{}:", cond_lbl));
                    let c = self.next_temp();
                    self.emit(&format!("  {} = call i32 @getchar()", c));
                    let is_nl = self.next_temp();
                    self.emit(&format!("  {} = icmp eq i32 {}, 10", is_nl, c));
                    let is_eof = self.next_temp();
                    self.emit(&format!("  {} = icmp eq i32 {}, -1", is_eof, c));
                    let is_done = self.next_temp();
                    self.emit(&format!("  {} = or i1 {}, {}", is_done, is_nl, is_eof));
                    self.emit(&format!(
                        "  br i1 {}, label %{}, label %{}",
                        is_done, end_lbl, body_lbl
                    ));

                    self.emit(&format!("{}:", body_lbl));
                    self.emit(&format!("  br label %{}", cond_lbl));

                    self.emit(&format!("{}:", end_lbl));
                    let empty_str = self.next_temp();
                    self.emit(&format!("  {} = call ptr @lmr_malloc(i64 1)", empty_str));
                    self.emit(&format!("  store i8 0, ptr {}", empty_str));
                    return (empty_str, Type::String);
                }

                if callee == "sleep" {
                    let (v, _) = self.gen_arg(args, 0);
                    let trunc = self.next_temp();
                    self.emit(&format!("  {} = trunc i64 {} to i32", trunc, v));
                    self.emit(&format!("  call void @lmr_sleep(i32 {})", trunc));
                    return ("0".to_string(), Type::Int);
                }

                if callee == "readFile" {
                    let (path_reg, _) = self.gen_arg(args, 0);
                    let f_ptr = self.next_temp();
                    self.emit(&format!(
                        "  {} = call ptr @fopen(ptr {}, ptr @.mode_rb)",
                        f_ptr, path_reg
                    ));

                    let id = self.next_label_id();
                    let ok_lbl = format!("rf_ok_{}", id);
                    let err_lbl = format!("rf_err_{}", id);
                    let end_lbl = format!("rf_end_{}", id);
                    let out_ptr = format!("%rf_out_{}", id);
                    self.emit(&format!("  {} = alloca ptr", out_ptr));

                    let is_null = self.next_temp();
                    self.emit(&format!("  {} = icmp eq ptr {}, null", is_null, f_ptr));
                    self.emit(&format!(
                        "  br i1 {}, label %{}, label %{}",
                        is_null, err_lbl, ok_lbl
                    ));

                    self.emit(&format!("{}:", err_lbl));
                    let empty_buf = self.next_temp();
                    self.emit(&format!("  {} = call ptr @lmr_malloc(i64 1)", empty_buf));
                    self.emit(&format!("  store i8 0, ptr {}", empty_buf));
                    self.emit(&format!("  store ptr {}, ptr {}", empty_buf, out_ptr));
                    self.emit(&format!("  br label %{}", end_lbl));

                    self.emit(&format!("{}:", ok_lbl));
                    let d1 = self.next_temp();
                    self.emit(&format!(
                        "  {} = call i32 @fseek(ptr {}, i64 0, i32 2)",
                        d1, f_ptr
                    ));
                    let size_reg = self.next_temp();
                    self.emit(&format!("  {} = call i64 @ftell(ptr {})", size_reg, f_ptr));
                    let d2 = self.next_temp();
                    self.emit(&format!(
                        "  {} = call i32 @fseek(ptr {}, i64 0, i32 0)",
                        d2, f_ptr
                    ));

                    let alloc_sz = self.next_temp();
                    self.emit(&format!("  {} = add i64 {}, 1", alloc_sz, size_reg));
                    let buf_reg = self.next_temp();
                    self.emit(&format!(
                        "  {} = call ptr @lmr_malloc(i64 {})",
                        buf_reg, alloc_sz
                    ));
                    let d3 = self.next_temp();
                    self.emit(&format!(
                        "  {} = call i64 @fread(ptr {}, i64 1, i64 {}, ptr {})",
                        d3, buf_reg, size_reg, f_ptr
                    ));
                    let term_gep = self.next_temp();
                    self.emit(&format!(
                        "  {} = getelementptr inbounds i8, ptr {}, i64 {}",
                        term_gep, buf_reg, size_reg
                    ));
                    self.emit(&format!("  store i8 0, ptr {}", term_gep));
                    let d4 = self.next_temp();
                    self.emit(&format!("  {} = call i32 @fclose(ptr {})", d4, f_ptr));
                    self.emit(&format!("  store ptr {}, ptr {}", buf_reg, out_ptr));
                    self.emit(&format!("  br label %{}", end_lbl));

                    self.emit(&format!("{}:", end_lbl));
                    let final_str = self.next_temp();
                    self.emit(&format!("  {} = load ptr, ptr {}", final_str, out_ptr));
                    return (final_str, Type::String);
                }

                if callee == "writeFile" {
                    let (path_reg, _) = self.gen_arg(args, 0);
                    let (data_reg, _) = self.gen_arg(args, 1);
                    let f_ptr = self.next_temp();
                    self.emit(&format!(
                        "  {} = call ptr @fopen(ptr {}, ptr @.mode_wb)",
                        f_ptr, path_reg
                    ));

                    let id = self.next_label_id();
                    let ok_lbl = format!("wf_ok_{}", id);
                    let err_lbl = format!("wf_err_{}", id);
                    let end_lbl = format!("wf_end_{}", id);
                    let res_ptr = format!("%wf_res_{}", id);
                    self.emit(&format!("  {} = alloca i1", res_ptr));

                    let is_null = self.next_temp();
                    self.emit(&format!("  {} = icmp eq ptr {}, null", is_null, f_ptr));
                    self.emit(&format!(
                        "  br i1 {}, label %{}, label %{}",
                        is_null, err_lbl, ok_lbl
                    ));

                    self.emit(&format!("{}:", err_lbl));
                    self.emit(&format!("  store i1 0, ptr {}", res_ptr));
                    self.emit(&format!("  br label %{}", end_lbl));

                    self.emit(&format!("{}:", ok_lbl));
                    let len_reg = self.next_temp();
                    self.emit(&format!(
                        "  {} = call i64 @strlen(ptr {})",
                        len_reg, data_reg
                    ));
                    let d1 = self.next_temp();
                    self.emit(&format!(
                        "  {} = call i64 @fwrite(ptr {}, i64 1, i64 {}, ptr {})",
                        d1, data_reg, len_reg, f_ptr
                    ));
                    let d2 = self.next_temp();
                    self.emit(&format!("  {} = call i32 @fclose(ptr {})", d2, f_ptr));
                    self.emit(&format!("  store i1 1, ptr {}", res_ptr));
                    self.emit(&format!("  br label %{}", end_lbl));

                    self.emit(&format!("{}:", end_lbl));
                    let final_bool = self.next_temp();
                    self.emit(&format!("  {} = load i1, ptr {}", final_bool, res_ptr));
                    return (final_bool, Type::Bool);
                }

                if let Some(intrinsic) = match callee.as_str() {
                    "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "exp" | "log" | "log10"
                    | "floor" | "ceil" | "round" => Some(callee.as_str()),
                    _ => None,
                } {
                    let value = self.gen_float_arg(args, 0);
                    let dest = self.next_temp();
                    self.emit(&format!(
                        "  {} = call double @{}(double {})",
                        dest, intrinsic, value
                    ));
                    return (dest, Type::Float);
                }

                if callee == "sqrt" {
                    let value = self.gen_float_arg(args, 0);
                    let dest = self.next_temp();
                    self.emit(&format!("  {} = call double @sqrt(double {})", dest, value));
                    return (dest, Type::Float);
                }

                if callee == "pow" {
                    let b = self.gen_float_arg(args, 0);
                    let e = self.gen_float_arg(args, 1);
                    let dest = self.next_temp();
                    self.emit(&format!(
                        "  {} = call double @pow(double {}, double {})",
                        dest, b, e
                    ));
                    return (dest, Type::Float);
                }

                if callee == "atan2" || callee == "fmod" {
                    let a = self.gen_float_arg(args, 0);
                    let b = self.gen_float_arg(args, 1);
                    let dest = self.next_temp();
                    self.emit(&format!(
                        "  {} = call double @{}(double {}, double {})",
                        dest, callee, a, b
                    ));
                    return (dest, Type::Float);
                }

                if callee == "abs" {
                    let (v, t) = self.gen_arg(args, 0);
                    if t == Type::Float {
                        let dest = self.next_temp();
                        self.emit(&format!("  {} = call double @fabs(double {})", dest, v));
                        return (dest, Type::Float);
                    } else {
                        let id = self.next_label_id();
                        let neg_lbl = format!("abs_neg_{}", id);
                        let pos_lbl = format!("abs_pos_{}", id);
                        let end_lbl = format!("abs_end_{}", id);
                        let ptr = format!("%abs_res_{}", id);
                        self.emit(&format!("  {} = alloca i64", ptr));
                        let is_neg = self.next_temp();
                        self.emit(&format!("  {} = icmp slt i64 {}, 0", is_neg, v));
                        self.emit(&format!(
                            "  br i1 {}, label %{}, label %{}",
                            is_neg, neg_lbl, pos_lbl
                        ));

                        self.emit(&format!("{}:", neg_lbl));
                        let inv = self.next_temp();
                        self.emit(&format!("  {} = sub i64 0, {}", inv, v));
                        self.emit(&format!("  store i64 {}, ptr {}", inv, ptr));
                        self.emit(&format!("  br label %{}", end_lbl));

                        self.emit(&format!("{}:", pos_lbl));
                        self.emit(&format!("  store i64 {}, ptr {}", v, ptr));
                        self.emit(&format!("  br label %{}", end_lbl));

                        self.emit(&format!("{}:", end_lbl));
                        let out = self.next_temp();
                        self.emit(&format!("  {} = load i64, ptr {}", out, ptr));
                        return (out, Type::Int);
                    }
                }

                if callee == "dispose" {
                    if args.len() == 1 {
                        if let Some(Expr::Variable(v_name)) = args.first() {
                            if let Some(info) = self.lookup_var(v_name) {
                                let target_ptr = self.next_temp();
                                self.emit(&format!(
                                    "  {} = load ptr, ptr {}",
                                    target_ptr, info.llvm_ptr
                                ));
                                self.emit(&format!("  call void @lmr_free(ptr {})", target_ptr));
                                self.emit(&format!("  store ptr null, ptr {}", info.llvm_ptr));
                            }
                        }
                    }
                    return ("1".to_string(), Type::Bool);
                }

                if let Some(ref c_name) = self.current_class {
                    if let Some((target_class, m_decl)) = self.find_class_method(c_name, callee) {
                        let ret_llvm = match &m_decl.return_type {
                            Some(t) => self.llvm_type(t),
                            None => "void",
                        };
                        let this_info = self.required_var("this");
                        let this_reg = self.next_temp();
                        self.emit(&format!(
                            "  {} = load ptr, ptr {}",
                            this_reg, this_info.llvm_ptr
                        ));

                        let mut comp_args = vec![format!("ptr {}", this_reg)];
                        for arg in args {
                            let (r, t) = self.gen_expr(arg);
                            comp_args.push(format!("{} {}", self.llvm_type(&t), r));
                        }

                        if let Some(ref ret_t) = m_decl.return_type {
                            let dest = self.next_temp();
                            self.emit(&format!(
                                "  {} = call {} @{}_{}({})",
                                dest,
                                ret_llvm,
                                target_class,
                                callee,
                                comp_args.join(", ")
                            ));
                            return (dest, ret_t.clone());
                        } else {
                            self.emit(&format!(
                                "  call void @{}_{}({})",
                                target_class,
                                callee,
                                comp_args.join(", ")
                            ));
                            return ("0".to_string(), Type::Int);
                        }
                    }
                }

                let mut compiled_args = Vec::new();
                for arg in args {
                    let (r, t) = self.gen_expr(arg);
                    compiled_args.push(format!("{} {}", self.llvm_type(&t), r));
                }

                let ret_type = self
                    .functions
                    .get(callee)
                    .cloned()
                    .flatten()
                    .unwrap_or(Type::Int);
                let llvm_ret = self.llvm_type(&ret_type);
                let dest = self.next_temp();
                self.emit(&format!(
                    "  {} = call {} @{}({})",
                    dest,
                    llvm_ret,
                    callee,
                    compiled_args.join(", ")
                ));
                (dest, ret_type)
            }
        }
    }

    fn llvm_type(&self, t: &Type) -> &'static str {
        match t {
            Type::Int => "i64",
            Type::Float => "double",
            Type::Bool => "i1",
            Type::String => "ptr",
            Type::Char => "i8",
            Type::Array(_) => "ptr",
            Type::Map(_, _) => "ptr",
            Type::Custom(_) => "ptr",
        }
    }

    fn emit(&mut self, line: &str) {
        self.output.push_str(line);
        self.output.push('\n');
    }
    fn next_temp(&mut self) -> String {
        self.temp_counter += 1;
        format!("%t{}", self.temp_counter)
    }
    fn next_temp_id(&mut self) -> usize {
        self.temp_counter += 1;
        self.temp_counter
    }
    fn next_label_id(&mut self) -> usize {
        self.label_counter += 1;
        self.label_counter
    }
    fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
        self.scope_roots.push(Vec::new());
    }
    fn exit_scope(&mut self) {
        self.scopes.pop();
        self.scope_roots.pop();
    }

    fn insert_var(&mut self, name: &str, llvm_ptr: &str, var_type: Type, is_local: bool) {
        let has_root = matches!(var_type, Type::String | Type::Array(_) | Type::Map(..))
            || matches!(var_type, Type::Custom(ref name) if name != "ptr");
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(
                name.to_string(),
                VarInfo {
                    llvm_ptr: llvm_ptr.to_string(),
                    var_type,
                    is_local,
                },
            );
        }
        if has_root {
            self.emit(&format!("  call void @lmr_register_root(ptr {})", llvm_ptr));
            if let Some(roots) = self.scope_roots.last_mut() {
                roots.push(llvm_ptr.to_string());
            }
        }
    }

    fn lookup_var(&self, name: &str) -> Option<VarInfo> {
        for scope in self.scopes.iter().rev() {
            if let Some(info) = scope.get(name) {
                return Some(info.clone());
            }
        }
        None
    }

    fn fail_codegen(&mut self, message: impl Into<String>) {
        if self.codegen_error.is_none() {
            self.codegen_error = Some(message.into());
        }
    }

    fn required_class(&mut self, name: &str) -> ClassDecl {
        match self.classes.get(name).cloned() {
            Some(class) => class,
            None => {
                self.fail_codegen(format!(
                    "clase '{}' no disponible durante la generación",
                    name
                ));
                ClassDecl {
                    name: name.to_string(),
                    line: 0,
                    column: 0,
                    source_file: String::new(),
                    parent: None,
                    fields: Vec::new(),
                    methods: Vec::new(),
                }
            }
        }
    }

    fn required_var(&mut self, name: &str) -> VarInfo {
        match self.lookup_var(name) {
            Some(info) => info,
            None => {
                self.fail_codegen(format!(
                    "variable '{}' no disponible durante la generación",
                    name
                ));
                VarInfo {
                    llvm_ptr: "null".to_string(),
                    var_type: Type::Int,
                    is_local: false,
                }
            }
        }
    }

    fn required_field_index(&mut self, owner: &str, fields: &[FieldDecl], name: &str) -> usize {
        match fields.iter().position(|field| field.name == name) {
            Some(index) => index,
            None => {
                self.fail_codegen(format!(
                    "campo '{}' no encontrado en '{}' durante la generación",
                    name, owner
                ));
                0
            }
        }
    }

    fn required_field_type(&mut self, fields: &[FieldDecl], index: usize, owner: &str) -> Type {
        match fields.get(index) {
            Some(field) => field.field_type.clone(),
            None => {
                self.fail_codegen(format!(
                    "índice de campo inválido en '{}' durante la generación",
                    owner
                ));
                Type::Int
            }
        }
    }

    fn gen_arg(&mut self, args: &[Expr], index: usize) -> (String, Type) {
        match args.get(index) {
            Some(expr) => self.gen_expr(expr),
            None => {
                self.fail_codegen(format!(
                    "falta el argumento {} durante la generación",
                    index + 1
                ));
                ("0".to_string(), Type::Int)
            }
        }
    }

    fn gen_float_arg(&mut self, args: &[Expr], index: usize) -> String {
        let (value, value_type) = self.gen_arg(args, index);
        if value_type == Type::Int {
            let converted = self.next_temp();
            self.emit(&format!("  {} = sitofp i64 {} to double", converted, value));
            converted
        } else {
            value
        }
    }
}
