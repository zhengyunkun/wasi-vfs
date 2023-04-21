use heck::*;
use std::{path::Path, str::FromStr};
use witx::*;

#[derive(Debug, Clone, Copy)]
pub enum AbiVariant {
    Legacy,
    Latest,
}

impl FromStr for AbiVariant {
    type Err = String;
    fn from_str(day: &str) -> Result<Self, Self::Err> {
        match day {
            "legacy" => Ok(Self::Legacy),
            "latest" => Ok(Self::Latest),
            other => Err(format!("unsupported abi variant {}", other)),
        }
    }
}

pub fn generate<P: AsRef<Path>>(witx_paths: &[P], variant: AbiVariant) -> String {
    let doc = witx::load(witx_paths).unwrap();

    let mut raw = String::new();
    raw.push_str(
        "\
// This file is automatically generated, DO NOT EDIT
//
// To regenerate this file run the `crates/wasi-libc-trampoline-bindgen` command
// This file is written in C to add `__attribute__((weak))` to the functions so
// that they won't be linked if the user doesn't use those WASI functions.

#include <stdint.h>

",
    );
    for m in doc.modules() {
        render_module(&m, variant, &mut raw);
        raw.push('\n');
    }

    raw
}

trait RenderC {
    fn render_c(&self, src: &mut String);
}

impl RenderC for IntRepr {
    fn render_c(&self, src: &mut String) {
        match self {
            IntRepr::U8 => src.push_str("uint8_t"),
            IntRepr::U16 => src.push_str("uint16_t"),
            IntRepr::U32 => src.push_str("uint32_t"),
            IntRepr::U64 => src.push_str("uint64_t"),
        }
    }
}

impl RenderC for WasmType {
    fn render_c(&self, src: &mut String) {
        match self {
            WasmType::I32 => src.push_str("int32_t"),
            WasmType::I64 => src.push_str("int64_t"),
            WasmType::F32 => src.push_str("float"),
            WasmType::F64 => src.push_str("double"),
        }
    }
}

fn render_module(module: &Module, variant: AbiVariant, src: &mut String) {
    for f in module.funcs() {
        if !crate::WASI_HOOK_FUNCTIONS.contains(&f.name.as_str()) {
            continue;
        }
        let f_name = f.name.as_str();

        let abi_name = match variant {
            AbiVariant::Latest => {
                let mut name = String::new();
                name.push_str("__imported_");
                name.push_str(&module.name.as_str().to_snake_case());
                name.push('_');
                name.push_str(&f_name.to_snake_case());
                name
            }
            AbiVariant::Legacy => {
                let mut name = String::new();
                name.push_str("__wasi_");
                name.push_str(&f_name.to_snake_case());
                name
            }
        };

        render_libc_hook_point(
            &*f,
            &abi_name,
            &format!(
                "wasi_vfs_{}_{}",
                module.name.as_str().to_snake_case(),
                f_name.to_snake_case()
            ),
            src,
        );
        src.push('\n');
    }
}

fn render_libc_hook_point(
    func: &InterfaceFunc,
    name: &str,
    trampoline_name: &str,
    src: &mut String,
) {
    let (params, results) = func.wasm_signature();
    assert!(results.len() <= 1);
    src.push_str("__attribute__((weak))\n");
    results[0].render_c(src);
    let params_str = params
        .iter()
        .enumerate()
        .map(|(i, param_ty)| {
            let mut param = String::new();
            param_ty.render_c(&mut param);
            param.push(' ');
            param.push_str("arg");
            param.push_str(&i.to_string());
            param
        })
        .collect::<Vec<_>>()
        .join(", ");
    src.push(' ');
    src.push_str(name);
    src.push('(');
    src.push_str(&params_str);
    src.push(')');
    src.push_str(" {\n");

    src.push_str("  extern ");
    results[0].render_c(src);
    src.push(' ');
    src.push_str(trampoline_name);
    src.push('(');
    src.push_str(&params_str);
    src.push(')');
    src.push_str(";\n");

    src.push_str("  return ");
    src.push_str(trampoline_name);
    src.push('(');
    src.push_str(
        &params
            .iter()
            .enumerate()
            .map(|(i, _)| format!("arg{}", i))
            .collect::<Vec<_>>()
            .join(", "),
    );
    src.push(')');
    src.push_str(";\n");

    src.push_str("}\n");
}
