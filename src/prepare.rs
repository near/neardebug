use finite_wasm::prefix_sum_vec;
use finite_wasm::wasmparser as wp;
use wasm_bindgen::JsError;
use wasm_encoder::{Encode, Section, SectionId};

struct LimitConfig {
    max_functions_number_per_contract: Option<u64>,
    max_locals_per_contract: Option<u64>,
    initial_memory_pages: u32,
    max_memory_pages: u32,
}

struct Config {
    limit_config: LimitConfig,
    discard_custom_sections: bool,
    regular_op_cost: u64,
}

struct PrepareContext<'a> {
    code: &'a [u8],
    config: &'a Config,
    output_code: Vec<u8>,
    function_limit: u64,
    local_limit: u64,
    validator: wp::Validator,
    func_validator_allocations: wp::FuncValidatorAllocations,
    before_import_section: bool,
}

impl<'a> PrepareContext<'a> {
    fn new(code: &'a [u8], features: wp::WasmFeatures, config: &'a Config) -> Self {
        let limits = &config.limit_config;
        Self {
            code,
            config,
            output_code: Vec::with_capacity(code.len()),
            // Practically reaching u64::MAX locals or functions is infeasible, so when the limit is not
            // specified, use that as a limit.
            function_limit: limits.max_functions_number_per_contract.unwrap_or(u64::MAX),
            local_limit: limits.max_locals_per_contract.unwrap_or(u64::MAX),
            validator: wp::Validator::new_with_features(features.into()),
            func_validator_allocations: wp::FuncValidatorAllocations::default(),
            before_import_section: true,
        }
    }

    /// “Early” preparation.
    ///
    /// Must happen before the finite-wasm analysis and is applicable to NearVm just as much as it is
    /// applicable to other runtimes.
    ///
    /// This will validate the module, normalize the memories within, apply limits.
    fn run(&mut self) -> Result<Vec<u8>, JsError> {
        self.before_import_section = true;
        let parser = wp::Parser::new(0);
        for payload in parser.parse_all(self.code) {
            let payload = payload
                .map_err(|err| JsError::new(&format!("could not parse webassembly: {err}")))?;
            match payload {
                wp::Payload::Version {
                    num,
                    encoding,
                    range,
                } => {
                    self.copy(range.clone())?;
                    self.validator.version(num, encoding, &range).map_err(|e| {
                        JsError::new(&format!("could not validate webassembly: {e}"))
                    })?;
                }
                wp::Payload::End(offset) => {
                    self.validator.end(offset).map_err(|e| {
                        JsError::new(&format!("could not validate webassembly: {e}"))
                    })?;
                }

                wp::Payload::TypeSection(reader) => {
                    self.validator.type_section(&reader).map_err(|e| {
                        JsError::new(&format!("could not validate type section: {e}"))
                    })?;
                    self.copy_section(SectionId::Type, reader.range())?;
                }

                wp::Payload::ImportSection(reader) => {
                    self.before_import_section = false;
                    self.validator.import_section(&reader).map_err(|e| {
                        JsError::new(&format!("could not validate import section: {e}"))
                    })?;
                    self.transform_import_section(&reader)?;
                }

                wp::Payload::FunctionSection(reader) => {
                    self.ensure_import_section();
                    self.validator.function_section(&reader).map_err(|e| {
                        JsError::new(&format!("could not validate function section: {e}"))
                    })?;
                    self.copy_section(SectionId::Function, reader.range())?;
                }
                wp::Payload::TableSection(reader) => {
                    self.ensure_import_section();
                    self.validator.table_section(&reader).map_err(|e| {
                        JsError::new(&format!("could not validate table section: {e}"))
                    })?;
                    self.copy_section(SectionId::Table, reader.range())?;
                }
                wp::Payload::MemorySection(reader) => {
                    // We do not want to include the implicit memory anymore as we normalized it by
                    // importing the memory instead.
                    self.ensure_import_section();
                    self.validator.memory_section(&reader).map_err(|e| {
                        JsError::new(&format!("could not validate memory section: {e}"))
                    })?;
                }
                wp::Payload::GlobalSection(reader) => {
                    self.ensure_import_section();
                    self.validator.global_section(&reader).map_err(|e| {
                        JsError::new(&format!("could not validate globals section: {e}"))
                    })?;
                    self.copy_section(SectionId::Global, reader.range())?;
                }
                wp::Payload::ExportSection(reader) => {
                    self.ensure_import_section();
                    self.validator.export_section(&reader).map_err(|e| {
                        JsError::new(&format!("could not validate exports section: {e}"))
                    })?;
                    self.copy_section(SectionId::Export, reader.range())?;
                }
                wp::Payload::StartSection { func, range } => {
                    self.ensure_import_section();
                    self.validator.start_section(func, &range).map_err(|e| {
                        JsError::new(&format!("could not validate start section: {e}"))
                    })?;
                    self.copy_section(SectionId::Start, range.clone())?;
                }
                wp::Payload::ElementSection(reader) => {
                    self.ensure_import_section();
                    self.validator.element_section(&reader).map_err(|e| {
                        JsError::new(&format!("could not validate elements section: {e}"))
                    })?;
                    self.copy_section(SectionId::Element, reader.range())?;
                }
                wp::Payload::DataCountSection { count, range } => {
                    self.ensure_import_section();
                    self.validator
                        .data_count_section(count, &range)
                        .map_err(|e| {
                            JsError::new(&format!("could not validate data count section: {e}"))
                        })?;
                    self.copy_section(SectionId::DataCount, range.clone())?;
                }
                wp::Payload::DataSection(reader) => {
                    self.ensure_import_section();
                    self.validator.data_section(&reader).map_err(|e| {
                        JsError::new(&format!("could not validate data section: {e}"))
                    })?;
                    self.copy_section(SectionId::Data, reader.range())?;
                }
                wp::Payload::CodeSectionStart {
                    size: _,
                    count,
                    range,
                } => {
                    self.ensure_import_section();
                    self.function_limit = self
                        .function_limit
                        .checked_sub(u64::from(count))
                        .ok_or_else(|| {
                            JsError::new(&format!("module contains too many functions!"))
                        })?;
                    self.validator
                        .code_section_start(count, &range)
                        .map_err(|e| {
                            JsError::new(&format!("could not validate code section start: {e}"))
                        })?;
                    self.copy_section(SectionId::Code, range.clone())?;
                }
                wp::Payload::CodeSectionEntry(func) => {
                    let local_reader = func.get_locals_reader().map_err(|e| {
                        JsError::new(&format!("could not parse parse locals for function {e}"))
                    })?;
                    for local in local_reader {
                        let (count, _ty) = local.map_err(|e| {
                            JsError::new(&format!("could not parse locals for function: {e}"))
                        })?;
                        self.local_limit = self
                            .local_limit
                            .checked_sub(u64::from(count))
                            .ok_or_else(|| {
                                JsError::new(&format!(
                                    "a function contains too many ({count}) locals"
                                ))
                            })?;
                    }

                    let func_validator = self.validator.code_section_entry(&func).map_err(|e| {
                        JsError::new(&format!("could not validate code section entry: {e}"))
                    })?;
                    // PANIC-SAFETY: no big deal if we panic here while the allocations are taken.
                    // Worst-case we are going to be making new allocations again, but in practice
                    // this should never happen as this context should not be reused.
                    let allocs = std::mem::replace(
                        &mut self.func_validator_allocations,
                        wp::FuncValidatorAllocations::default(),
                    );
                    let mut func_validator = func_validator.into_validator(allocs);
                    func_validator
                        .validate(&func)
                        .map_err(|e| JsError::new(&format!("could not validate function: {e}")))?;
                    self.func_validator_allocations = func_validator.into_allocations();
                }
                wp::Payload::CustomSection(reader) => {
                    if !self.config.discard_custom_sections {
                        self.ensure_import_section();
                        self.copy_section(SectionId::Custom, reader.range())?;
                    }
                }

                // Extensions not supported.
                wp::Payload::UnknownSection { .. }
                | wp::Payload::TagSection(_)
                | wp::Payload::ModuleSection { .. }
                | wp::Payload::InstanceSection(_)
                | wp::Payload::CoreTypeSection(_)
                | wp::Payload::ComponentSection { .. }
                | wp::Payload::ComponentInstanceSection(_)
                | wp::Payload::ComponentAliasSection(_)
                | wp::Payload::ComponentTypeSection(_)
                | wp::Payload::ComponentCanonicalSection(_)
                | wp::Payload::ComponentStartSection { .. }
                | wp::Payload::ComponentImportSection(_)
                | wp::Payload::ComponentExportSection(_) => {
                    return Err(JsError::new("unsupported section encountered in wasm"));
                }
            }
        }
        Ok(std::mem::replace(&mut self.output_code, Vec::new()))
    }

    fn transform_import_section(
        &mut self,
        reader: &wp::ImportSectionReader,
    ) -> Result<(), JsError> {
        let mut new_section = wasm_encoder::ImportSection::new();
        for import in reader.clone() {
            let import =
                import.map_err(|e| JsError::new(&format!("could not parse an import: {e}")))?;
            if import.module != "env" {
                return Err(JsError::new("an import specifies module other than `env`"));
            }
            let new_type = match import.ty {
                wp::TypeRef::Func(id) => {
                    // TODO: validate imported function types here.
                    self.function_limit = self
                        .function_limit
                        .checked_sub(1)
                        .ok_or_else(|| JsError::new("too many functions in the module"))?;
                    wasm_encoder::EntityType::Function(id)
                }
                wp::TypeRef::Table(_) => return Err(JsError::new("tables cannot be imported")),
                wp::TypeRef::Global(_) => return Err(JsError::new("globals cannot be imported")),
                wp::TypeRef::Memory(_) => return Err(JsError::new("memories cannot be imported")),
                wp::TypeRef::Tag(_) => return Err(JsError::new("tags cannot be imported")),
            };
            new_section.import(import.module, import.name, new_type);
        }
        new_section.import("env", "memory", self.memory_import());
        // wasm_encoder a section with all imports and the imported standardized memory.
        new_section.append_to(&mut self.output_code);
        Ok(())
    }

    fn ensure_import_section(&mut self) {
        if self.before_import_section {
            self.before_import_section = false;
            let mut new_section = wasm_encoder::ImportSection::new();
            new_section.import("env", "memory", self.memory_import());
            // wasm_encoder a section with all imports and the imported standardized memory.
            new_section.append_to(&mut self.output_code);
        }
    }

    fn memory_import(&self) -> wasm_encoder::EntityType {
        wasm_encoder::EntityType::Memory(wasm_encoder::MemoryType {
            minimum: u64::from(self.config.limit_config.initial_memory_pages),
            maximum: Some(u64::from(self.config.limit_config.max_memory_pages)),
            memory64: false,
            shared: false,
            page_size_log2: None,
        })
    }

    fn copy_section(
        &mut self,
        id: SectionId,
        range: std::ops::Range<usize>,
    ) -> Result<(), JsError> {
        id.encode(&mut self.output_code);
        range.len().encode(&mut self.output_code);
        self.copy(range)
    }

    /// Copy over the payload to the output binary without significant processing.
    fn copy(&mut self, range: std::ops::Range<usize>) -> Result<(), JsError> {
        Ok(self.output_code.extend(self.code.get(range).ok_or_else(|| {
            JsError::new("could not copy data from input wasm module to the output was module")
        })?))
    }
}

pub(crate) fn prepare_contract(original_code: &[u8]) -> Result<Vec<u8>, JsError> {
    let features = wp::WasmFeatures {
        floats: true,
        mutable_global: true,
        sign_extension: true,

        saturating_float_to_int: false,
        reference_types: false,
        multi_value: false,
        bulk_memory: true,
        simd: false,
        relaxed_simd: false,
        threads: false,
        tail_call: false,
        multi_memory: false,
        exceptions: false,
        memory64: false,
        extended_const: false,
        component_model: false,
        function_references: false,
        memory_control: false,
        gc: false,
    };
    let config = Config {
        limit_config: LimitConfig {
            max_functions_number_per_contract: Some(10_000),
            max_locals_per_contract: Some(1_000_000),
            initial_memory_pages: 1_024,
            max_memory_pages: 2_048,
        },
        discard_custom_sections: false,
        regular_op_cost: 3_856_371,
    };

    let lightly_steamed = PrepareContext::new(original_code, features, &config).run()?;
    let res = finite_wasm::Analysis::new()
        .with_stack(Box::new(SimpleMaxStackCfg))
        .with_gas(Box::new(SimpleGasCostCfg(u64::from(
            config.regular_op_cost,
        ))))
        .analyze(&lightly_steamed)
        .map_err(|err| {
            JsError::new(&format!(
                "could not finite-wasm analyze the contract: {err}"
            ))
        })?
        // Make sure contracts can’t call the instrumentation functions via `env`.
        .instrument("internal", &lightly_steamed)
        .map_err(|err| {
            JsError::new(&format!(
                "could not finite-wasm instrument the contract: {err}"
            ))
        })?;
    Ok(res)
}

// TODO: refactor to avoid copy-paste with the ones currently defined in near_vm_runner
struct SimpleMaxStackCfg;

impl finite_wasm::max_stack::SizeConfig for SimpleMaxStackCfg {
    fn size_of_value(&self, ty: wp::ValType) -> u8 {
        use wp::ValType;
        match ty {
            ValType::I32 => 4,
            ValType::I64 => 8,
            ValType::F32 => 4,
            ValType::F64 => 8,
            ValType::V128 => 16,
            ValType::Ref(_) => 8,
        }
    }
    fn size_of_function_activation(
        &self,
        locals: &prefix_sum_vec::PrefixSumVec<wp::ValType, u32>,
    ) -> u64 {
        let mut res = 64_u64; // Rough accounting for rip, rbp and some registers spilled. Not exact.
        let mut last_idx_plus_one = 0_u64;
        for (idx, local) in locals {
            let idx = u64::from(*idx);
            res = res.saturating_add(
                idx.checked_sub(last_idx_plus_one)
                    .expect("prefix-sum-vec indices went backwards")
                    .saturating_add(1)
                    .saturating_mul(u64::from(self.size_of_value(*local))),
            );
            last_idx_plus_one = idx.saturating_add(1);
        }
        res
    }
}

struct SimpleGasCostCfg(u64);

macro_rules! gas_cost {
    ($( @$proposal:ident $op:ident $({ $($arg:ident: $argty:ty),* })? => $visit:ident)*) => {
        $(
            fn $visit(&mut self $($(, $arg: $argty)*)?) -> u64 {
                gas_cost!(@@$proposal $op self $({ $($arg: $argty),* })? => $visit)
            }
        )*
    };

    (@@mvp $_op:ident $_self:ident $({ $($_arg:ident: $_argty:ty),* })? => visit_block) => {
        0
    };
    (@@mvp $_op:ident $_self:ident $({ $($_arg:ident: $_argty:ty),* })? => visit_end) => {
        0
    };
    (@@mvp $_op:ident $_self:ident $({ $($_arg:ident: $_argty:ty),* })? => visit_else) => {
        0
    };
    (@@$_proposal:ident $_op:ident $self:ident $({ $($arg:ident: $argty:ty),* })? => $visit:ident) => {
        $self.0
    };
}

impl<'a> wp::VisitOperator<'a> for SimpleGasCostCfg {
    type Output = u64;
    wp::for_each_operator!(gas_cost);
}
