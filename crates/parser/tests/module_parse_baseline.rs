#![forbid(unsafe_code)]

use ast::{ModuleExport, ModuleImportBinding, Stmt};
use parser::parse_module;

#[test]
fn module_parse_baseline() {
    let source = "\
import value from './dep.js';\n\
import { inc as plus } from './math.js';\n\
const local = plus + 1;\n\
export { local };\n\
export default value;\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 2);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(
        parsed.imports[0].bindings,
        vec![ModuleImportBinding {
            imported: "default".to_string(),
            local: "value".to_string(),
        }]
    );
    assert_eq!(parsed.imports[1].specifier, "./math.js");
    assert_eq!(
        parsed.imports[1].bindings,
        vec![ModuleImportBinding {
            imported: "inc".to_string(),
            local: "plus".to_string(),
        }]
    );

    assert!(parsed.exports.contains(&ModuleExport {
        exported: "local".to_string(),
        local: "local".to_string(),
    }));
    assert!(
        parsed
            .exports
            .iter()
            .any(|entry| entry.exported == "default")
    );
    assert!(
        matches!(parsed.body.statements.last(), Some(Stmt::Expression(_))),
        "module parse should append synthetic export snapshot expression",
    );
}

#[test]
fn module_parse_named_reexport_baseline() {
    let source = "export { value as answer, default as fallback } from './dep.js';\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 2);
    assert_eq!(parsed.imports[0].bindings[0].imported, "value");
    assert_eq!(parsed.imports[0].bindings[1].imported, "default");
    assert!(
        parsed
            .imports
            .iter()
            .flat_map(|entry| entry.bindings.iter())
            .all(|binding| binding.local.starts_with("$__qjs_module_reexport_")),
        "re-export should synthesize hidden locals"
    );
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "$__qjs_module_reexport_0__$".to_string(),
    }));
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "fallback".to_string(),
        local: "$__qjs_module_reexport_1__$".to_string(),
    }));
}

#[test]
fn module_parse_export_star_baseline() {
    let source = "export * from './dep.js';\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "*");
    assert!(
        parsed.imports[0].bindings[0]
            .local
            .starts_with("$__qjs_module_export_star_"),
        "export * should synthesize hidden namespace capture binding",
    );
    assert!(parsed.exports.is_empty());
}

#[test]
fn module_parse_export_star_namespace_baseline() {
    let source = "export * as ns from './dep.js';\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "*");
    assert!(
        parsed.imports[0].bindings[0]
            .local
            .starts_with("$__qjs_module_export_star_namespace_"),
        "export * as ns should synthesize hidden namespace capture binding",
    );
    assert_eq!(parsed.exports.len(), 1);
    assert_eq!(parsed.exports[0].exported, "ns");
    assert_eq!(parsed.exports[0].local, parsed.imports[0].bindings[0].local);
}

#[test]
fn module_parse_empty_named_import_keeps_runtime_dependency() {
    let source = "import {} from './dep.js';\nexport const answer = 42;\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert!(
        parsed.imports[0].bindings.is_empty(),
        "empty named import should keep dependency edge without local bindings",
    );
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "answer".to_string(),
    }));
}

#[test]
fn module_parse_import_with_extra_from_spacing_baseline() {
    let source = "import { value }   from   './dep.js';\nexport const answer = value;\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "value");
    assert_eq!(parsed.imports[0].bindings[0].local, "value");
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "answer".to_string(),
    }));
}

#[test]
fn module_parse_semicolonless_import_export_baseline() {
    let source = "import { value } from './dep.js'\nexport const answer = value\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "value");
    assert_eq!(parsed.imports[0].bindings[0].local, "value");
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "answer".to_string(),
    }));
}

#[test]
fn module_parse_compact_keyword_spacing_baseline() {
    let source = "import{ value }from'./dep.js'\nconst answer = value\nexport{answer}\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "value");
    assert_eq!(parsed.imports[0].bindings[0].local, "value");
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "answer".to_string(),
    }));
}

#[test]
fn module_parse_trailing_line_comment_baseline() {
    let source = "import { value } from './from-token-dep.js' // from trailing comment\nexport const answer = value // still semicolonless\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./from-token-dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "value");
    assert_eq!(parsed.imports[0].bindings[0].local, "value");
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "answer".to_string(),
    }));
}
