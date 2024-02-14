use arbitrary::{Arbitrary, Unstructured};
use rand::{rngs::SmallRng, RngCore, SeedableRng};
use wasm_smith::{Config, Module};
use wasmparser::{ExternalKind, FuncType, Parser, TypeRef, ValType, Validator};

mod common;
use common::{parser_features_from_config, validate};

struct TestCase {
    export_module: String,
    expected_exports: Vec<(String, FuncType)>,
}

fn test_cases() -> Vec<TestCase> {
    vec![TestCase {
        export_module: r#"
		(module
			(func (export "foo") (param i32) (result i64)
				unreachable
			)
		)
		"#
        .to_string(),
        expected_exports: vec![(
            "foo".to_string(),
            FuncType::new([ValType::I32], [ValType::I64]),
        )],
    }]
}

#[test]
fn smoke_test_exports_config() {
    let mut rng = SmallRng::seed_from_u64(11);
    let mut buf = vec![0; 512];

    for test_case in test_cases() {
        for _ in 0..1024 {
            rng.fill_bytes(&mut buf);
            let mut u = Unstructured::new(&buf);

            let mut config = Config::arbitrary(&mut u).expect("arbitrary config");
            config.exports = Some(wat::parse_str(&test_case.export_module).unwrap());

            let features = parser_features_from_config(&config);
            let module = Module::new(config, &mut u).unwrap();

            let wasm_bytes = module.to_bytes();
            let mut validator = Validator::new_with_features(features);
            validate(&mut validator, &wasm_bytes);

            let mut types = vec![];
            let mut func_imports = vec![];
            let mut funcs = vec![];
            let mut exports = vec![];

            for payload in Parser::new(0).parse_all(&wasm_bytes) {
                let payload = payload.unwrap();
                if let wasmparser::Payload::TypeSection(rdr) = payload {
                    // Gather the signature types to later check function types
                    // against.
                    for ty in rdr.into_iter_err_on_gc_types() {
                        types.push(ty.unwrap());
                    }
                } else if let wasmparser::Payload::FunctionSection(rdr) = payload {
                    funcs = rdr.into_iter().collect::<Result<_, _>>().unwrap();
                } else if let wasmparser::Payload::ImportSection(rdr) = payload {
                    func_imports = rdr
                        .into_iter()
                        .filter_map(|imp| {
                            let imp = imp.unwrap();
                            if let TypeRef::Func(i) = imp.ty {
                                Some(i)
                            } else {
                                None
                            }
                        })
                        .collect();
                } else if let wasmparser::Payload::ExportSection(rdr) = payload {
                    exports = rdr.into_iter().collect::<Result<_, _>>().unwrap();
                }
            }

            assert_eq!(test_case.expected_exports.len(), exports.len());
            for ((name, ty), export) in test_case.expected_exports.iter().zip(exports.iter()) {
                assert_eq!(name, export.name);
                assert_eq!(ExternalKind::Func, export.kind);
                let index = export.index as usize;
                let type_index = if index < func_imports.len() {
                    func_imports[index]
                } else {
                    funcs[index - func_imports.len()]
                };
                let export_ty = &types[type_index as usize];
                assert_eq!(ty, export_ty);
            }
        }
    }
}
