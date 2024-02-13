use arbitrary::Unstructured;
use rand::{rngs::SmallRng, RngCore, SeedableRng};
use wasm_smith::{Config, Module};
use wasmparser::{Parser, Validator};

#[test]
fn smoke_test_exports_config() {
    let mut rng = SmallRng::seed_from_u64(11);
    let mut buf = vec![0; 512];

    rng.fill_bytes(&mut buf);
    let mut u = Unstructured::new(&buf);

    let mut config = Config::default();
    config.exports = Some(
        wat::parse_str(
            r#"
		(module
			(func (export "foo") (param i32) (result i64)
				unreachable
			)
		)
		"#,
        )
        .unwrap(),
    );

    let module = Module::new(config, &mut u).unwrap();

    let wasm_bytes = module.to_bytes();
    let mut validator = Validator::default();
    validate(&mut validator, &wasm_bytes);

    let mut types = vec![];
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
        } else if let wasmparser::Payload::ExportSection(rdr) = payload {
            exports = rdr.into_iter().collect::<Result<_, _>>().unwrap();
        }
    }

    assert_eq!(1, exports.len());
}

fn validate(validator: &mut Validator, bytes: &[u8]) {
    let err = match validator.validate_all(bytes) {
        Ok(_) => return,
        Err(e) => e,
    };
    eprintln!("Writing Wasm to `test.wasm`");
    drop(std::fs::write("test.wasm", &bytes));
    if let Ok(text) = wasmprinter::print_bytes(bytes) {
        eprintln!("Writing WAT to `test.wat`");
        drop(std::fs::write("test.wat", &text));
    }
    panic!("wasm failed to validate: {}", err);
}
