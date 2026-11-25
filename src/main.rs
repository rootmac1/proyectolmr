// src/main.rs

#[allow(dead_code)]
mod ast;
mod codegen;
mod lexer;
mod parser;
mod sema;
#[allow(dead_code)]
mod token;

use ast::Program;
use codegen::CodeGenerator;
use lexer::Lexer;
use parser::Parser;
use sema::SemanticAnalyzer;

use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

fn print_help() {
    println!("Uso de LMR Compiler:");
    println!("  lmr                              Inicia la consola interactiva (REPL)");
    println!("  lmr <archivo.lmr>                Compila a ejecutable nativo");
    println!("  lmr run <archivo.lmr>            Compila y ejecuta inmediatamente");
    println!("  lmr check <archivo.lmr>          Valida sintaxis, tipos e imports");
    println!("  lmr check --stdin <archivo.lmr>  Valida código enviado por stdin");
    println!("  lmr <archivo.lmr> -c             Genera archivo objeto (.o)");
    println!("  lmr <archivo.lmr> -o <salida>    Especifica el nombre del binario");
    println!("  lmr <archivo.lmr> --target <tri> Compilación cruzada");
    println!("  lmr --version                    Muestra la versión del compilador");
    println!("  Variable LMR_MODULE_PATH         Rutas adicionales para módulos comunitarios");
}

fn annotate_statement_source(stmt: &mut ast::Stmt, source_file: &str) {
    use ast::Stmt;
    match stmt {
        Stmt::Located {
            source_file: file,
            statement,
            ..
        } => {
            *file = source_file.to_string();
            annotate_statement_source(statement, source_file);
        }
        Stmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            for child in then_branch {
                annotate_statement_source(child, source_file);
            }
            if let Some(branch) = else_branch {
                for child in branch {
                    annotate_statement_source(child, source_file);
                }
            }
        }
        Stmt::While { body, .. } | Stmt::ForRange { body, .. } | Stmt::ForIn { body, .. } => {
            for child in body {
                annotate_statement_source(child, source_file);
            }
        }
        Stmt::TryCatch {
            try_block,
            catch_block,
            ..
        } => {
            for child in try_block {
                annotate_statement_source(child, source_file);
            }
            for child in catch_block {
                annotate_statement_source(child, source_file);
            }
        }
        _ => {}
    }
}

fn annotate_function_source(func: &mut ast::FunctionDecl, source_file: &str) {
    func.source_file = source_file.to_string();
    for stmt in &mut func.body {
        annotate_statement_source(stmt, source_file);
    }
}

fn annotate_program_source(program: &mut Program, source_file: &str) {
    for record in &mut program.records {
        record.source_file = source_file.to_string();
        for field in &mut record.fields {
            field.source_file = source_file.to_string();
        }
    }
    for ext in &mut program.extern_funcs {
        ext.source_file = source_file.to_string();
    }
    for class in &mut program.classes {
        class.source_file = source_file.to_string();
        for field in &mut class.fields {
            field.source_file = source_file.to_string();
        }
        for method in &mut class.methods {
            annotate_function_source(method, source_file);
        }
    }
    for func in &mut program.functions {
        annotate_function_source(func, source_file);
    }
    for stmt in &mut program.main_block {
        annotate_statement_source(stmt, source_file);
    }
}

fn report_source_error(kind: &str, path: &Path, source: &str, error: &str) {
    let source_path = error
        .rfind("archivo: ")
        .and_then(|start| {
            let rest = &error[start + "archivo: ".len()..];
            let end = rest.find(", línea ")?;
            Some(PathBuf::from(&rest[..end]))
        })
        .unwrap_or_else(|| path.to_path_buf());
    let source_text = fs::read_to_string(&source_path).unwrap_or_else(|_| source.to_string());
    let location = error.rfind("línea ").and_then(|line_start| {
        let line_text = &error[line_start + "línea ".len()..];
        let line_end = line_text.find(|c: char| !c.is_ascii_digit())?;
        let line = line_text[..line_end].parse::<usize>().ok()?;
        let col_marker = line_text
            .find("columna ")
            .map(|i| (i, "columna "))
            .or_else(|| line_text.find("col ").map(|i| (i, "col ")))?;
        let col_text = &line_text[col_marker.0 + col_marker.1.len()..];
        let col_end = col_text
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(col_text.len());
        let col = col_text[..col_end].parse::<usize>().ok()?;
        Some((line, col))
    });

    if let Some((line, col)) = location {
        eprintln!(
            "❌ Error {} en {}:{}:{}: {}",
            kind,
            source_path.display(),
            line,
            col,
            error
        );
        if let Some(source_line) = source_text.lines().nth(line.saturating_sub(1)) {
            eprintln!(" {:>4} | {}", line, source_line);
            eprintln!("      | {}^", " ".repeat(col.saturating_sub(1)));
        }
    } else {
        eprintln!(
            "❌ Error {} en '{}': {}",
            kind,
            source_path.display(),
            error
        );
    }
}

fn generate_llvm(program: &Program) -> Result<String, String> {
    CodeGenerator::new().generate(program)
}

// Recursos incluidos en una distribución portable viven junto al ejecutable.
// Durante el desarrollo, se usa el checkout indicado por Cargo como fallback.
fn compiler_asset_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = env::var_os("LMR_HOME") {
        roots.push(PathBuf::from(home));
    }
    if let Ok(executable) = env::current_exe() {
        if let Some(parent) = executable.parent() {
            roots.push(parent.to_path_buf());
        }
    }
    roots.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    let mut unique = HashSet::new();
    roots.retain(|root| unique.insert(root.clone()));
    roots
}

fn resolve_compiler_asset(relative_path: &Path) -> Option<PathBuf> {
    compiler_asset_roots()
        .into_iter()
        .map(|root| root.join(relative_path))
        .find(|path| path.is_file())
}

fn user_module_root() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        env::var_os("LOCALAPPDATA")
            .or_else(|| env::var_os("APPDATA"))
            .map(PathBuf::from)
            .map(|root| root.join("LMR"))
    }
    #[cfg(not(windows))]
    {
        if let Some(data_home) = env::var_os("XDG_DATA_HOME") {
            let path = PathBuf::from(data_home);
            if path.is_absolute() {
                return Some(path.join("lmr"));
            }
        }
        env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join(".local").join("share").join("lmr"))
    }
}

fn module_search_roots(project_root: &Path) -> Vec<PathBuf> {
    let mut roots = vec![project_root.to_path_buf()];
    if let Some(paths) = env::var_os("LMR_MODULE_PATH") {
        roots.extend(env::split_paths(&paths));
    }
    if let Some(user_root) = user_module_root() {
        roots.push(user_root);
    }
    roots.extend(compiler_asset_roots());

    let mut unique = HashSet::new();
    roots.retain(|root| unique.insert(root.clone()));
    roots
}

fn load_module_recursive(
    module_name: &str,
    search_roots: &[PathBuf],
    visited_modules: &mut HashSet<String>,
    combined_program: &mut Program,
) -> Result<(), String> {
    if visited_modules.contains(module_name) {
        return Ok(());
    }
    visited_modules.insert(module_name.to_string());

    let relative_path = module_name.replace('.', "/") + ".lmr";
    let module_path = search_roots.iter().find_map(|root| {
        let modules_dir_path = root.join("modules").join(&relative_path);
        let direct_path = root.join(&relative_path);
        if modules_dir_path.is_file() { Some(modules_dir_path) }
        else if direct_path.is_file() { Some(direct_path) }
        else { None }
    }).ok_or_else(|| format!("No se encontró el módulo '{}' en las rutas del programa, LMR_MODULE_PATH, módulos del usuario ni la distribución", module_name))?;

    println!(
        "  -> Cargando módulo: '{}' ({})",
        module_name,
        module_path.display()
    );

    let source = fs::read_to_string(&module_path)
        .map_err(|e| format!("Error al leer módulo '{}': {}", module_name, e))?;

    let mut lexer = Lexer::new(&source);
    let tokens = lexer
        .tokenize()
        .map_err(|e| format!("Error léxico en módulo '{}': {}", module_name, e))?;

    let mut parser = Parser::new(tokens);
    let mut mod_ast = parser
        .parse_program()
        .map_err(|e| format!("Error sintáctico en módulo '{}': {}", module_name, e))?;
    annotate_program_source(&mut mod_ast, &module_path.to_string_lossy());

    for import_decl in &mod_ast.imports {
        load_module_recursive(
            &import_decl.module_path,
            search_roots,
            visited_modules,
            combined_program,
        )?;
    }

    for e in mod_ast.enums {
        if !combined_program
            .enums
            .iter()
            .any(|existing| existing.name == e.name)
        {
            combined_program.enums.push(e);
        }
    }
    for r in mod_ast.records {
        if !combined_program
            .records
            .iter()
            .any(|existing| existing.name == r.name)
        {
            combined_program.records.push(r);
        }
    }
    for ext in mod_ast.extern_funcs {
        if !combined_program
            .extern_funcs
            .iter()
            .any(|existing| existing.name == ext.name)
        {
            combined_program.extern_funcs.push(ext);
        }
    }
    for class in mod_ast.classes {
        if !combined_program
            .classes
            .iter()
            .any(|c| c.name == class.name)
        {
            combined_program.classes.push(class);
        }
    }
    for func in mod_ast.functions {
        if !combined_program
            .functions
            .iter()
            .any(|f| f.name == func.name)
        {
            combined_program.functions.push(func);
        }
    }

    Ok(())
}

fn compile_and_run_source(source_code: &str, output_exe: &Path) -> Result<(), String> {
    let mut lexer = Lexer::new(source_code);
    let tokens = lexer.tokenize()?;

    let mut parser = Parser::new(tokens);
    let ast = parser.parse_program()?;

    let mut sema = SemanticAnalyzer::new();
    sema.analyze(&ast)?;

    let mut llvm_ir = generate_llvm(&ast)?;

    // --- PARCHE DE EMERGENCIA CONTRA RET VOID EN MAIN ---
    // Si aparece un "ret void" antes de la última llave de cierre del main, lo reemplazamos por "ret i32 0"
    if let Some(main_pos) = llvm_ir.find("define i32 @main()") {
        let main_code = &mut llvm_ir[main_pos..];
        if let Some(ret_pos) = main_code.rfind("ret void") {
            // Reemplazamos ese "ret void" específico dentro de main por "ret i32 0"
            let absolute_pos = main_pos + ret_pos;
            llvm_ir.replace_range(absolute_pos..absolute_pos + 8, "ret i32 0");
        }
    }

    let ll_file = output_exe.with_extension("ll");
    fs::write(&ll_file, &llvm_ir).map_err(|e| e.to_string())?;

    let mut clang_cmd = Command::new("clang");
    clang_cmd.arg(&ll_file);

    if let Some(bridge_path) = resolve_compiler_asset(Path::new("src/trade_bridge.c")) {
        clang_cmd.arg(bridge_path);
    }

    #[cfg(target_os = "windows")]
    {
        clang_cmd.arg("-llegacy_stdio_definitions");
        clang_cmd.arg("-luser32");
        clang_cmd.arg("-lgdi32");
        clang_cmd.arg("-lshell32");
    }

    clang_cmd.arg("-o").arg(output_exe);

    let status = clang_cmd.status().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("Clang falló con código {}", status));
    }

    let exe_path = Path::new(".").join(output_exe);
    let run_status = Command::new(exe_path).status().map_err(|e| e.to_string())?;
    if !run_status.success() {
        return Err("Error al ejecutar binario".to_string());
    }

    let _ = fs::remove_file(&ll_file);
    let _ = fs::remove_file(output_exe);
    Ok(())
}

// --- CONSOLA INTERACTIVA REPL ---
fn run_repl() {
    println!("================================================");
    println!("        LMR INTERACTIVE SHELL (REPL) v2.0       ");
    println!("================================================");
    println!("Escribe sentencias LMR (ej: writeln(\"Hola LMR\"))");
    println!("Escribe 'salir' o 'exit' para terminar.\n");

    let tmp_exe = env::temp_dir().join("lmr_repl_eval.exe");
    let stdin = io::stdin();

    loop {
        print!("lmr> ");
        let _ = io::stdout().flush();

        let mut line = String::new();
        if stdin.read_line(&mut line).is_err() {
            break;
        }
        let trimmed = line.trim();

        if trimmed == "salir" || trimmed == "exit" {
            println!("Cerrando REPL de LMR...");
            break;
        }

        if trimmed.is_empty() {
            continue;
        }

        let program_code = format!("begin\n    {}\nend.", trimmed);
        if let Err(e) = compile_and_run_source(&program_code, &tmp_exe) {
            eprintln!("❌ {}", e);
        }
    }
}

fn check_source_file(input_path_str: &str, read_stdin: bool) -> i32 {
    let input_path = Path::new(input_path_str);
    if !read_stdin && !input_path.exists() {
        eprintln!("❌ El archivo '{}' no existe", input_path_str);
        return 2;
    }
    let source_code = if read_stdin {
        let mut source = String::new();
        if let Err(error) = io::stdin().read_to_string(&mut source) {
            eprintln!("❌ Error al leer código desde stdin: {}", error);
            return 2;
        }
        source
    } else {
        match fs::read_to_string(input_path) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("❌ Error al leer archivo: {}", error);
                return 2;
            }
        }
    };

    let tokens = match Lexer::new(&source_code).tokenize() {
        Ok(tokens) => tokens,
        Err(error) => {
            report_source_error("léxico", input_path, &source_code, &error);
            return 1;
        }
    };
    let mut ast = match Parser::new(tokens).parse_program() {
        Ok(program) => program,
        Err(error) => {
            report_source_error("sintáctico", input_path, &source_code, &error);
            return 1;
        }
    };
    annotate_program_source(&mut ast, &input_path.to_string_lossy());

    let project_root = input_path.parent().unwrap_or_else(|| Path::new("."));
    let module_roots = module_search_roots(project_root);
    let mut visited_modules = HashSet::new();
    for import_decl in ast.imports.clone() {
        if let Err(error) = load_module_recursive(
            &import_decl.module_path,
            &module_roots,
            &mut visited_modules,
            &mut ast,
        ) {
            eprintln!("❌ Error al cargar módulo: {}", error);
            return 1;
        }
    }

    if let Err(error) = SemanticAnalyzer::new().analyze(&ast) {
        report_source_error("semántico", input_path, &source_code, &error);
        return 1;
    }
    println!("✅ '{}' es válido", input_path.display());
    0
}

fn main() {
    let exit_code = run_main();
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
}

fn run_main() -> i32 {
    let args: Vec<String> = env::args().collect();

    // Si se ejecuta sin parámetros, inicia el REPL en vivo
    if args.len() < 2 {
        run_repl();
        return 0;
    }

    let mut should_run = false;
    let mut check_only = false;
    let mut check_stdin = false;
    let mut compile_only = false;
    let mut input_file: Option<String> = None;
    let mut custom_output: Option<String> = None;
    let mut custom_target: Option<String> = None;
    let mut program_args: Vec<String> = Vec::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "run" => should_run = true,
            "check" => check_only = true,
            "--stdin" => check_stdin = true,
            "repl" => {
                run_repl();
                return 0;
            }
            "-c" => compile_only = true,
            "-o" => {
                if i + 1 < args.len() {
                    custom_output = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    eprintln!("❌ Falta nombre tras '-o'");
                    return 2;
                }
            }
            "--target" => {
                if i + 1 < args.len() {
                    custom_target = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    eprintln!("❌ Falta tripleta tras '--target'");
                    return 2;
                }
            }
            "--help" | "-h" => {
                print_help();
                return 0;
            }
            "--version" | "-V" => {
                println!("lmr {}", env!("CARGO_PKG_VERSION"));
                return 0;
            }
            _ => {
                if input_file.is_none() {
                    input_file = Some(args[i].clone());
                } else if should_run {
                    program_args.push(args[i].clone());
                }
            }
        }
        i += 1;
    }

    let input_path_str = match input_file {
        Some(f) => f,
        None => {
            eprintln!("❌ No se especificó archivo .lmr");
            return 2;
        }
    };

    if check_only {
        let exit_code = check_source_file(&input_path_str, check_stdin);
        let _ = io::stdout().flush();
        return exit_code;
    }

    let input_path = Path::new(&input_path_str);
    if !input_path.exists() {
        eprintln!("❌ El archivo '{}' no existe", input_path_str);
        return 2;
    }

    let source_code = match fs::read_to_string(input_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("❌ Error al leer archivo: {}", e);
            return 2;
        }
    };

    println!("Compilando '{}'...", input_path.display());

    let mut lexer = Lexer::new(&source_code);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => {
            report_source_error("léxico", input_path, &source_code, &e);
            return 1;
        }
    };

    let mut parser = Parser::new(tokens);
    let mut ast = match parser.parse_program() {
        Ok(tree) => tree,
        Err(e) => {
            report_source_error("sintáctico", input_path, &source_code, &e);
            return 1;
        }
    };
    annotate_program_source(&mut ast, &input_path.to_string_lossy());

    let project_root = input_path.parent().unwrap_or_else(|| Path::new("."));
    let module_roots = module_search_roots(project_root);
    let mut visited_modules = HashSet::new();

    let main_imports = ast.imports.clone();
    for import_decl in main_imports {
        if let Err(e) = load_module_recursive(
            &import_decl.module_path,
            &module_roots,
            &mut visited_modules,
            &mut ast,
        ) {
            eprintln!("❌ Error al cargar módulo: {}", e);
            return 1;
        }
    }

    let mut sema = SemanticAnalyzer::new();
    if let Err(e) = sema.analyze(&ast) {
        report_source_error("semántico", input_path, &source_code, &e);
        return 1;
    }

    let llvm_ir = match generate_llvm(&ast) {
        Ok(ir) => ir,
        Err(e) => {
            eprintln!("❌ Error interno del compilador: {}", e);
            return 1;
        }
    };

    let stem = input_path
        .file_stem()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "programa".to_string());
    let ll_file = format!("{}.ll", stem);

    let output_file: PathBuf = if let Some(out) = custom_output {
        PathBuf::from(out)
    } else if compile_only {
        PathBuf::from(format!("{}.o", stem))
    } else {
        #[cfg(target_os = "windows")]
        {
            PathBuf::from(format!("{}.exe", stem))
        }
        #[cfg(not(target_os = "windows"))]
        {
            PathBuf::from(&stem)
        }
    };

    if let Err(e) = fs::write(&ll_file, &llvm_ir) {
        eprintln!("❌ Error al escribir IR: {}", e);
        return 1;
    }

    let mut clang_cmd = Command::new("clang");
    clang_cmd.arg(&ll_file);

    if !compile_only {
        if let Some(bridge_path) = resolve_compiler_asset(Path::new("src/trade_bridge.c")) {
            clang_cmd.arg(bridge_path);
        }
    }

    #[cfg(target_os = "windows")]
    {
        clang_cmd.arg("-llegacy_stdio_definitions");
        clang_cmd.arg("-luser32"); // Ventanas de Windows
        clang_cmd.arg("-lgdi32"); // Gráficos 2D
        clang_cmd.arg("-lshell32"); // CommandLineToArgvW
    }
    #[cfg(not(target_os = "windows"))]
    {
        clang_cmd.arg("-lm");
    }

    if compile_only {
        clang_cmd.arg("-c");
    }
    clang_cmd.arg("-o").arg(&output_file);

    if let Some(ref target) = custom_target {
        clang_cmd.arg(format!("--target={}", target));
    }

    let clang_status = clang_cmd.status();
    match clang_status {
        Ok(status) if status.success() => {
            if compile_only {
                println!("✅ Archivo objeto generado -> {}", output_file.display());
            } else {
                println!("✅ Compilación exitosa -> {}", output_file.display());
            }
        }
        Ok(status) => {
            eprintln!("❌ Error en Clang ({})", status);
            return 1;
        }
        Err(e) => {
            eprintln!("❌ Error al invocar clang: {}", e);
            return 1;
        }
    }

    if should_run {
        if compile_only {
            eprintln!("⚠️ No se puede ejecutar un archivo .o");
            return 2;
        }
        println!("------------------------------------------");
        let exe_path = if output_file.is_absolute() {
            output_file.clone()
        } else {
            env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(&output_file)
        };
        let run_status = Command::new(&exe_path).args(&program_args).status();
        println!("------------------------------------------");
        match run_status {
            Ok(status) if status.success() => {}
            Ok(status) => {
                eprintln!("❌ El programa terminó con código {}", status);
                return status.code().unwrap_or(1);
            }
            Err(e) => {
                eprintln!("❌ No se pudo iniciar '{}': {}", exe_path.display(), e);
                return 1;
            }
        }
    }
    0
}

#[cfg(test)]
mod compiler_regression_tests {
    use super::{CodeGenerator, Lexer, Parser, Program, SemanticAnalyzer};
    use crate::ast::{Expr, Stmt};
    use std::fs;
    use std::process::Command;

    fn analyze_source(source: &str) -> Result<(), String> {
        let tokens = Lexer::new(source).tokenize()?;
        let program = Parser::new(tokens).parse_program()?;
        SemanticAnalyzer::new().analyze(&program)
    }

    fn compile_frontend_to_ir(source: &str) -> Result<String, String> {
        let tokens = Lexer::new(source).tokenize()?;
        let program = Parser::new(tokens).parse_program()?;
        SemanticAnalyzer::new().analyze(&program)?;
        CodeGenerator::new().generate(&program)
    }

    #[test]
    fn accepts_a_minimal_program() {
        assert!(analyze_source("begin\nend.").is_ok());
    }

    #[test]
    fn rejects_redeclaration_in_same_scope() {
        let error = analyze_source("begin\nint score\nint score\nend.")
            .expect_err("declarar dos veces en el mismo ámbito debe fallar");
        assert!(error.contains("score"), "mensaje inesperado: {error}");
    }

    #[test]
    fn rejects_assignment_to_undeclared_variable() {
        let error = analyze_source("begin\nmissing = 3\nend.")
            .expect_err("asignar a una variable no declarada debe fallar");
        assert!(error.contains("missing"), "mensaje inesperado: {error}");
    }

    #[test]
    fn reports_unterminated_string_with_source_location() {
        let error = Lexer::new("begin\n\"sin cerrar")
            .tokenize()
            .expect_err("una cadena sin cerrar debe producir error léxico");
        assert!(error.contains("línea 2"), "mensaje inesperado: {error}");
    }

    #[test]
    fn parser_errors_include_line_and_column() {
        let tokens = Lexer::new("begin\nint = 1\nend.").tokenize().unwrap();
        let error = Parser::new(tokens)
            .parse_program()
            .expect_err("una declaración sin nombre debe fallar");
        assert!(error.contains("línea 2"), "mensaje inesperado: {error}");
        assert!(error.contains("columna 5"), "columna incorrecta: {error}");
    }

    #[test]
    fn rejects_undeclared_custom_types_before_codegen() {
        let error = analyze_source("begin\nMissingType item\nend.")
            .expect_err("un tipo personalizado inexistente debe fallar");
        assert!(error.contains("MissingType"), "mensaje inesperado: {error}");
    }

    #[test]
    fn rejects_invalid_builtin_arity_before_codegen() {
        let error = analyze_source("begin\nint count\ncount = len()\nend.")
            .expect_err("len sin argumento debe fallar");
        assert!(error.contains("len"), "mensaje inesperado: {error}");
    }

    #[test]
    fn backend_returns_error_instead_of_panicking_on_invalid_ast() {
        let program = Program {
            imports: Vec::new(),
            enums: Vec::new(),
            records: Vec::new(),
            extern_funcs: Vec::new(),
            classes: Vec::new(),
            functions: Vec::new(),
            main_block: vec![Stmt::Expression(Expr::Variable("missing".to_string()))],
        };
        let error = CodeGenerator::new()
            .generate(&program)
            .expect_err("el backend debe devolver error para una variable inválida");
        assert!(error.contains("missing"), "mensaje inesperado: {error}");
    }

    #[test]
    fn rejects_mixed_or_invalid_operator_types() {
        let error = analyze_source("begin\nbool result\nresult = true + false\nend.")
            .expect_err("no se debe sumar bool");
        assert!(error.contains("'+'"), "mensaje inesperado: {error}");
        assert!(
            error.contains("línea 3") && error.contains("columna"),
            "falta ubicación: {error}"
        );
    }

    #[test]
    fn requires_all_paths_of_value_function_to_return() {
        let error = analyze_source("func answer(): int\nbegin\nend\nbegin\nend.")
            .expect_err("la función no devuelve valor");
        assert!(error.contains("answer"), "mensaje inesperado: {error}");
        assert!(
            error.contains("línea 1") && error.contains("columna 1"),
            "falta ubicación: {error}"
        );
    }

    #[test]
    fn accepts_value_returned_from_both_if_branches() {
        let source = "func answer(bool yes): int\nbegin\nif yes then begin\nreturn 1\nend\nelse begin\nreturn 2\nend\nend\nbegin\nend.";
        assert!(analyze_source(source).is_ok());
    }

    #[test]
    fn validates_override_signatures() {
        let source = "class Parent begin\nfunc value(): int begin\nreturn 1\nend\nend\nclass Child extends Parent begin\noverride func value(): float begin\nreturn 1.0\nend\nend\nbegin\nend.";
        let error = analyze_source(source).expect_err("firma incompatible debe fallar");
        assert!(error.contains("firma"), "mensaje inesperado: {error}");
    }

    #[test]
    fn rejects_cyclic_inheritance() {
        let source = "class First extends Second begin\nend\nclass Second extends First begin\nend\nbegin\nend.";
        let error = analyze_source(source).expect_err("la herencia circular debe fallar");
        assert!(error.contains("circular"), "mensaje inesperado: {error}");
    }

    #[test]
    fn points_to_the_type_of_an_invalid_record_field() {
        let source = "record Example\nbegin\n    Missing field\nend\nbegin\nend.";
        let error = analyze_source(source).expect_err("tipo de campo desconocido debe fallar");
        assert!(
            error.contains("línea 3") && error.contains("columna 5"),
            "ubicación incorrecta: {error}"
        );
    }

    #[test]
    fn points_to_a_duplicate_class_field() {
        let source = "class Box\nbegin\nint value\nfloat value\nend\nbegin\nend.";
        let error = analyze_source(source).expect_err("campo repetido debe fallar");
        assert!(
            error.contains("línea 4") && error.contains("columna 7"),
            "ubicación incorrecta: {error}"
        );
    }

    #[test]
    fn locates_invalid_external_signature() {
        let source = "extern func invoke(Missing item): int\nbegin\nend.";
        let error =
            analyze_source(source).expect_err("firma externa con tipo desconocido debe fallar");
        assert!(
            error.contains("línea 1") && error.contains("columna 1"),
            "ubicación incorrecta: {error}"
        );
    }

    #[test]
    fn preserves_module_path_in_semantic_diagnostic() {
        let source = "record Example\nbegin\nMissing field\nend\nbegin\nend.";
        let tokens = Lexer::new(source).tokenize().unwrap();
        let mut program = Parser::new(tokens).parse_program().unwrap();
        super::annotate_program_source(&mut program, "modules/Example.lmr");
        let error = SemanticAnalyzer::new()
            .analyze(&program)
            .expect_err("tipo de campo desconocido debe fallar");
        assert!(
            error.contains("archivo: modules/Example.lmr"),
            "ruta incorrecta: {error}"
        );
        assert!(
            error.contains("línea 3") && error.contains("columna 1"),
            "ubicación incorrecta: {error}"
        );
    }

    #[test]
    fn rejects_override_without_parent_method() {
        let source =
            "class Child begin\noverride func value(): int begin\nreturn 1\nend\nend\nbegin\nend.";
        let error = analyze_source(source).expect_err("override sin método padre debe fallar");
        assert!(error.contains("override"), "mensaje inesperado: {error}");
    }

    #[test]
    fn compiles_a_complete_source_through_llvm_generation() {
        let source = "func answer(): int\nbegin\nreturn 42\nend\nbegin\nint n\nn = answer()\nend.";
        let ir = compile_frontend_to_ir(source).expect("programa válido debe generar LLVM");
        assert!(ir.contains("define i64 @answer()"));
        assert!(ir.contains("define i32 @main()"));
    }

    #[test]
    fn generates_long_string_concatenations_without_recursive_codegen() {
        let pieces = vec!["\"x\""; 64].join(" + ");
        let source = format!("begin\nstring query\nquery = {}\nend.", pieces);
        let ir = compile_frontend_to_ir(&source)
            .expect("una concatenación larga debe generar LLVM sin agotar la pila");
        assert!(ir.contains("@strcat"));
    }

    #[test]
    fn compiles_and_runs_a_complete_program_with_clang() {
        let source = "func answer(): int\nbegin\nreturn 42\nend\nbegin\nstring saved\nint i\nsaved = \"start\"\ni = 0\nwhile i < 2 do begin\nstring current\ncurrent = \"value\" + intToString(i)\nsaved = current\ni = i + 1\nend\nwriteln(answer())\nwriteln(saved)\nwriteln(sin(0))\nwriteln(atan2(0, 1))\nwriteln(round(2.5))\nwriteln(fmod(5, 2))\nend.";
        let ir = compile_frontend_to_ir(source).expect("programa válido debe generar LLVM");
        let base = std::env::current_dir()
            .expect("directorio del proyecto")
            .join("target")
            .join(format!("lmr_e2e_{}", std::process::id()));
        let ir_path = base.with_extension("ll");
        let exe_path = if cfg!(target_os = "windows") {
            base.with_extension("exe")
        } else {
            base.clone()
        };
        fs::write(&ir_path, ir).expect("guardar LLVM temporal");
        let mut clang = Command::new("clang");
        clang.arg(&ir_path).arg("src/trade_bridge.c");
        if cfg!(target_os = "windows") {
            clang.args([
                "-llegacy_stdio_definitions",
                "-luser32",
                "-lgdi32",
                "-lshell32",
            ]);
        } else {
            clang.arg("-lm");
        }
        let compile = clang
            .arg("-o")
            .arg(&exe_path)
            .output()
            .expect("clang debe estar disponible para compilar LMR");
        assert!(
            compile.status.success(),
            "clang falló: {}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let run = Command::new(&exe_path)
            .output()
            .expect("ejecutable de prueba debe iniciar");
        assert!(run.status.success(), "el programa terminó con error");
        assert!(String::from_utf8_lossy(&run.stdout).contains("42"));
        assert!(String::from_utf8_lossy(&run.stdout).contains("value1"));
        assert!(String::from_utf8_lossy(&run.stdout).contains("0.00"));
        assert!(String::from_utf8_lossy(&run.stdout).contains("3.00"));
        let _ = fs::remove_file(ir_path);
        let _ = fs::remove_file(exe_path);
    }

    #[test]
    fn native_http_urls_reject_shell_injection_and_non_http_schemes() {
        let base = std::env::current_dir()
            .expect("directorio del proyecto")
            .join("target")
            .join(format!("lmr_url_guard_{}", std::process::id()));
        let exe_path = if cfg!(target_os = "windows") {
            base.with_extension("exe")
        } else {
            base.clone()
        };
        let mut clang = Command::new("clang");
        clang.args(["-DLMR_BRIDGE_URL_TEST", "src/trade_bridge.c"]);
        if cfg!(target_os = "windows") {
            clang.args([
                "-llegacy_stdio_definitions",
                "-luser32",
                "-lgdi32",
                "-lshell32",
            ]);
        } else {
            clang.args(["-lm", "-pthread"]);
        }
        let compile = clang
            .arg("-o")
            .arg(&exe_path)
            .output()
            .expect("clang debe estar disponible para probar el puente nativo");
        assert!(
            compile.status.success(),
            "clang falló: {}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let run = Command::new(&exe_path)
            .output()
            .expect("prueba de validación URL debe iniciar");
        assert!(
            run.status.success(),
            "falló la prueba C del puente URL con código {:?}: {}",
            run.status.code(),
            String::from_utf8_lossy(&run.stdout)
        );
        assert!(String::from_utf8_lossy(&run.stdout).contains("Native URL guard OK"));
        let _ = fs::remove_file(exe_path);
    }
}
