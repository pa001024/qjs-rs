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
