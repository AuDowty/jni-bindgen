use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

#[derive(Parser)]
#[command(
    name = "jni-bindgen",
    version,
    about = "Generate Rust JNI wrappers from Java .class files"
)]
struct Cli {
    class_file: PathBuf,
    #[arg(short, long)]
    output: Option<PathBuf>,
    #[arg(long)]
    name: Option<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let bytes = fs::read(&cli.class_file)
        .map_err(|e| format!("read {}: {e}", cli.class_file.display()))?;
    let class = cafebabe::parse_class(&bytes)
        .map_err(|e| format!("parse class: {e}"))?;
    let code = generate(&class, cli.name.as_deref())?;
    match cli.output {
        Some(p) => fs::write(&p, &code)
            .map_err(|e| format!("write {}: {e}", p.display()))?,
        None => {
            let stdout = std::io::stdout();
            let mut h = stdout.lock();
            h.write_all(code.as_bytes()).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

enum JniType {
    Void,
    Bool,
    Byte,
    Char,
    Short,
    Int,
    Long,
    Float,
    Double,
    Object(String),
    Array(Box<JniType>),
}

impl JniType {
    fn descriptor(&self) -> String {
        match self {
            JniType::Void => "V".into(),
            JniType::Bool => "Z".into(),
            JniType::Byte => "B".into(),
            JniType::Char => "C".into(),
            JniType::Short => "S".into(),
            JniType::Int => "I".into(),
            JniType::Long => "J".into(),
            JniType::Float => "F".into(),
            JniType::Double => "D".into(),
            JniType::Object(c) => format!("L{c};"),
            JniType::Array(inner) => format!("[{}", inner.descriptor()),
        }
    }
    fn is_string(&self) -> bool {
        matches!(self, JniType::Object(c) if c == "java/lang/String")
    }
    fn rust_param(&self) -> String {
        match self {
            JniType::Void => "()".into(),
            JniType::Bool => "bool".into(),
            JniType::Byte => "i8".into(),
            JniType::Char => "u16".into(),
            JniType::Short => "i16".into(),
            JniType::Int => "i32".into(),
            JniType::Long => "i64".into(),
            JniType::Float => "f32".into(),
            JniType::Double => "f64".into(),
            JniType::Object(_) if self.is_string() => "&str".into(),
            JniType::Object(_) | JniType::Array(_) => "&jni::objects::JObject<'local>".into(),
        }
    }
    fn rust_return(&self) -> String {
        match self {
            JniType::Void => "()".into(),
            JniType::Bool => "bool".into(),
            JniType::Byte => "i8".into(),
            JniType::Char => "u16".into(),
            JniType::Short => "i16".into(),
            JniType::Int => "i32".into(),
            JniType::Long => "i64".into(),
            JniType::Float => "f32".into(),
            JniType::Double => "f64".into(),
            JniType::Object(_) if self.is_string() => "String".into(),
            JniType::Object(_) | JniType::Array(_) => "jni::objects::JObject<'local>".into(),
        }
    }
    fn short_tag(&self) -> String {
        match self {
            JniType::Void => "v".into(),
            JniType::Bool => "z".into(),
            JniType::Byte => "b".into(),
            JniType::Char => "c".into(),
            JniType::Short => "s".into(),
            JniType::Int => "i".into(),
            JniType::Long => "j".into(),
            JniType::Float => "f".into(),
            JniType::Double => "d".into(),
            JniType::Object(c) if self.is_string() => "string".into(),
            JniType::Object(c) => simple_name(c).to_ascii_lowercase(),
            JniType::Array(inner) => format!("a{}", inner.short_tag()),
        }
    }
}

fn parse_descriptor(d: &str) -> Result<(Vec<JniType>, JniType), String> {
    let mut chars = d.chars().peekable();
    if chars.next() != Some('(') {
        return Err(format!("descriptor missing '(': {d}"));
    }
    let mut params = Vec::new();
    while let Some(&c) = chars.peek() {
        if c == ')' {
            chars.next();
            break;
        }
        params.push(parse_one(&mut chars)?);
    }
    let ret = parse_one(&mut chars)?;
    Ok((params, ret))
}

fn parse_one(
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> Result<JniType, String> {
    let c = chars.next().ok_or_else(|| "unexpected end of descriptor".to_string())?;
    match c {
        'V' => Ok(JniType::Void),
        'Z' => Ok(JniType::Bool),
        'B' => Ok(JniType::Byte),
        'C' => Ok(JniType::Char),
        'S' => Ok(JniType::Short),
        'I' => Ok(JniType::Int),
        'J' => Ok(JniType::Long),
        'F' => Ok(JniType::Float),
        'D' => Ok(JniType::Double),
        'L' => {
            let mut s = String::new();
            for ch in chars.by_ref() {
                if ch == ';' {
                    return Ok(JniType::Object(s));
                }
                s.push(ch);
            }
            Err("unterminated object descriptor".into())
        }
        '[' => Ok(JniType::Array(Box::new(parse_one(chars)?))),
        other => Err(format!("unknown type code '{other}'")),
    }
}

fn simple_name(qualified: &str) -> &str {
    qualified.rsplit('/').next().unwrap_or(qualified)
}

fn snake(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    let mut prev_lower = false;
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            if prev_lower {
                out.push('_');
            }
            for lc in c.to_lowercase() {
                out.push(lc);
            }
            prev_lower = false;
        } else if c == '_' || c == '$' {
            out.push('_');
            prev_lower = false;
        } else {
            out.push(c);
            prev_lower = c.is_ascii_lowercase() || c.is_ascii_digit();
        }
    }
    if RUST_KEYWORDS.contains(&out.as_str()) {
        out.push('_');
    }
    out
}

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else",
    "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop",
    "match", "mod", "move", "mut", "pub", "ref", "return", "self", "static",
    "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while",
    "abstract", "become", "box", "do", "final", "macro", "override", "priv",
    "typeof", "unsized", "virtual", "yield", "try",
];

fn generate(
    class: &cafebabe::ClassFile,
    name_override: Option<&str>,
) -> Result<String, String> {
    let class_internal = class.this_class.as_ref();
    let struct_name = name_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| simple_name(class_internal).to_string());

    let mut methods: Vec<(String, Vec<JniType>, JniType, bool, bool)> = Vec::new();
    for m in &class.methods {
        let name = m.name.to_string();
        if name.starts_with('<') && name != "<init>" {
            continue;
        }
        let is_static = m
            .access_flags
            .contains(cafebabe::MethodAccessFlags::STATIC);
        let is_ctor = name == "<init>";
        let descriptor = m.descriptor.to_string();
        let (params, ret) = parse_descriptor(&descriptor)?;
        methods.push((name, params, ret, is_static, is_ctor));
    }

    let mut name_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for (name, _, _, _, is_ctor) in &methods {
        let base = if *is_ctor { "new".to_string() } else { snake(name) };
        *name_counts.entry(base).or_insert(0) += 1;
    }

    let mut out = String::new();
    out.push_str("use jni::JNIEnv;\n");
    out.push_str("use jni::objects::{JObject, JValue, JString};\n");
    out.push_str("use jni::errors::Result as JniResult;\n\n");
    out.push_str(&format!(
        "pub const CLASS_NAME: &str = \"{class_internal}\";\n\n"
    ));
    out.push_str(&format!("pub struct {struct_name}<'local> {{\n"));
    out.push_str("    pub obj: JObject<'local>,\n");
    out.push_str("}\n\n");
    out.push_str(&format!("impl<'local> {struct_name}<'local> {{\n"));

    let mut emitted_names: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    for (jname, params, ret, is_static, is_ctor) in &methods {
        let base_name = if *is_ctor { "new".to_string() } else { snake(jname) };
        let needs_suffix = *name_counts.get(&base_name).unwrap_or(&0) > 1;
        let rust_name = if needs_suffix {
            let suffix: String = params
                .iter()
                .map(|p| p.short_tag())
                .collect::<Vec<_>>()
                .join("_");
            let counter = emitted_names.entry(base_name.clone()).or_insert(0);
            *counter += 1;
            if suffix.is_empty() {
                format!("{base_name}_{}", counter)
            } else {
                format!("{base_name}_{suffix}")
            }
        } else {
            base_name
        };

        let descriptor = build_descriptor(params, ret);
        out.push_str(&render_method(
            jname,
            &rust_name,
            params,
            ret,
            *is_static,
            *is_ctor,
            &descriptor,
            class_internal,
            &struct_name,
        ));
        out.push('\n');
    }
    out.push_str("}\n");
    Ok(out)
}

fn build_descriptor(params: &[JniType], ret: &JniType) -> String {
    let mut s = String::from("(");
    for p in params {
        s.push_str(&p.descriptor());
    }
    s.push(')');
    s.push_str(&ret.descriptor());
    s
}

fn render_method(
    jname: &str,
    rust_name: &str,
    params: &[JniType],
    ret: &JniType,
    is_static: bool,
    is_ctor: bool,
    descriptor: &str,
    class_internal: &str,
    struct_name: &str,
) -> String {
    let mut s = String::new();
    let param_names: Vec<String> = (0..params.len()).map(|i| format!("a{i}")).collect();
    let param_decls: String = params
        .iter()
        .enumerate()
        .map(|(i, t)| format!(", {}: {}", param_names[i], t.rust_param()))
        .collect();

    let env_decl = if is_static || is_ctor {
        "env: &mut JNIEnv<'local>".to_string()
    } else {
        "&mut self, env: &mut JNIEnv<'local>".to_string()
    };

    let ret_ty = if is_ctor {
        format!("Self")
    } else {
        ret.rust_return()
    };

    s.push_str(&format!(
        "    pub fn {rust_name}({env_decl}{param_decls}) -> JniResult<{ret_ty}> {{\n"
    ));

    let mut local_setup = String::new();
    let mut arg_exprs: Vec<String> = Vec::new();
    for (i, t) in params.iter().enumerate() {
        let n = &param_names[i];
        match t {
            JniType::Bool
            | JniType::Byte
            | JniType::Short
            | JniType::Int
            | JniType::Long
            | JniType::Float
            | JniType::Double
            | JniType::Char => {
                arg_exprs.push(format!("JValue::from({n})"));
            }
            JniType::Object(_) if t.is_string() => {
                local_setup.push_str(&format!("        let {n}_s = env.new_string({n})?;\n"));
                arg_exprs.push(format!("JValue::Object(&{n}_s.into())"));
            }
            JniType::Object(_) | JniType::Array(_) => {
                arg_exprs.push(format!("JValue::Object({n})"));
            }
            JniType::Void => unreachable!(),
        }
    }
    s.push_str(&local_setup);
    let args_array = if arg_exprs.is_empty() {
        "&[]".to_string()
    } else {
        format!("&[{}]", arg_exprs.join(", "))
    };

    if is_ctor {
        s.push_str(&format!(
            "        let obj = env.new_object(\"{class_internal}\", \"{descriptor}\", {args_array})?;\n"
        ));
        s.push_str(&format!("        Ok({struct_name} {{ obj }})\n"));
    } else if is_static {
        s.push_str(&format!(
            "        let v = env.call_static_method(\"{class_internal}\", \"{jname}\", \"{descriptor}\", {args_array})?;\n"
        ));
        s.push_str(&return_conversion(ret));
    } else {
        s.push_str(&format!(
            "        let v = env.call_method(&self.obj, \"{jname}\", \"{descriptor}\", {args_array})?;\n"
        ));
        s.push_str(&return_conversion(ret));
    }
    s.push_str("    }\n");
    s
}

fn return_conversion(ret: &JniType) -> String {
    match ret {
        JniType::Void => "        let _ = v;\n        Ok(())\n".into(),
        JniType::Bool => "        Ok(v.z()?)\n".into(),
        JniType::Byte => "        Ok(v.b()?)\n".into(),
        JniType::Char => "        Ok(v.c()?)\n".into(),
        JniType::Short => "        Ok(v.s()?)\n".into(),
        JniType::Int => "        Ok(v.i()?)\n".into(),
        JniType::Long => "        Ok(v.j()?)\n".into(),
        JniType::Float => "        Ok(v.f()?)\n".into(),
        JniType::Double => "        Ok(v.d()?)\n".into(),
        JniType::Object(_) if ret.is_string() => {
            "        let obj = v.l()?;\n        let js: JString = obj.into();\n        let raw = env.get_string(&js)?;\n        Ok(raw.to_string_lossy().into_owned())\n".into()
        }
        JniType::Object(_) | JniType::Array(_) => "        Ok(v.l()?)\n".into(),
    }
}
