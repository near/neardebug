mod near_vm_runner;
mod prepare;

use wasm_bindgen::prelude::*;
use finite_wasm::wasmparser::{self, Type};
pub use near_vm_runner::{Logic, Context};

#[no_mangle]
pub fn rustsecp256k1_v0_8_1_context_preallocated_size() {
    todo!("not supported")
}

#[no_mangle]
pub fn rustsecp256k1_v0_8_1_context_preallocated_create() {
    todo!("not supported")
}

#[no_mangle]
pub fn rustsecp256k1_v0_8_1_context_preallocated_destroy() {
    todo!("not supported")
}

#[wasm_bindgen]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn list_methods(wasm_bytes: &[u8]) -> Result<Vec<String>, JsError> {
    let parser = wasmparser::Parser::new(0);
    let mut types = vec![];
    let mut fns = vec![];
    let mut callable_methods = vec![];
    for payload in parser.parse_all(wasm_bytes) {
        match payload? {
            wasmparser::Payload::ImportSection(ims) => {
                for im in ims {
                    let im = im?;
                    match im.ty {
                        wasmparser::TypeRef::Func(f) => fns.push(f),
                        _ => {}
                    }
                }

            }
            wasmparser::Payload::TypeSection(type_section) => {
                for ty in type_section {
                    let ty = ty?;
                    types.push(ty);
                }
            }
            wasmparser::Payload::FunctionSection(function_section) => {
                for (i, f) in function_section.into_iter().enumerate() {
                    let f = f?;
                    println!("{} {} {:?}", i, f, types[f as usize]);
                    fns.push(f);
                }
            }
            wasmparser::Payload::ExportSection(exports) => {
                for export in exports {
                    let ex = export?;
                    let wasmparser::ExternalKind::Func = ex.kind else {
                        continue;
                    };
                    let f = fns.get(ex.index as usize).copied();
                    let Some(Type::Func(ty)) = f.and_then(|ty| {
                        types.get(ty as usize)
                    }) else {
                        return Err(JsError::new("could not obtain function type for export"));
                    };
                    if ty.params().is_empty() && ty.results().is_empty() {
                        callable_methods.push(ex.name.to_string());
                    }
                }
                return Ok(callable_methods);
            }
            _ => {}
        }
    }
    Ok(callable_methods)
}

#[wasm_bindgen]
pub fn prepare_contract(wasm_bytes: &[u8]) -> Result<Vec<u8>, JsError> {
    prepare::prepare_contract(wasm_bytes)
}
