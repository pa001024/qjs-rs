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

#[test]
fn module_parse_compact_reexport_from_baseline() {
    let source = "export*from'./dep.js'\nexport{value as answer}from'./dep.js'\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 2);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "*");
    assert_eq!(parsed.imports[1].specifier, "./dep.js");
    assert_eq!(parsed.imports[1].bindings.len(), 1);
    assert_eq!(parsed.imports[1].bindings[0].imported, "value");
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "$__qjs_module_reexport_0__$".to_string(),
    }));
}

#[test]
fn module_parse_multiline_import_export_baseline() {
    let source = "import {\n  value,\n  extra as bonus,\n}\nfrom\n  './dep.js'\nexport const answer =\n  value + bonus\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(
        parsed.imports[0].bindings,
        vec![
            ModuleImportBinding {
                imported: "value".to_string(),
                local: "value".to_string(),
            },
            ModuleImportBinding {
                imported: "extra".to_string(),
                local: "bonus".to_string(),
            },
        ]
    );
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "answer".to_string(),
    }));
}

#[test]
fn module_parse_multiline_named_reexport_baseline() {
    let source = "export {\n  value as answer,\n  default as fallback,\n}\nfrom\n  './dep.js'\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 2);
    assert_eq!(parsed.imports[0].bindings[0].imported, "value");
    assert_eq!(parsed.imports[0].bindings[1].imported, "default");
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
fn module_parse_destructuring_export_declaration_baseline() {
    let source = "const payload = { value: 40, extra: 2 };\nexport const { value, extra } = payload;\nexport const [first, , third] = [1, 2, 3];\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert!(parsed.exports.contains(&ModuleExport {
        exported: "value".to_string(),
        local: "value".to_string(),
    }));
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "extra".to_string(),
        local: "extra".to_string(),
    }));
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "first".to_string(),
        local: "first".to_string(),
    }));
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "third".to_string(),
        local: "third".to_string(),
    }));
    assert!(
        parsed
            .exports
            .iter()
            .all(|entry| !entry.local.starts_with("$__for_in_decl_")),
        "module exports should not leak parser-generated temporary bindings",
    );
}

#[test]
fn module_parse_export_with_object_literal_initializer_baseline() {
    let source = "export const left = { a: 1, b: 2 }, right = 40 + 2;\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert!(parsed.exports.contains(&ModuleExport {
        exported: "left".to_string(),
        local: "left".to_string(),
    }));
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "right".to_string(),
        local: "right".to_string(),
    }));
}

#[test]
fn module_parse_keyword_identifier_names_in_clauses() {
    let source = "const value = 42;\nexport { value as if };\nimport { if as condition } from './dep.js';\nexport { condition as while };\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(
        parsed.imports[0].bindings,
        vec![ModuleImportBinding {
            imported: "if".to_string(),
            local: "condition".to_string(),
        }]
    );
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "if".to_string(),
        local: "value".to_string(),
    }));
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "while".to_string(),
        local: "condition".to_string(),
    }));
}

#[test]
fn module_parse_generator_export_declaration_baseline() {
    let source = "export function* values() { yield 40; yield 2; }\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert!(parsed.exports.contains(&ModuleExport {
        exported: "values".to_string(),
        local: "values".to_string(),
    }));
}

#[test]
fn module_parse_string_named_import_export_clauses() {
    let source = "const value = 42;\nexport { value as \"kebab-name\" };\nimport { \"kebab-name\" as kebabName } from './dep.js';\nexport { kebabName as answer };\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(
        parsed.imports[0].bindings,
        vec![ModuleImportBinding {
            imported: "kebab-name".to_string(),
            local: "kebabName".to_string(),
        }]
    );
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "kebab-name".to_string(),
        local: "value".to_string(),
    }));
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "kebabName".to_string(),
    }));
}

#[test]
fn module_parse_multiline_default_export_expression() {
    let source = "export default\n  42\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert!(parsed.exports.contains(&ModuleExport {
        exported: "default".to_string(),
        local: "$__qjs_module_default_export_0__$".to_string(),
    }));
}

#[test]
fn module_parse_default_named_function_declaration_binding() {
    let source =
        "export default function Named() { return 41; }\nexport const answer = Named() + 1;\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert!(parsed.exports.contains(&ModuleExport {
        exported: "default".to_string(),
        local: "Named".to_string(),
    }));
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "answer".to_string(),
    }));
}

#[test]
fn module_parse_default_named_class_declaration_binding() {
    let source = "export default class Counter { static base() { return 41; } }\nexport const answer = Counter.base() + 1;\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert!(parsed.exports.contains(&ModuleExport {
        exported: "default".to_string(),
        local: "Counter".to_string(),
    }));
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "answer".to_string(),
    }));
}

#[test]
fn module_parse_import_with_attributes_clause() {
    let source =
        "import { value } from './dep.js' with { type: 'json' };\nexport const answer = value;\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(
        parsed.imports[0].bindings,
        vec![ModuleImportBinding {
            imported: "value".to_string(),
            local: "value".to_string(),
        }]
    );
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "answer".to_string(),
    }));
}

#[test]
fn module_parse_reexport_with_attributes_clause() {
    let source = "export { value as answer } from './dep.js' assert { type: 'json' };\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "value");
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "$__qjs_module_reexport_0__$".to_string(),
    }));
}

#[test]
fn module_parse_multiline_import_with_attributes_clause() {
    let source =
        "import { value } from './dep.js'\nwith { type: 'json' }\nexport const answer = value;\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "value");
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "answer".to_string(),
    }));
}

#[test]
fn module_parse_multiline_reexport_with_attributes_clause() {
    let source = "export { value as answer } from './dep.js'\nassert { type: 'json' }\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "value");
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "$__qjs_module_reexport_0__$".to_string(),
    }));
}

#[test]
fn module_parse_default_named_generator_declaration_binding() {
    let source = "export default function* Gen() { yield 40; yield 2; }\nexport const total = Gen().next().value + Gen().next().value;\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert!(parsed.exports.contains(&ModuleExport {
        exported: "default".to_string(),
        local: "Gen".to_string(),
    }));
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "total".to_string(),
        local: "total".to_string(),
    }));
}

#[test]
fn module_parse_string_named_reexport_clause() {
    let source = "export { value as \"kebab-name\" } from './dep.js';\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "value");
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "kebab-name".to_string(),
        local: "$__qjs_module_reexport_0__$".to_string(),
    }));
}

#[test]
fn module_parse_multiline_export_function_declaration_body() {
    let source = "export function build()\n{\n  return 42;\n}\nexport const answer = build();\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert!(parsed.exports.contains(&ModuleExport {
        exported: "build".to_string(),
        local: "build".to_string(),
    }));
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "answer".to_string(),
    }));
}

#[test]
fn module_parse_multiline_default_class_declaration_body() {
    let source = "export default class Counter\n{\n  static value() { return 42; }\n}\nexport const answer = Counter.value();\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert!(parsed.exports.contains(&ModuleExport {
        exported: "default".to_string(),
        local: "Counter".to_string(),
    }));
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "answer".to_string(),
    }));
}

#[test]
fn module_parse_linebreak_as_alias_clauses() {
    let source = "import {\n  value\n  as\n  alias,\n} from './dep.js';\nexport {\n  alias\n  as\n  answer,\n};\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "value");
    assert_eq!(parsed.imports[0].bindings[0].local, "alias");
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "alias".to_string(),
    }));
}

#[test]
fn module_parse_namespace_import_across_linebreaks() {
    let source = "import *\nas\nns from './dep.js';\nexport const answer = ns.value;\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(
        parsed.imports[0].bindings,
        vec![ModuleImportBinding {
            imported: "*".to_string(),
            local: "ns".to_string(),
        }]
    );
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "answer".to_string(),
    }));
}

#[test]
fn module_parse_keyword_only_linebreaks() {
    let source = "import\n{ value } from './dep.js';\nexport\n{ value as answer };\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "value");
    assert_eq!(parsed.imports[0].bindings[0].local, "value");
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "value".to_string(),
    }));
}

#[test]
fn module_parse_string_named_alias_with_spaces() {
    let source = "import { \"kebab name\" as kebabName } from './dep.js';\nexport { kebabName as answer };\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(
        parsed.imports[0].bindings,
        vec![ModuleImportBinding {
            imported: "kebab name".to_string(),
            local: "kebabName".to_string(),
        }]
    );
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "kebabName".to_string(),
    }));
}

#[test]
fn module_parse_string_named_reexport_with_spaces() {
    let source = "export { value as \"kebab name\" } from './dep.js';\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "value");
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "kebab name".to_string(),
        local: "$__qjs_module_reexport_0__$".to_string(),
    }));
}

#[test]
fn module_parse_keyword_block_comment_separators() {
    let source =
        "import/* comment */{ value }from'./dep.js'\nexport/* comment */{value as answer}\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "value");
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "value".to_string(),
    }));
}

#[test]
fn module_parse_keyword_line_comment_continuation() {
    let source =
        "import// comment\n{ value } from './dep.js';\nexport// comment\n{ value as answer };\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "value");
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "value".to_string(),
    }));
}

#[test]
fn module_parse_import_with_comments_around_from_keyword() {
    let source = "import { value }/* gap */from/* gap */'./dep.js';\nexport { value as answer };\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "value");
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "value".to_string(),
    }));
}

#[test]
fn module_parse_reexport_with_comments_around_from_keyword() {
    let source = "export { value as answer }/* gap */from/* gap */'./dep.js';\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "value");
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "$__qjs_module_reexport_0__$".to_string(),
    }));
}

#[test]
fn module_parse_namespace_import_with_comment_after_as() {
    let source = "import * as/* gap */ns from './dep.js';\nexport const answer = ns.value;\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(
        parsed.imports[0].bindings,
        vec![ModuleImportBinding {
            imported: "*".to_string(),
            local: "ns".to_string(),
        }]
    );
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "answer".to_string(),
    }));
}

#[test]
fn module_parse_export_star_namespace_with_comment_after_as() {
    let source = "export * as/* gap */ns from './dep.js';\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "*");
    assert_eq!(parsed.exports.len(), 1);
    assert_eq!(parsed.exports[0].exported, "ns");
}

#[test]
fn module_parse_named_alias_with_comments_around_as() {
    let source = "import { value/* gap */as/* gap */alias } from './dep.js';\nexport { alias/* gap */as/* gap */answer };\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(
        parsed.imports[0].bindings,
        vec![ModuleImportBinding {
            imported: "value".to_string(),
            local: "alias".to_string(),
        }]
    );
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "alias".to_string(),
    }));
}

#[test]
fn module_parse_default_async_function_declaration_binding() {
    let source = "export default async function Named() { return 42; }\nexport const namedType = typeof Named;\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert!(parsed.exports.contains(&ModuleExport {
        exported: "default".to_string(),
        local: "Named".to_string(),
    }));
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "namedType".to_string(),
        local: "namedType".to_string(),
    }));
}

#[test]
fn module_parse_default_async_generator_declaration_binding() {
    let source =
        "export default async function* Gen() { yield 1; }\nexport const genType = typeof Gen;\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert!(parsed.exports.contains(&ModuleExport {
        exported: "default".to_string(),
        local: "Gen".to_string(),
    }));
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "genType".to_string(),
        local: "genType".to_string(),
    }));
}

#[test]
fn module_parse_default_with_comment_separator() {
    let source = "export default/* gap */function Named() { return 42; }\nexport const namedType = typeof Named;\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert!(parsed.exports.contains(&ModuleExport {
        exported: "default".to_string(),
        local: "Named".to_string(),
    }));
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "namedType".to_string(),
        local: "namedType".to_string(),
    }));
}

#[test]
fn module_parse_split_attributes_clause_body() {
    let source =
        "import { value } from './dep.js' assert\n{ type: 'json' };\nexport { value as answer };\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "value");
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "value".to_string(),
    }));
}

#[test]
fn module_parse_split_reexport_attributes_clause_body() {
    let source = "export { value as answer } from './dep.js' assert\n{ type: 'json' };\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "value");
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "$__qjs_module_reexport_0__$".to_string(),
    }));
}

#[test]
fn module_parse_namespace_import_with_split_attributes_clause() {
    let source =
        "import * as ns from './dep.js' with\n{ type: 'json' };\nexport const answer = ns.value;\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(
        parsed.imports[0].bindings,
        vec![ModuleImportBinding {
            imported: "*".to_string(),
            local: "ns".to_string(),
        }]
    );
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "answer".to_string(),
    }));
}

#[test]
fn module_parse_export_star_namespace_with_split_attributes_clause() {
    let source = "export * as ns from './dep.js' assert\n{ type: 'json' };\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "*");
    assert_eq!(parsed.exports.len(), 1);
    assert_eq!(parsed.exports[0].exported, "ns");
}

#[test]
fn module_parse_mixed_default_named_import_with_comments_around_comma() {
    let source = "import fallback/* gap */,/* gap */{ value as named } from './dep.js';\nexport { fallback as left, named as right };\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(
        parsed.imports[0].bindings,
        vec![
            ModuleImportBinding {
                imported: "default".to_string(),
                local: "fallback".to_string(),
            },
            ModuleImportBinding {
                imported: "value".to_string(),
                local: "named".to_string(),
            },
        ]
    );
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "left".to_string(),
        local: "fallback".to_string(),
    }));
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "right".to_string(),
        local: "named".to_string(),
    }));
}

#[test]
fn module_parse_mixed_default_namespace_import_with_comments_around_comma() {
    let source = "import fallback/* gap */,/* gap */* as ns from './dep.js';\nexport const answer = fallback + ns.value;\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(
        parsed.imports[0].bindings,
        vec![
            ModuleImportBinding {
                imported: "default".to_string(),
                local: "fallback".to_string(),
            },
            ModuleImportBinding {
                imported: "*".to_string(),
                local: "ns".to_string(),
            },
        ]
    );
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "answer".to_string(),
        local: "answer".to_string(),
    }));
}

#[test]
fn module_parse_named_clause_entries_with_trailing_comments() {
    let source = "const local = 42;\nexport { local/* gap */ };\nexport { value/* gap */ } from './dep.js';\n";
    let parsed = parse_module(source).expect("module parsing should succeed");

    assert_eq!(parsed.imports.len(), 1);
    assert_eq!(parsed.imports[0].specifier, "./dep.js");
    assert_eq!(parsed.imports[0].bindings.len(), 1);
    assert_eq!(parsed.imports[0].bindings[0].imported, "value");
    assert!(
        parsed.imports[0].bindings[0]
            .local
            .starts_with("$__qjs_module_reexport_")
    );
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "local".to_string(),
        local: "local".to_string(),
    }));
    assert!(parsed.exports.contains(&ModuleExport {
        exported: "value".to_string(),
        local: "$__qjs_module_reexport_0__$".to_string(),
    }));
}
