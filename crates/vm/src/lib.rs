#![forbid(unsafe_code)]

use ast::{BindingKind, ForInitializer, Identifier, Script, Stmt, VariableDeclaration};
use bytecode::{Chunk, CompiledFunction, Opcode, compile_expression, compile_script};
use parser::{parse_expression, parse_script_with_super};
use regex::RegexBuilder;
use runtime::{JsValue, NativeFunction, Realm};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const NON_SIMPLE_PARAMS_MARKER: &str = "$__qjs_non_simple_params__$";
const ARROW_FUNCTION_MARKER: &str = "$__qjs_arrow_function__$";
const REST_PARAM_MARKER_PREFIX: &str = "$__qjs_rest_param__$";
const CLASS_CONSTRUCTOR_MARKER: &str = "$__qjs_class_constructor__$";
const CLASS_DERIVED_CONSTRUCTOR_MARKER: &str = "$__qjs_class_derived_constructor__$";
const CLASS_CONSTRUCTOR_PARENT_MARKER: &str = "$__qjs_class_constructor_parent__$";
const CLASS_HERITAGE_RESTRICTED_MARKER: &str = "$__qjs_class_heritage_restricted__$";
const CLASS_METHOD_NO_PROTOTYPE_MARKER: &str = "$__qjs_class_method_no_prototype__$";
const GENERATOR_FUNCTION_MARKER: &str = "$__qjs_generator_function__$";
const ASYNC_FUNCTION_MARKER: &str = "$__qjs_async_function__$";
const NAMED_FUNCTION_EXPR_MARKER: &str = "$__qjs_named_function_expr__$";
const DERIVED_THIS_BINDING: &str = "$__qjs_derived_this__$";
const BOXED_PRIMITIVE_VALUE_KEY: &str = "$__qjs_boxed_primitive_value__$";
const DATE_OBJECT_MARKER_KEY: &str = "$__qjs_date_object__$";
const GENERATOR_VALUES_KEY: &str = "$__qjs_generator_values__$";
const GENERATOR_INDEX_KEY: &str = "$__qjs_generator_index__$";
const GENERATOR_ITERATOR_MARKER_KEY: &str = "$__qjs_generator_iterator__$";
const GENERATOR_PRODUCER_CLOSURE_KEY: &str = "$__qjs_generator_producer_closure__$";
const GENERATOR_PRODUCER_ARGS_KEY: &str = "$__qjs_generator_producer_args__$";
const GENERATOR_PRODUCER_THIS_KEY: &str = "$__qjs_generator_producer_this__$";
const GENERATOR_PRODUCER_THIS_IS_MISSING_KEY: &str = "$__qjs_generator_producer_this_missing__$";
const ARRAY_ITERATOR_TARGET_KEY: &str = "$__qjs_array_iterator_target__$";
const ARRAY_ITERATOR_INDEX_KEY: &str = "$__qjs_array_iterator_index__$";
const ARRAY_ITERATOR_KIND_KEY: &str = "$__qjs_array_iterator_kind__$";
const ARRAY_ITERATOR_MARKER_KEY: &str = "$__qjs_array_iterator__$";
const SURROGATE_PLACEHOLDER_START: u32 = 0xE000;
const SURROGATE_PLACEHOLDER_END: u32 = 0xE7FF;
const SURROGATE_START: u16 = 0xD800;
const OBJECT_ID_SLOT_BITS: u64 = 32;
const OBJECT_ID_SLOT_MASK: u64 = (1u64 << OBJECT_ID_SLOT_BITS) - 1;

type BindingId = u64;
type ObjectId = u64;
type Scope = BTreeMap<String, BindingId>;
type ScopeRef = Rc<RefCell<Scope>>;

#[derive(Debug, Clone, PartialEq)]
struct Binding {
    value: JsValue,
    mutable: bool,
    deletable: bool,
    sloppy_readonly_write_ignored: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct Closure {
    function_id: usize,
    functions: Rc<Vec<CompiledFunction>>,
    captured_scopes: Vec<ScopeRef>,
    captured_with_objects: Vec<WithFrame>,
    strict: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct JsObject {
    properties: BTreeMap<String, JsValue>,
    getters: BTreeMap<String, JsValue>,
    setters: BTreeMap<String, JsValue>,
    property_attributes: BTreeMap<String, PropertyAttributes>,
    argument_mappings: BTreeMap<String, BindingId>,
    prototype: Option<ObjectId>,
    prototype_value: Option<JsValue>,
    prototype_overridden: bool,
    extensible: bool,
}

impl Default for JsObject {
    fn default() -> Self {
        Self {
            properties: BTreeMap::new(),
            getters: BTreeMap::new(),
            setters: BTreeMap::new(),
            property_attributes: BTreeMap::new(),
            argument_mappings: BTreeMap::new(),
            prototype: None,
            prototype_value: None,
            prototype_overridden: false,
            extensible: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PropertyAttributes {
    writable: bool,
    enumerable: bool,
    configurable: bool,
}

impl Default for PropertyAttributes {
    fn default() -> Self {
        Self {
            writable: true,
            enumerable: true,
            configurable: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct ExceptionHandler {
    catch_target: Option<usize>,
    finally_target: Option<usize>,
    scope_depth: usize,
    stack_depth: usize,
    with_depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FunctionMethod {
    Call,
    Apply,
    Bind,
}

#[derive(Debug, Clone, PartialEq)]
enum HostFunction {
    BoundMethod {
        target: JsValue,
        method: FunctionMethod,
    },
    BoundCall {
        target: JsValue,
        this_arg: JsValue,
        bound_args: Vec<JsValue>,
    },
    GeneratorFactory {
        producer: JsValue,
    },
    GeneratorIteratorNextThis,
    StringReplaceThis,
    StringMatchThis,
    StringSearchThis,
    StringIndexOfThis,
    StringSplitThis,
    StringToLowerCaseThis,
    StringToUpperCase,
    StringTrim,
    StringToString,
    StringValueOf,
    StringCharAt,
    StringCharCodeAt,
    StringLastIndexOf,
    StringSubstring,
    NumberToString,
    NumberValueOf,
    NumberToFixed,
    NumberToExponential,
    ArrayBufferSliceThis,
    MapSetThis,
    SetAddThis,
    BooleanToString,
    BooleanValueOf,
    FunctionPrototype,
    JsonStringify,
    JsonParse,
    ArrayPush(ObjectId),
    ArrayForEach(ObjectId),
    ArrayReduce(ObjectId),
    ArrayJoin(ObjectId),
    ArrayJoinThis,
    ArrayConcatThis,
    ArrayPopThis,
    ArrayKeysThis,
    ArrayEntriesThis,
    ArrayValuesThis,
    ArrayIteratorNextThis,
    ArrayReverse(ObjectId),
    ArraySort(ObjectId),
    RegExpTestThis,
    RegExpExecThis,
    RegExpToStringThis,
    HasOwnProperty {
        target: JsValue,
    },
    IsPrototypeOf {
        target: JsValue,
    },
    ObjectToString,
    ObjectValueOf,
    ErrorToStringThis,
    AssertSameValue,
    AssertNotSameValue,
    AssertThrows,
    AssertCompareArray,
    ThrowTypeError,
    DateToString(ObjectId),
    DateValueOf(ObjectId),
    DateGetFullYearThis,
    DateGetMonthThis,
    DateGetDateThis,
    DateGetUTCFullYearThis,
    DateGetUTCMonthThis,
    DateGetUTCDateThis,
    FunctionToString {
        target: JsValue,
    },
    FunctionValueOf {
        target: JsValue,
    },
}

#[derive(Debug, Clone, PartialEq)]
enum IdentifierReference {
    Binding {
        name: String,
        binding_id: BindingId,
    },
    Property {
        base: JsValue,
        property: String,
        strict_on_missing: bool,
        with_base_object: bool,
    },
    Unresolvable {
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
struct WithFrame {
    object: JsValue,
    scope_depth: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum VmError {
    EmptyStack,
    StackUnderflow,
    ScopeUnderflow,
    UnknownIdentifier(String),
    ImmutableBinding(String),
    VariableAlreadyDefined(String),
    UnknownClosure(u64),
    UnknownHostFunction(u64),
    UnknownFunction(usize),
    UnknownObject(u64),
    NotCallable,
    TopLevelReturn,
    InvalidJump(usize),
    HandlerUnderflow,
    NoPendingException,
    UncaughtException(JsValue),
    TypeError(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecutionSignal {
    Halt,
    Return,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EvalContext {
    is_arrow_function: bool,
    non_simple_params: bool,
    has_arguments_param: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EvalCallKind {
    Direct,
    Indirect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GcStats {
    pub roots: usize,
    pub objects_before: usize,
    pub marked_objects: usize,
    pub reclaimed_objects: usize,
    pub remaining_objects: usize,
    pub peak_objects: usize,
    pub mark_duration_ns: u128,
    pub sweep_duration_ns: u128,
    pub collections_total: usize,
    pub boundary_collections: usize,
    pub runtime_collections: usize,
}

#[derive(Debug, Default)]
struct GcShadowRoots {
    stack: Vec<JsValue>,
    scopes: Vec<ScopeRef>,
    with_objects: Vec<WithFrame>,
    pending_exception: Option<JsValue>,
    identifier_references: Vec<IdentifierReference>,
}

#[derive(Debug, Default)]
pub struct Vm {
    stack: Vec<JsValue>,
    scopes: Vec<ScopeRef>,
    bindings: BTreeMap<BindingId, Binding>,
    next_binding_id: BindingId,
    objects: BTreeMap<ObjectId, JsObject>,
    next_object_slot: u32,
    object_generations: Vec<u32>,
    closures: BTreeMap<u64, Closure>,
    closure_objects: BTreeMap<u64, JsObject>,
    next_closure_id: u64,
    host_functions: BTreeMap<u64, HostFunction>,
    host_function_objects: BTreeMap<u64, JsObject>,
    next_host_function_id: u64,
    host_pins: BTreeMap<u64, JsValue>,
    next_host_pin_id: u64,
    object_to_string_host_id: Option<u64>,
    global_object_id: Option<ObjectId>,
    object_prototype_id: Option<ObjectId>,
    function_prototype_host_id: Option<u64>,
    function_prototype_prototype_getter: Option<JsValue>,
    generator_function_prototype_id: Option<ObjectId>,
    array_prototype_id: Option<ObjectId>,
    string_prototype_id: Option<ObjectId>,
    number_prototype_id: Option<ObjectId>,
    boolean_prototype_id: Option<ObjectId>,
    error_prototype_id: Option<ObjectId>,
    type_error_prototype_id: Option<ObjectId>,
    date_prototype_id: Option<ObjectId>,
    regexp_prototype_id: Option<ObjectId>,
    array_buffer_prototype_id: Option<ObjectId>,
    data_view_prototype_id: Option<ObjectId>,
    map_prototype_id: Option<ObjectId>,
    set_prototype_id: Option<ObjectId>,
    promise_prototype_id: Option<ObjectId>,
    uint8_array_prototype_id: Option<ObjectId>,
    with_objects: Vec<WithFrame>,
    identifier_references: Vec<IdentifierReference>,
    exception_handlers: Vec<ExceptionHandler>,
    pending_exception: Option<JsValue>,
    template_cache: BTreeMap<u64, JsValue>,
    eval_contexts: Vec<EvalContext>,
    eval_deletable_binding_depth: usize,
    param_init_body_scopes: Vec<ScopeRef>,
    var_scope_stack: Vec<usize>,
    gc_mark_stack: Vec<JsValue>,
    gc_shadow_roots: Vec<GcShadowRoots>,
    gc_last_stats: GcStats,
    free_object_slots: Vec<u32>,
    gc_peak_objects: usize,
    auto_gc_enabled: bool,
    auto_gc_object_threshold: usize,
    runtime_gc_enabled: bool,
    runtime_gc_check_interval: usize,
    runtime_gc_tick: usize,
    gc_collections_total: usize,
    gc_boundary_collections: usize,
    gc_runtime_collections: usize,
    gc_reclaimed_objects_total: usize,
}

impl Vm {
    pub fn execute(&mut self, chunk: &Chunk) -> Result<JsValue, VmError> {
        let empty_realm = Realm::default();
        self.execute_in_realm(chunk, &empty_realm)
    }

    pub fn execute_in_realm(&mut self, chunk: &Chunk, realm: &Realm) -> Result<JsValue, VmError> {
        self.stack.clear();
        self.scopes.clear();
        self.scopes.push(Rc::new(RefCell::new(BTreeMap::new())));
        self.bindings.clear();
        self.next_binding_id = 0;
        self.objects.clear();
        self.next_object_slot = 0;
        self.object_generations.clear();
        self.closures.clear();
        self.closure_objects.clear();
        self.next_closure_id = 0;
        self.host_functions.clear();
        self.host_function_objects.clear();
        self.next_host_function_id = 0;
        self.host_pins.clear();
        self.next_host_pin_id = 0;
        self.object_to_string_host_id = None;
        self.global_object_id = None;
        self.object_prototype_id = None;
        self.function_prototype_host_id = None;
        self.function_prototype_prototype_getter = None;
        self.generator_function_prototype_id = None;
        self.array_prototype_id = None;
        self.string_prototype_id = None;
        self.number_prototype_id = None;
        self.boolean_prototype_id = None;
        self.error_prototype_id = None;
        self.type_error_prototype_id = None;
        self.date_prototype_id = None;
        self.regexp_prototype_id = None;
        self.array_buffer_prototype_id = None;
        self.data_view_prototype_id = None;
        self.map_prototype_id = None;
        self.set_prototype_id = None;
        self.promise_prototype_id = None;
        self.uint8_array_prototype_id = None;
        self.with_objects.clear();
        self.identifier_references.clear();
        self.exception_handlers.clear();
        self.pending_exception = None;
        self.template_cache.clear();
        self.eval_contexts.clear();
        self.eval_deletable_binding_depth = 0;
        self.param_init_body_scopes.clear();
        self.var_scope_stack.clear();
        self.gc_mark_stack.clear();
        self.gc_shadow_roots.clear();
        self.gc_last_stats = GcStats::default();
        self.free_object_slots.clear();
        self.gc_peak_objects = 0;
        self.runtime_gc_tick = 0;
        self.gc_collections_total = 0;
        self.gc_boundary_collections = 0;
        self.gc_runtime_collections = 0;
        self.gc_reclaimed_objects_total = 0;
        let object_prototype = self.create_object_value();
        if let JsValue::Object(id) = object_prototype {
            self.object_prototype_id = Some(id);
        }
        let _ = self.function_prototype_value();
        let _ = self.array_prototype_value();
        let global_object = self.create_object_value();
        if let JsValue::Object(id) = global_object {
            self.global_object_id = Some(id);
        }
        self.seed_global_constant_properties()?;
        self.seed_global_realm_properties(realm)?;
        let _ = self.math_object_value()?;
        let _ = self.json_object_value()?;
        let _ = self.reflect_object_value()?;
        if let Some(object_prototype_id) = self.object_prototype_id {
            let to_string = self.shared_object_to_string_function();
            let value_of = self.create_host_function_value(HostFunction::ObjectValueOf);
            if let Some(object_prototype) = self.objects.get_mut(&object_prototype_id) {
                object_prototype.properties.insert(
                    "constructor".to_string(),
                    JsValue::NativeFunction(NativeFunction::ObjectConstructor),
                );
                object_prototype.property_attributes.insert(
                    "constructor".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
                object_prototype
                    .properties
                    .insert("toString".to_string(), to_string);
                object_prototype.property_attributes.insert(
                    "toString".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
                object_prototype
                    .properties
                    .insert("valueOf".to_string(), value_of);
                object_prototype.property_attributes.insert(
                    "valueOf".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
            }
        }

        let strict = self.code_is_strict(&chunk.code);
        self.var_scope_stack.push(0);
        let signal = self.execute_code(&chunk.code, &chunk.functions, realm, false, strict);
        let _ = self.var_scope_stack.pop();
        if signal.is_ok() {
            self.collect_garbage_if_needed(realm, false);
        }
        match signal? {
            ExecutionSignal::Halt => self.stack.pop().ok_or(VmError::EmptyStack),
            ExecutionSignal::Return => Err(VmError::TopLevelReturn),
        }
    }

    pub fn enable_auto_gc(&mut self, enabled: bool) {
        self.auto_gc_enabled = enabled;
    }

    pub fn set_auto_gc_object_threshold(&mut self, threshold: usize) {
        self.auto_gc_object_threshold = threshold;
    }

    pub fn enable_runtime_gc(&mut self, enabled: bool) {
        self.runtime_gc_enabled = enabled;
    }

    pub fn set_runtime_gc_check_interval(&mut self, interval: usize) {
        self.runtime_gc_check_interval = interval.max(1);
    }

    pub fn pin_host_value(&mut self, value: JsValue) -> u64 {
        let handle = self.next_host_pin_id;
        self.next_host_pin_id += 1;
        self.host_pins.insert(handle, value);
        handle
    }

    pub fn unpin_host_value(&mut self, handle: u64) -> Option<JsValue> {
        self.host_pins.remove(&handle)
    }

    pub fn collect_garbage(&mut self, realm: &Realm) -> GcStats {
        self.gc_collections_total += 1;
        let objects_before = self.objects.len();
        let roots = self.collect_roots(realm);
        let mark_started = Instant::now();
        let marked_objects = self.mark_from_roots(&roots);
        let mark_duration_ns = mark_started.elapsed().as_nanos();
        let sweep_started = Instant::now();
        let reclaimed = self.sweep_unreachable(&marked_objects);
        let sweep_duration_ns = sweep_started.elapsed().as_nanos();
        self.gc_reclaimed_objects_total = self
            .gc_reclaimed_objects_total
            .checked_add(reclaimed)
            .expect("gc reclaimed counter overflow");
        let stats = GcStats {
            roots: roots.len(),
            objects_before,
            marked_objects: marked_objects.len(),
            reclaimed_objects: self.gc_reclaimed_objects_total,
            remaining_objects: self.objects.len(),
            peak_objects: self.gc_peak_objects,
            mark_duration_ns,
            sweep_duration_ns,
            collections_total: self.gc_collections_total,
            boundary_collections: self.gc_boundary_collections,
            runtime_collections: self.gc_runtime_collections,
        };
        self.gc_last_stats = stats;
        stats
    }

    pub fn gc_stats(&self) -> GcStats {
        self.gc_last_stats
    }

    fn collect_garbage_if_needed(&mut self, realm: &Realm, runtime: bool) -> Option<GcStats> {
        if self.should_trigger_gc() {
            if runtime {
                self.gc_runtime_collections += 1;
            } else {
                self.gc_boundary_collections += 1;
            }
            Some(self.collect_garbage(realm))
        } else {
            None
        }
    }

    fn should_trigger_gc(&self) -> bool {
        self.auto_gc_enabled
            && self.auto_gc_object_threshold > 0
            && self.objects.len() >= self.auto_gc_object_threshold
    }

    fn object_id_slot(object_id: ObjectId) -> u32 {
        (object_id & OBJECT_ID_SLOT_MASK) as u32
    }

    #[cfg(test)]
    fn object_id_generation(object_id: ObjectId) -> u32 {
        (object_id >> OBJECT_ID_SLOT_BITS) as u32
    }

    fn make_object_id(slot: u32, generation: u32) -> ObjectId {
        ((generation as u64) << OBJECT_ID_SLOT_BITS) | (slot as u64)
    }

    fn allocate_object_id(&mut self) -> ObjectId {
        if let Some(slot) = self.free_object_slots.pop() {
            let index = slot as usize;
            let generation = self.object_generations[index]
                .checked_add(1)
                .expect("object generation overflow");
            self.object_generations[index] = generation;
            Self::make_object_id(slot, generation)
        } else {
            let slot = self.next_object_slot;
            self.next_object_slot = self
                .next_object_slot
                .checked_add(1)
                .expect("object slot overflow");
            self.object_generations.push(0);
            Self::make_object_id(slot, 0)
        }
    }

    fn collect_roots(&self, realm: &Realm) -> Vec<JsValue> {
        let mut roots = Vec::new();
        roots.extend(self.stack.iter().cloned());
        if let Some(exception) = &self.pending_exception {
            roots.push(exception.clone());
        }
        self.collect_roots_from_scopes(&self.scopes, &mut roots);
        for frame in &self.with_objects {
            roots.push(frame.object.clone());
        }
        Self::collect_roots_from_identifier_references(&self.identifier_references, &mut roots);
        for shadow in &self.gc_shadow_roots {
            roots.extend(shadow.stack.iter().cloned());
            if let Some(exception) = &shadow.pending_exception {
                roots.push(exception.clone());
            }
            self.collect_roots_from_scopes(&shadow.scopes, &mut roots);
            for frame in &shadow.with_objects {
                roots.push(frame.object.clone());
            }
            Self::collect_roots_from_identifier_references(
                &shadow.identifier_references,
                &mut roots,
            );
        }
        if let Some(global_id) = self.global_object_id {
            roots.push(JsValue::Object(global_id));
        }
        for proto_id in [
            self.object_prototype_id,
            self.array_prototype_id,
            self.string_prototype_id,
            self.number_prototype_id,
            self.boolean_prototype_id,
            self.error_prototype_id,
            self.type_error_prototype_id,
            self.date_prototype_id,
            self.regexp_prototype_id,
            self.array_buffer_prototype_id,
            self.data_view_prototype_id,
            self.map_prototype_id,
            self.set_prototype_id,
            self.promise_prototype_id,
            self.uint8_array_prototype_id,
        ]
        .into_iter()
        .flatten()
        {
            roots.push(JsValue::Object(proto_id));
        }
        if let Some(host_id) = self.function_prototype_host_id {
            roots.push(JsValue::HostFunction(host_id));
        }
        if let Some(getter) = &self.function_prototype_prototype_getter {
            roots.push(getter.clone());
        }
        roots.extend(realm.globals_values().cloned());
        roots.extend(self.template_cache.values().cloned());
        roots.extend(self.closures.keys().copied().map(JsValue::Function));
        roots.extend(
            self.host_functions
                .keys()
                .copied()
                .map(JsValue::HostFunction),
        );
        roots.extend(self.host_pins.values().cloned());
        roots
    }

    fn collect_roots_from_scopes(&self, scopes: &[ScopeRef], roots: &mut Vec<JsValue>) {
        for scope_ref in scopes {
            for binding_id in scope_ref.borrow().values() {
                if let Some(binding) = self.bindings.get(binding_id) {
                    roots.push(binding.value.clone());
                }
            }
        }
    }

    fn collect_roots_from_identifier_references(
        identifier_references: &[IdentifierReference],
        roots: &mut Vec<JsValue>,
    ) {
        for reference in identifier_references {
            if let IdentifierReference::Property { base, .. } = reference {
                roots.push(base.clone());
            }
        }
    }

    fn mark_from_roots(&mut self, roots: &[JsValue]) -> BTreeSet<ObjectId> {
        let mut marked_objects = BTreeSet::new();
        let mut marked_closures = BTreeSet::new();
        let mut marked_hosts = BTreeSet::new();
        self.gc_mark_stack.clear();
        self.gc_mark_stack.extend(roots.iter().cloned());
        while let Some(value) = self.gc_mark_stack.pop() {
            self.mark_value(
                value,
                &mut marked_objects,
                &mut marked_closures,
                &mut marked_hosts,
            );
        }
        marked_objects
    }

    fn mark_value(
        &mut self,
        value: JsValue,
        marked_objects: &mut BTreeSet<ObjectId>,
        marked_closures: &mut BTreeSet<u64>,
        marked_hosts: &mut BTreeSet<u64>,
    ) {
        match value {
            JsValue::Object(object_id) => {
                self.mark_object_edges(object_id, marked_objects);
            }
            JsValue::Function(closure_id) => {
                if !marked_closures.insert(closure_id) {
                    return;
                }
                if let Some(closure_object) = self.closure_objects.get(&closure_id).cloned() {
                    self.enqueue_object_values(closure_object);
                }
                if let Some(closure) = self.closures.get(&closure_id).cloned() {
                    for scope_ref in closure.captured_scopes {
                        let binding_ids: Vec<_> = scope_ref.borrow().values().copied().collect();
                        for binding_id in binding_ids {
                            if let Some(binding) = self.bindings.get(&binding_id) {
                                self.gc_mark_stack.push(binding.value.clone());
                            }
                        }
                    }
                    for frame in closure.captured_with_objects {
                        self.gc_mark_stack.push(frame.object);
                    }
                }
            }
            JsValue::HostFunction(host_id) => {
                if !marked_hosts.insert(host_id) {
                    return;
                }
                if let Some(host_object) = self.host_function_objects.get(&host_id).cloned() {
                    self.enqueue_object_values(host_object);
                }
                if let Some(host) = self.host_functions.get(&host_id).cloned() {
                    self.enqueue_host_function_values(host);
                }
            }
            _ => {}
        }
    }

    fn mark_object_edges(&mut self, object_id: ObjectId, marked_objects: &mut BTreeSet<ObjectId>) {
        if !marked_objects.insert(object_id) {
            return;
        }
        if let Some(object) = self.objects.get(&object_id).cloned() {
            self.enqueue_object_values(object);
        }
    }

    fn enqueue_object_values(&mut self, object: JsObject) {
        self.gc_mark_stack.extend(object.properties.into_values());
        self.gc_mark_stack.extend(object.getters.into_values());
        self.gc_mark_stack.extend(object.setters.into_values());
        if let Some(prototype_value) = object.prototype_value {
            self.gc_mark_stack.push(prototype_value);
        } else if let Some(proto_id) = object.prototype {
            self.gc_mark_stack.push(JsValue::Object(proto_id));
        }
    }

    fn enqueue_host_function_values(&mut self, host: HostFunction) {
        match host {
            HostFunction::BoundMethod { target, .. } => {
                self.gc_mark_stack.push(target);
            }
            HostFunction::BoundCall {
                target,
                this_arg,
                bound_args,
            } => {
                self.gc_mark_stack.push(target);
                self.gc_mark_stack.push(this_arg);
                self.gc_mark_stack.extend(bound_args);
            }
            HostFunction::GeneratorFactory { producer } => {
                self.gc_mark_stack.push(producer);
            }
            HostFunction::ArrayPush(object_id)
            | HostFunction::ArrayForEach(object_id)
            | HostFunction::ArrayReduce(object_id)
            | HostFunction::ArrayJoin(object_id)
            | HostFunction::ArrayReverse(object_id)
            | HostFunction::ArraySort(object_id)
            | HostFunction::DateToString(object_id)
            | HostFunction::DateValueOf(object_id) => {
                self.gc_mark_stack.push(JsValue::Object(object_id));
            }
            HostFunction::HasOwnProperty { target }
            | HostFunction::IsPrototypeOf { target }
            | HostFunction::FunctionToString { target }
            | HostFunction::FunctionValueOf { target } => {
                self.gc_mark_stack.push(target);
            }
            HostFunction::StringReplaceThis
            | HostFunction::StringMatchThis
            | HostFunction::StringSearchThis
            | HostFunction::StringIndexOfThis
            | HostFunction::StringSplitThis
            | HostFunction::StringToLowerCaseThis
            | HostFunction::StringToUpperCase
            | HostFunction::StringTrim
            | HostFunction::StringToString
            | HostFunction::StringValueOf
            | HostFunction::StringCharAt
            | HostFunction::StringCharCodeAt
            | HostFunction::StringLastIndexOf
            | HostFunction::StringSubstring
            | HostFunction::NumberToString
            | HostFunction::NumberValueOf
            | HostFunction::NumberToFixed
            | HostFunction::NumberToExponential
            | HostFunction::ArrayBufferSliceThis
            | HostFunction::MapSetThis
            | HostFunction::SetAddThis
            | HostFunction::BooleanToString
            | HostFunction::BooleanValueOf
            | HostFunction::ArrayConcatThis
            | HostFunction::ErrorToStringThis
            | HostFunction::DateGetFullYearThis
            | HostFunction::DateGetMonthThis
            | HostFunction::DateGetDateThis
            | HostFunction::DateGetUTCFullYearThis
            | HostFunction::DateGetUTCMonthThis
            | HostFunction::DateGetUTCDateThis
            | HostFunction::ThrowTypeError
            | HostFunction::FunctionPrototype
            | HostFunction::JsonStringify
            | HostFunction::JsonParse
            | HostFunction::ObjectToString
            | HostFunction::ObjectValueOf
            | HostFunction::ArrayJoinThis
            | HostFunction::ArrayPopThis
            | HostFunction::ArrayKeysThis
            | HostFunction::ArrayEntriesThis
            | HostFunction::ArrayValuesThis
            | HostFunction::ArrayIteratorNextThis
            | HostFunction::GeneratorIteratorNextThis
            | HostFunction::RegExpTestThis
            | HostFunction::RegExpExecThis
            | HostFunction::RegExpToStringThis
            | HostFunction::AssertSameValue
            | HostFunction::AssertNotSameValue
            | HostFunction::AssertThrows
            | HostFunction::AssertCompareArray => {}
        }
    }

    fn sweep_unreachable(&mut self, marked_objects: &BTreeSet<ObjectId>) -> usize {
        let unreached: Vec<_> = self
            .objects
            .keys()
            .copied()
            .filter(|id| !marked_objects.contains(id))
            .collect();
        let reclaimed = unreached.len();
        for id in unreached {
            self.objects.remove(&id);
            self.free_object_slots.push(Self::object_id_slot(id));
        }
        reclaimed
    }

    fn execute_code(
        &mut self,
        code: &[Opcode],
        functions: &[CompiledFunction],
        realm: &Realm,
        allow_return: bool,
        strict: bool,
    ) -> Result<ExecutionSignal, VmError> {
        let mut pc = 0usize;
        let check_interval = self.runtime_gc_check_interval.max(1);
        while pc < code.len() {
            if self.runtime_gc_enabled && self.auto_gc_enabled {
                self.runtime_gc_tick = self.runtime_gc_tick.saturating_add(1);
                if self.runtime_gc_tick % check_interval == 0 {
                    self.collect_garbage_if_needed(realm, true);
                }
            }
            match &code[pc] {
                Opcode::LoadNumber(value) => self.stack.push(JsValue::Number(*value)),
                Opcode::LoadBool(value) => self.stack.push(JsValue::Bool(*value)),
                Opcode::LoadNull => self.stack.push(JsValue::Null),
                Opcode::LoadString(value) => self.stack.push(JsValue::String(value.clone())),
                Opcode::LoadUndefined => self.stack.push(JsValue::Undefined),
                Opcode::LoadUninitialized => self.stack.push(JsValue::Uninitialized),
                Opcode::CreateObject => {
                    let object = self.create_object_value();
                    self.stack.push(object);
                }
                Opcode::CreateArray => {
                    let array = self.create_array_value();
                    self.stack.push(array);
                }
                Opcode::LoadFunction(function_id) => {
                    let function = self.instantiate_function(*function_id, functions, strict)?;
                    self.stack.push(function);
                }
                Opcode::LoadIdentifier(name) => {
                    let resolved = if let Some(reference) =
                        self.resolve_binding_or_with_reference(name, realm)?
                    {
                        self.load_identifier_reference_value(&reference, realm, strict)
                    } else if name == "undefined" {
                        Ok(JsValue::Undefined)
                    } else if name == "NaN" {
                        Ok(JsValue::Number(f64::NAN))
                    } else if name == "Infinity" {
                        Ok(JsValue::Number(f64::INFINITY))
                    } else if name == "globalThis" {
                        Ok(self.global_this_value())
                    } else if name == "Math" {
                        self.math_object_value()
                    } else if name == "Object" {
                        Ok(JsValue::NativeFunction(NativeFunction::ObjectConstructor))
                    } else if name == "JSON" {
                        self.json_object_value()
                    } else if name == "Reflect" {
                        self.reflect_object_value()
                    } else if name == "super" {
                        if let Some(base) = self.resolve_super_base_value() {
                            Ok(base)
                        } else {
                            Err(VmError::UnknownIdentifier(name.clone()))
                        }
                    } else if name == "this" {
                        Ok(realm
                            .resolve_identifier(name)
                            .unwrap_or_else(|| self.global_this_value()))
                    } else if let Some(value) = realm.resolve_identifier(name) {
                        Ok(value)
                    } else if let Some(global_object_id) = self.global_object_id {
                        let receiver = JsValue::Object(global_object_id);
                        if self.has_property_on_receiver(&receiver, name, realm)? {
                            self.get_property_from_receiver(receiver, name, realm)
                        } else {
                            Err(VmError::UnknownIdentifier(name.clone()))
                        }
                    } else {
                        Err(VmError::UnknownIdentifier(name.clone()))
                    };
                    match resolved {
                        Ok(value) => self.stack.push(value),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::DefineVar(name) => {
                    let existing_binding_id = {
                        let scope_ref = self.current_var_scope_ref()?;
                        scope_ref.borrow().get(name).copied()
                    };
                    if let Some(existing_binding_id) = existing_binding_id {
                        let existing_binding = self
                            .bindings
                            .get(&existing_binding_id)
                            .ok_or(VmError::ScopeUnderflow)?;
                        if !existing_binding.mutable {
                            return Err(VmError::VariableAlreadyDefined(name.clone()));
                        }
                    } else {
                        let binding_id = self.create_binding_with_flags(
                            JsValue::Undefined,
                            true,
                            self.eval_deletable_binding_depth > 0,
                        );
                        let scope_ref = self.current_var_scope_ref()?;
                        if scope_ref
                            .borrow_mut()
                            .insert(name.clone(), binding_id)
                            .is_some()
                        {
                            return Err(VmError::VariableAlreadyDefined(name.clone()));
                        }
                    }
                    self.define_global_var_property(name)?;
                }
                Opcode::DefineVariable { name, mutable } => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    self.maybe_set_inferred_function_name(&value, name);
                    let existing_binding_id = {
                        let scope_ref = self.current_scope_ref()?;
                        scope_ref.borrow().get(name).copied()
                    };
                    if let Some(existing_binding_id) = existing_binding_id {
                        let existing_binding = self
                            .bindings
                            .get_mut(&existing_binding_id)
                            .ok_or(VmError::ScopeUnderflow)?;
                        if !existing_binding.mutable {
                            return Err(VmError::VariableAlreadyDefined(name.clone()));
                        }
                        let should_reset_undefined = name.starts_with("$__loop_completion_")
                            || name.starts_with("$__switch_tmp_")
                            || name.starts_with("$__class_ctor_");
                        if matches!(existing_binding.value, JsValue::Uninitialized)
                            || value != JsValue::Undefined
                            || should_reset_undefined
                        {
                            existing_binding.value = value;
                        }
                    } else {
                        let binding_id = self.create_binding(value, *mutable);
                        let scope_ref = self.current_scope_ref()?;
                        if scope_ref
                            .borrow_mut()
                            .insert(name.clone(), binding_id)
                            .is_some()
                        {
                            return Err(VmError::VariableAlreadyDefined(name.clone()));
                        }
                    }
                }
                Opcode::DefineFunction { name, function_id } => {
                    let function_value =
                        self.instantiate_function(*function_id, functions, strict)?;
                    let existing_binding_id = {
                        let scope_ref = self.current_scope_ref()?;
                        scope_ref.borrow().get(name).copied()
                    };
                    if let Some(existing_binding_id) = existing_binding_id {
                        let existing_binding = self
                            .bindings
                            .get_mut(&existing_binding_id)
                            .ok_or(VmError::ScopeUnderflow)?;
                        if !existing_binding.mutable {
                            return Err(VmError::VariableAlreadyDefined(name.clone()));
                        }
                        existing_binding.value = function_value;
                    } else {
                        let function_binding = self.create_binding_with_flags(
                            function_value,
                            true,
                            self.eval_deletable_binding_depth > 0,
                        );
                        let scope_ref = self.current_scope_ref()?;
                        if scope_ref
                            .borrow_mut()
                            .insert(name.clone(), function_binding)
                            .is_some()
                        {
                            return Err(VmError::VariableAlreadyDefined(name.clone()));
                        }
                    }
                }
                Opcode::StoreVariable(name) => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    self.maybe_set_inferred_function_name(&value, name);
                    if let Some(binding_id) = self.resolve_binding_id(name) {
                        let mut wrote_binding = false;
                        {
                            let binding = self
                                .bindings
                                .get_mut(&binding_id)
                                .ok_or(VmError::ScopeUnderflow)?;
                            if matches!(binding.value, JsValue::Uninitialized) {
                                return Err(VmError::UnknownIdentifier(name.clone()));
                            }
                            if !binding.mutable {
                                if !(binding.sloppy_readonly_write_ignored && !strict) {
                                    return Err(VmError::ImmutableBinding(name.clone()));
                                }
                            } else {
                                binding.value = value.clone();
                                wrote_binding = true;
                            }
                        }
                        if wrote_binding {
                            self.sync_global_property_from_binding(name, binding_id)?;
                        }
                    } else {
                        if strict {
                            return Err(VmError::UnknownIdentifier(name.clone()));
                        }
                        if let Some(global_object_id) = self.global_object_id {
                            let _ = self.set_object_property(
                                global_object_id,
                                name.clone(),
                                value.clone(),
                                realm,
                            )?;
                        } else {
                            let global_scope = self
                                .scopes
                                .first()
                                .cloned()
                                .ok_or(VmError::ScopeUnderflow)?;
                            let binding_id = self.create_binding(value.clone(), true);
                            global_scope.borrow_mut().insert(name.clone(), binding_id);
                        }
                    }
                    self.stack.push(value);
                }
                Opcode::GetProperty(name) => {
                    let receiver = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let result = self.get_property_from_receiver(receiver, name, realm);
                    match result {
                        Ok(value) => self.stack.push(value),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::GetPropertyByValue => {
                    let key = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let key = self.coerce_to_property_key(&key);
                    let receiver = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let result = self.get_property_from_receiver(receiver, &key, realm);
                    match result {
                        Ok(value) => self.stack.push(value),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::GetSuperProperty(name) => {
                    let this_value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let base = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    match self.get_property_from_base_with_receiver(base, name, this_value, realm) {
                        Ok(value) => self.stack.push(value),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::GetSuperPropertyByValue => {
                    let key_value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let this_value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let base = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let result = (|| {
                        let key = self.coerce_to_property_key_runtime(key_value, realm, strict)?;
                        self.get_property_from_base_with_receiver(base, &key, this_value, realm)
                    })();
                    match result {
                        Ok(value) => self.stack.push(value),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::PrepareSuperMethod(name) => {
                    let this_value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let base = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    match self.get_property_from_base_with_receiver(
                        base,
                        name,
                        this_value.clone(),
                        realm,
                    ) {
                        Ok(callee) => {
                            self.stack.push(this_value);
                            self.stack.push(callee);
                        }
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::PrepareSuperMethodByValue => {
                    let key_value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let this_value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let base = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let result = (|| {
                        let key = self.coerce_to_property_key_runtime(key_value, realm, strict)?;
                        self.get_property_from_base_with_receiver(
                            base,
                            &key,
                            this_value.clone(),
                            realm,
                        )
                    })();
                    match result {
                        Ok(callee) => {
                            self.stack.push(this_value);
                            self.stack.push(callee);
                        }
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::DefineProperty(name) => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let receiver = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let object_id = match receiver {
                        JsValue::Object(id) => id,
                        _ => return Err(VmError::TypeError("property write expects object")),
                    };
                    let object = self
                        .objects
                        .get_mut(&object_id)
                        .ok_or(VmError::UnknownObject(object_id))?;
                    object.properties.insert(name.clone(), value);
                    object
                        .property_attributes
                        .entry(name.clone())
                        .or_insert_with(PropertyAttributes::default);
                    self.stack.push(JsValue::Object(object_id));
                }
                Opcode::DefineProtoProperty => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let receiver = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let object_id = match receiver {
                        JsValue::Object(id) => id,
                        _ => return Err(VmError::TypeError("property write expects object")),
                    };
                    let object = self
                        .objects
                        .get_mut(&object_id)
                        .ok_or(VmError::UnknownObject(object_id))?;
                    match value {
                        JsValue::Object(proto_id) => {
                            object.prototype = Some(proto_id);
                            object.prototype_value = None;
                        }
                        JsValue::Null => {
                            object.prototype = None;
                            object.prototype_value = None;
                        }
                        _ => {}
                    }
                    self.stack.push(JsValue::Object(object_id));
                }
                Opcode::DefineArrayLength => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let receiver = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let object_id = match receiver {
                        JsValue::Object(id) => id,
                        _ => return Err(VmError::TypeError("property write expects object")),
                    };
                    let object = self
                        .objects
                        .get_mut(&object_id)
                        .ok_or(VmError::UnknownObject(object_id))?;
                    object.properties.insert("length".to_string(), value);
                    object.property_attributes.insert(
                        "length".to_string(),
                        PropertyAttributes {
                            writable: true,
                            enumerable: false,
                            configurable: false,
                        },
                    );
                    self.stack.push(JsValue::Object(object_id));
                }
                Opcode::ArrayAppend => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let object_id = self.current_array_literal_target()?;
                    self.array_push_value(object_id, value)?;
                }
                Opcode::ArrayAppendSpread => {
                    let spread_source = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let object_id = self.current_array_literal_target()?;
                    let values = self.collect_spread_arguments(spread_source)?;
                    for value in values {
                        self.array_push_value(object_id, value)?;
                    }
                }
                Opcode::ArrayElision => {
                    let object_id = self.current_array_literal_target()?;
                    self.array_advance_length(object_id, 1)?;
                }
                Opcode::DefineGetter(name) => {
                    let getter = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let receiver = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let object_id = match receiver {
                        JsValue::Object(id) => id,
                        _ => return Err(VmError::TypeError("property write expects object")),
                    };
                    let object = self
                        .objects
                        .get_mut(&object_id)
                        .ok_or(VmError::UnknownObject(object_id))?;
                    object.getters.insert(name.clone(), getter);
                    object.property_attributes.insert(
                        name.clone(),
                        PropertyAttributes {
                            writable: false,
                            enumerable: true,
                            configurable: true,
                        },
                    );
                    self.stack.push(JsValue::Object(object_id));
                }
                Opcode::DefineSetter(name) => {
                    let setter = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let receiver = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let object_id = match receiver {
                        JsValue::Object(id) => id,
                        _ => return Err(VmError::TypeError("property write expects object")),
                    };
                    let object = self
                        .objects
                        .get_mut(&object_id)
                        .ok_or(VmError::UnknownObject(object_id))?;
                    object.setters.insert(name.clone(), setter);
                    object.property_attributes.insert(
                        name.clone(),
                        PropertyAttributes {
                            writable: false,
                            enumerable: true,
                            configurable: true,
                        },
                    );
                    self.stack.push(JsValue::Object(object_id));
                }
                Opcode::DefineGetterByValue => {
                    let getter = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let key = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let key = self.coerce_to_property_key(&key);
                    let receiver = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let object_id = match receiver {
                        JsValue::Object(id) => id,
                        _ => return Err(VmError::TypeError("property write expects object")),
                    };
                    let object = self
                        .objects
                        .get_mut(&object_id)
                        .ok_or(VmError::UnknownObject(object_id))?;
                    object.getters.insert(key.clone(), getter);
                    object.property_attributes.insert(
                        key,
                        PropertyAttributes {
                            writable: false,
                            enumerable: true,
                            configurable: true,
                        },
                    );
                    self.stack.push(JsValue::Object(object_id));
                }
                Opcode::DefineSetterByValue => {
                    let setter = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let key = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let key = self.coerce_to_property_key(&key);
                    let receiver = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let object_id = match receiver {
                        JsValue::Object(id) => id,
                        _ => return Err(VmError::TypeError("property write expects object")),
                    };
                    let object = self
                        .objects
                        .get_mut(&object_id)
                        .ok_or(VmError::UnknownObject(object_id))?;
                    object.setters.insert(key.clone(), setter);
                    object.property_attributes.insert(
                        key,
                        PropertyAttributes {
                            writable: false,
                            enumerable: true,
                            configurable: true,
                        },
                    );
                    self.stack.push(JsValue::Object(object_id));
                }
                Opcode::SetProperty(name) => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let receiver = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let result = match receiver {
                        JsValue::Object(object_id) => {
                            self.set_object_property(object_id, name.clone(), value, realm)
                        }
                        JsValue::Function(closure_id) => {
                            if self.function_rejects_caller_arguments(closure_id)?
                                && matches!(name.as_str(), "caller" | "arguments")
                                && !self.closure_has_own_property(closure_id, name)
                            {
                                Err(VmError::TypeError("restricted function property access"))
                            } else {
                                self.set_function_property(closure_id, name.clone(), value, realm)
                            }
                        }
                        JsValue::NativeFunction(_) | JsValue::HostFunction(_) => {
                            if Self::is_restricted_function_property(name) {
                                Err(VmError::TypeError("restricted function property access"))
                            } else {
                                Ok(value)
                            }
                        }
                        _ => Err(VmError::TypeError("property write expects object")),
                    };
                    match result {
                        Ok(result) => self.stack.push(result),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::SetPropertyByValue => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let key = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let key = self.coerce_to_property_key(&key);
                    let receiver = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let result = match receiver {
                        JsValue::Object(object_id) => {
                            self.set_object_property(object_id, key, value, realm)
                        }
                        JsValue::Function(closure_id) => {
                            if self.function_rejects_caller_arguments(closure_id)?
                                && matches!(key.as_str(), "caller" | "arguments")
                                && !self.closure_has_own_property(closure_id, &key)
                            {
                                Err(VmError::TypeError("restricted function property access"))
                            } else {
                                self.set_function_property(closure_id, key, value, realm)
                            }
                        }
                        JsValue::NativeFunction(_) | JsValue::HostFunction(_) => {
                            if Self::is_restricted_function_property(&key) {
                                Err(VmError::TypeError("restricted function property access"))
                            } else {
                                Ok(value)
                            }
                        }
                        _ => Err(VmError::TypeError("property write expects object")),
                    };
                    match result {
                        Ok(result) => self.stack.push(result),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::SetSuperProperty(name) => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let this_value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let base = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    match self.set_property_on_base_with_receiver(
                        base,
                        name.clone(),
                        value,
                        this_value,
                        realm,
                    ) {
                        Ok(result) => self.stack.push(result),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::SetSuperPropertyByValue => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let key_value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let this_value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let base = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let result = (|| {
                        let key = self.coerce_to_property_key_runtime(key_value, realm, strict)?;
                        self.set_property_on_base_with_receiver(base, key, value, this_value, realm)
                    })();
                    match result {
                        Ok(result) => self.stack.push(result),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::DeleteIdentifier(name) => {
                    let deleted = if let Some(reference) =
                        self.resolve_binding_or_with_reference(name, realm)?
                    {
                        match reference {
                            IdentifierReference::Binding { binding_id, .. } => {
                                self.delete_binding_reference(binding_id)
                            }
                            IdentifierReference::Property { base, property, .. } => {
                                self.delete_property(base, property)?
                            }
                            IdentifierReference::Unresolvable { .. } => true,
                        }
                    } else if let Some(global_object_id) = self.global_object_id {
                        let receiver = JsValue::Object(global_object_id);
                        if self.has_property_on_receiver(&receiver, name, realm)? {
                            self.delete_property(receiver, name.clone())?
                        } else {
                            true
                        }
                    } else {
                        true
                    };
                    self.stack.push(JsValue::Bool(deleted));
                }
                Opcode::DeleteProperty(name) => {
                    let receiver = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let deleted = self.delete_property(receiver, name.clone())?;
                    if strict && !deleted {
                        return Err(VmError::TypeError("cannot delete property"));
                    }
                    self.stack.push(JsValue::Bool(deleted));
                }
                Opcode::DeletePropertyByValue => {
                    let key = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let key = self.coerce_to_property_key(&key);
                    let receiver = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let deleted = self.delete_property(receiver, key)?;
                    if strict && !deleted {
                        return Err(VmError::TypeError("cannot delete property"));
                    }
                    self.stack.push(JsValue::Bool(deleted));
                }
                Opcode::DeleteSuperProperty => {
                    let target = self.route_runtime_error_to_handler(
                        VmError::UnknownIdentifier("super".to_string()),
                        code.len(),
                    )?;
                    pc = target;
                    continue;
                }
                Opcode::ResolveIdentifierReference(name) => {
                    let reference = self.resolve_identifier_reference(name, realm, strict)?;
                    self.identifier_references.push(reference);
                }
                Opcode::LoadReferenceValue => {
                    let reference = self
                        .identifier_references
                        .last()
                        .cloned()
                        .ok_or(VmError::StackUnderflow)?;
                    match self.load_identifier_reference_value(&reference, realm, strict) {
                        Ok(value) => self.stack.push(value),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::StoreReferenceValue => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let reference = self
                        .identifier_references
                        .pop()
                        .ok_or(VmError::StackUnderflow)?;
                    match self.store_identifier_reference_value(reference, value, realm, strict) {
                        Ok(result) => self.stack.push(result),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::EnterScope => self.scopes.push(Rc::new(RefCell::new(BTreeMap::new()))),
                Opcode::ExitScope => {
                    if self.scopes.pop().is_none() || self.scopes.is_empty() {
                        return Err(VmError::ScopeUnderflow);
                    }
                }
                Opcode::EnterParamInitScope => {
                    if self.scopes.len() < 2 {
                        return Err(VmError::ScopeUnderflow);
                    }
                    let body_scope = self.scopes.pop().ok_or(VmError::ScopeUnderflow)?;
                    self.param_init_body_scopes.push(body_scope);
                    if let Some(last_var_scope) = self.var_scope_stack.last_mut() {
                        *last_var_scope = self.scopes.len().saturating_sub(1);
                    }
                }
                Opcode::ExitParamInitScope => {
                    let body_scope = self
                        .param_init_body_scopes
                        .pop()
                        .ok_or(VmError::ScopeUnderflow)?;
                    self.scopes.push(body_scope);
                    if let Some(last_var_scope) = self.var_scope_stack.last_mut() {
                        *last_var_scope = self.scopes.len().saturating_sub(1);
                    }
                }
                Opcode::EnterWith => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    match self.with_object_from_value(value) {
                        Ok(object) => self.with_objects.push(WithFrame {
                            object,
                            scope_depth: self.scopes.len(),
                        }),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::ExitWith => {
                    if self.with_objects.pop().is_none() {
                        return Err(VmError::ScopeUnderflow);
                    }
                }
                Opcode::Add => match self.evaluate_add(realm, strict) {
                    Ok(result) => self.stack.push(result),
                    Err(err) => {
                        let target = self.route_runtime_error_to_handler(err, code.len())?;
                        pc = target;
                        continue;
                    }
                },
                Opcode::Sub => {
                    match self.eval_numeric_binary(realm, strict, |lhs, rhs| lhs - rhs) {
                        Ok(result) => self.stack.push(JsValue::Number(result)),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::Mul => {
                    match self.eval_numeric_binary(realm, strict, |lhs, rhs| lhs * rhs) {
                        Ok(result) => self.stack.push(JsValue::Number(result)),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::Div => {
                    match self.eval_numeric_binary(realm, strict, |lhs, rhs| lhs / rhs) {
                        Ok(result) => self.stack.push(JsValue::Number(result)),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::Mod => {
                    match self.eval_numeric_binary(realm, strict, |lhs, rhs| lhs % rhs) {
                        Ok(result) => self.stack.push(JsValue::Number(result)),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::Shl => {
                    let result = (|| -> Result<JsValue, VmError> {
                        let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                        let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                        // Align with QuickJS/ECMAScript: coerce left operand before right operand.
                        let lhs = self.coerce_int32_runtime(left, realm, strict)?;
                        let shift = self.coerce_uint32_runtime(right, realm, strict)? & 0x1F;
                        let result = lhs << shift;
                        Ok(JsValue::Number(result as f64))
                    })();
                    match result {
                        Ok(value) => self.stack.push(value),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::Shr => {
                    let result = (|| -> Result<JsValue, VmError> {
                        let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                        let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                        // Align with QuickJS/ECMAScript: coerce left operand before right operand.
                        let lhs = self.coerce_int32_runtime(left, realm, strict)?;
                        let shift = self.coerce_uint32_runtime(right, realm, strict)? & 0x1F;
                        let result = lhs >> shift;
                        Ok(JsValue::Number(result as f64))
                    })();
                    match result {
                        Ok(value) => self.stack.push(value),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::UShr => {
                    let result = (|| -> Result<JsValue, VmError> {
                        let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                        let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                        // Align with QuickJS/ECMAScript: coerce left operand before right operand.
                        let lhs = self.coerce_uint32_runtime(left, realm, strict)?;
                        let shift = self.coerce_uint32_runtime(right, realm, strict)? & 0x1F;
                        let result = lhs >> shift;
                        Ok(JsValue::Number(result as f64))
                    })();
                    match result {
                        Ok(value) => self.stack.push(value),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::BitAnd => {
                    let result = (|| -> Result<JsValue, VmError> {
                        let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                        let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                        let result = self.coerce_int32_runtime(left, realm, strict)?
                            & self.coerce_int32_runtime(right, realm, strict)?;
                        Ok(JsValue::Number(result as f64))
                    })();
                    match result {
                        Ok(value) => self.stack.push(value),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::BitOr => {
                    let result = (|| -> Result<JsValue, VmError> {
                        let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                        let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                        let result = self.coerce_int32_runtime(left, realm, strict)?
                            | self.coerce_int32_runtime(right, realm, strict)?;
                        Ok(JsValue::Number(result as f64))
                    })();
                    match result {
                        Ok(value) => self.stack.push(value),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::BitXor => {
                    let result = (|| -> Result<JsValue, VmError> {
                        let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                        let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                        let result = self.coerce_int32_runtime(left, realm, strict)?
                            ^ self.coerce_int32_runtime(right, realm, strict)?;
                        Ok(JsValue::Number(result as f64))
                    })();
                    match result {
                        Ok(value) => self.stack.push(value),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::Neg => {
                    let result = (|| -> Result<JsValue, VmError> {
                        let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                        let value = self.coerce_number_runtime(value, realm, strict)?;
                        Ok(JsValue::Number(-value))
                    })();
                    match result {
                        Ok(value) => self.stack.push(value),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::Not => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    self.stack.push(JsValue::Bool(!self.is_truthy(&value)));
                }
                Opcode::BitNot => {
                    let result = (|| -> Result<JsValue, VmError> {
                        let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                        let value = self.coerce_int32_runtime(value, realm, strict)?;
                        Ok(JsValue::Number((!value) as f64))
                    })();
                    match result {
                        Ok(value) => self.stack.push(value),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::Typeof => {
                    let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    self.stack
                        .push(JsValue::String(self.typeof_value(&value).to_string()));
                }
                Opcode::TypeofIdentifier(name) => {
                    let value = if let Some(binding_id) = self.resolve_binding_id(name) {
                        let binding = self
                            .bindings
                            .get(&binding_id)
                            .ok_or(VmError::ScopeUnderflow)?;
                        binding.value.clone()
                    } else if name == "undefined" {
                        JsValue::Undefined
                    } else if name == "NaN" {
                        JsValue::Number(f64::NAN)
                    } else if name == "Infinity" {
                        JsValue::Number(f64::INFINITY)
                    } else if name == "globalThis" {
                        self.global_this_value()
                    } else if name == "Math" {
                        self.math_object_value()?
                    } else if name == "Reflect" {
                        self.reflect_object_value()?
                    } else if let Some(value) = realm.resolve_identifier(name) {
                        value
                    } else if let Some(global_object_id) = self.global_object_id {
                        let receiver = JsValue::Object(global_object_id);
                        if self.has_property_on_receiver(&receiver, name, realm)? {
                            self.get_property_from_receiver(receiver, name, realm)?
                        } else {
                            JsValue::Undefined
                        }
                    } else {
                        JsValue::Undefined
                    };
                    self.stack
                        .push(JsValue::String(self.typeof_value(&value).to_string()));
                }
                Opcode::ToNumber => {
                    let result = (|| -> Result<JsValue, VmError> {
                        let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                        let number = self.coerce_number_runtime(value, realm, strict)?;
                        Ok(JsValue::Number(number))
                    })();
                    match result {
                        Ok(value) => self.stack.push(value),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::ToPropertyKey => {
                    let result = (|| -> Result<JsValue, VmError> {
                        let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                        let key = self.coerce_to_property_key_runtime(value, realm, strict)?;
                        Ok(JsValue::String(key))
                    })();
                    match result {
                        Ok(value) => self.stack.push(value),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::Eq => {
                    let result = (|| -> Result<JsValue, VmError> {
                        let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                        let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                        let equal = self.abstract_equality_compare(left, right, realm, strict)?;
                        Ok(JsValue::Bool(equal))
                    })();
                    match result {
                        Ok(value) => self.stack.push(value),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::Ne => {
                    let result = (|| -> Result<JsValue, VmError> {
                        let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                        let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                        let equal = self.abstract_equality_compare(left, right, realm, strict)?;
                        Ok(JsValue::Bool(!equal))
                    })();
                    match result {
                        Ok(value) => self.stack.push(value),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::StrictEq => {
                    let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let equal = self.strict_equality_compare(&left, &right);
                    self.stack.push(JsValue::Bool(equal));
                }
                Opcode::StrictNe => {
                    let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let equal = self.strict_equality_compare(&left, &right);
                    self.stack.push(JsValue::Bool(!equal));
                }
                Opcode::Lt => match self.eval_relational_operator(realm, strict, Opcode::Lt) {
                    Ok(result) => self.stack.push(JsValue::Bool(result)),
                    Err(err) => {
                        let target = self.route_runtime_error_to_handler(err, code.len())?;
                        pc = target;
                        continue;
                    }
                },
                Opcode::Le => match self.eval_relational_operator(realm, strict, Opcode::Le) {
                    Ok(result) => self.stack.push(JsValue::Bool(result)),
                    Err(err) => {
                        let target = self.route_runtime_error_to_handler(err, code.len())?;
                        pc = target;
                        continue;
                    }
                },
                Opcode::Gt => match self.eval_relational_operator(realm, strict, Opcode::Gt) {
                    Ok(result) => self.stack.push(JsValue::Bool(result)),
                    Err(err) => {
                        let target = self.route_runtime_error_to_handler(err, code.len())?;
                        pc = target;
                        continue;
                    }
                },
                Opcode::Ge => match self.eval_relational_operator(realm, strict, Opcode::Ge) {
                    Ok(result) => self.stack.push(JsValue::Bool(result)),
                    Err(err) => {
                        let target = self.route_runtime_error_to_handler(err, code.len())?;
                        pc = target;
                        continue;
                    }
                },
                Opcode::In => {
                    let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let key = self.coerce_to_property_key(&left);
                    match self.evaluate_in_operator(key, right, realm) {
                        Ok(result) => self.stack.push(JsValue::Bool(result)),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::InstanceOf => {
                    let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    match self.evaluate_instanceof_operator(left, right, realm) {
                        Ok(result) => self.stack.push(JsValue::Bool(result)),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::JumpIfFalse(target) => {
                    let condition = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    if !self.is_truthy(&condition) {
                        if *target >= code.len() {
                            return Err(VmError::InvalidJump(*target));
                        }
                        pc = *target;
                        continue;
                    }
                }
                Opcode::Jump(target) => {
                    if *target >= code.len() {
                        return Err(VmError::InvalidJump(*target));
                    }
                    pc = *target;
                    continue;
                }
                Opcode::PushExceptionHandler {
                    catch_target,
                    finally_target,
                } => {
                    self.exception_handlers.push(ExceptionHandler {
                        catch_target: *catch_target,
                        finally_target: *finally_target,
                        scope_depth: self.scopes.len(),
                        stack_depth: self.stack.len(),
                        with_depth: self.with_objects.len(),
                    });
                }
                Opcode::PopExceptionHandler => {
                    self.exception_handlers
                        .pop()
                        .ok_or(VmError::HandlerUnderflow)?;
                }
                Opcode::LoadException => {
                    let exception = self
                        .pending_exception
                        .take()
                        .ok_or(VmError::NoPendingException)?;
                    self.stack.push(exception);
                }
                Opcode::RethrowIfException => {
                    if let Some(exception) = self.pending_exception.take() {
                        let target = self.throw_to_handler(exception, code.len())?;
                        pc = target;
                        continue;
                    }
                }
                Opcode::Throw => {
                    let exception = self.stack.pop().ok_or(VmError::StackUnderflow)?;
                    let target = self.throw_to_handler(exception, code.len())?;
                    pc = target;
                    continue;
                }
                Opcode::Call(arg_count) => match self.execute_call(*arg_count, realm, strict) {
                    Ok(result) => self.stack.push(result),
                    Err(err) => {
                        let target = self.route_runtime_error_to_handler(err, code.len())?;
                        pc = target;
                        continue;
                    }
                },
                Opcode::CallWithSpread(spread_flags) => {
                    match self.execute_call_with_spread(spread_flags, realm, strict) {
                        Ok(result) => self.stack.push(result),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::CallIdentifier { name, arg_count } => {
                    match self.execute_identifier_call(name, *arg_count, realm, strict) {
                        Ok(result) => self.stack.push(result),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::CallIdentifierWithSpread { name, spread_flags } => {
                    match self.execute_identifier_call_with_spread(
                        name,
                        spread_flags,
                        realm,
                        strict,
                    ) {
                        Ok(result) => self.stack.push(result),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::CallMethod(arg_count) => {
                    match self.execute_method_call(*arg_count, realm, strict) {
                        Ok(result) => self.stack.push(result),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::CallMethodWithSpread(spread_flags) => {
                    match self.execute_method_call_with_spread(spread_flags, realm, strict) {
                        Ok(result) => self.stack.push(result),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::Construct(arg_count) => {
                    match self.execute_construct(*arg_count, realm, strict) {
                        Ok(result) => self.stack.push(result),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::ConstructWithSpread(spread_flags) => {
                    match self.execute_construct_with_spread(spread_flags, realm, strict) {
                        Ok(result) => self.stack.push(result),
                        Err(err) => {
                            let target = self.route_runtime_error_to_handler(err, code.len())?;
                            pc = target;
                            continue;
                        }
                    }
                }
                Opcode::Return => {
                    if !allow_return {
                        return Err(VmError::TopLevelReturn);
                    }
                    return Ok(ExecutionSignal::Return);
                }
                Opcode::MarkStrict => {}
                Opcode::Dup => {
                    let value = self.stack.last().cloned().ok_or(VmError::StackUnderflow)?;
                    self.stack.push(value);
                }
                Opcode::Dup2 => {
                    let len = self.stack.len();
                    if len < 2 {
                        return Err(VmError::StackUnderflow);
                    }
                    let first = self.stack[len - 2].clone();
                    let second = self.stack[len - 1].clone();
                    self.stack.push(first);
                    self.stack.push(second);
                }
                Opcode::Dup3 => {
                    let len = self.stack.len();
                    if len < 3 {
                        return Err(VmError::StackUnderflow);
                    }
                    let first = self.stack[len - 3].clone();
                    let second = self.stack[len - 2].clone();
                    let third = self.stack[len - 1].clone();
                    self.stack.push(first);
                    self.stack.push(second);
                    self.stack.push(third);
                }
                Opcode::Swap => {
                    let len = self.stack.len();
                    if len < 2 {
                        return Err(VmError::StackUnderflow);
                    }
                    self.stack.swap(len - 1, len - 2);
                }
                Opcode::RotRight4 => {
                    let len = self.stack.len();
                    if len < 4 {
                        return Err(VmError::StackUnderflow);
                    }
                    let top = self.stack.remove(len - 1);
                    self.stack.insert(len - 4, top);
                }
                Opcode::RotRight5 => {
                    let len = self.stack.len();
                    if len < 5 {
                        return Err(VmError::StackUnderflow);
                    }
                    let top = self.stack.remove(len - 1);
                    self.stack.insert(len - 5, top);
                }
                Opcode::Pop => {
                    self.stack.pop().ok_or(VmError::StackUnderflow)?;
                }
                Opcode::Nop => {}
                Opcode::Halt => return Ok(ExecutionSignal::Halt),
            }

            pc += 1;
        }

        Ok(ExecutionSignal::Halt)
    }

    fn execute_call(
        &mut self,
        arg_count: usize,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        let args = self.pop_call_arguments(arg_count)?;
        let callee = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        self.execute_callable(callee, None, args, realm, caller_strict)
    }

    fn execute_call_with_spread(
        &mut self,
        spread_flags: &[bool],
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        let raw_args = self.pop_call_arguments(spread_flags.len())?;
        let args = self.expand_spread_arguments(raw_args, spread_flags)?;
        let callee = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        self.execute_callable(callee, None, args, realm, caller_strict)
    }

    fn execute_identifier_call(
        &mut self,
        name: &str,
        arg_count: usize,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        let args = self.pop_call_arguments(arg_count)?;
        let reference = self.resolve_identifier_reference(name, realm, caller_strict)?;
        let callee = self.load_identifier_reference_value(&reference, realm, caller_strict)?;
        if name == "super" {
            return self.execute_super_constructor_call(callee, args, realm, caller_strict);
        }
        if name == "eval" && matches!(callee, JsValue::NativeFunction(NativeFunction::Eval)) {
            return self.execute_eval_argument(
                args.first(),
                realm,
                caller_strict,
                EvalCallKind::Direct,
            );
        }
        let this_arg = match reference {
            IdentifierReference::Property {
                base,
                with_base_object: true,
                ..
            } => Some(base),
            _ => None,
        };
        self.execute_callable(callee, this_arg, args, realm, caller_strict)
    }

    fn execute_identifier_call_with_spread(
        &mut self,
        name: &str,
        spread_flags: &[bool],
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        let raw_args = self.pop_call_arguments(spread_flags.len())?;
        let args = self.expand_spread_arguments(raw_args, spread_flags)?;
        let reference = self.resolve_identifier_reference(name, realm, caller_strict)?;
        let callee = self.load_identifier_reference_value(&reference, realm, caller_strict)?;
        if name == "super" {
            return self.execute_super_constructor_call(callee, args, realm, caller_strict);
        }
        if name == "eval" && matches!(callee, JsValue::NativeFunction(NativeFunction::Eval)) {
            return self.execute_eval_argument(
                args.first(),
                realm,
                caller_strict,
                EvalCallKind::Direct,
            );
        }
        let this_arg = match reference {
            IdentifierReference::Property {
                base,
                with_base_object: true,
                ..
            } => Some(base),
            _ => None,
        };
        self.execute_callable(callee, this_arg, args, realm, caller_strict)
    }

    fn execute_method_call(
        &mut self,
        arg_count: usize,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        let args = self.pop_call_arguments(arg_count)?;
        let callee = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        let this_arg = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        self.execute_callable(callee, Some(this_arg), args, realm, caller_strict)
    }

    fn execute_method_call_with_spread(
        &mut self,
        spread_flags: &[bool],
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        let raw_args = self.pop_call_arguments(spread_flags.len())?;
        let args = self.expand_spread_arguments(raw_args, spread_flags)?;
        let callee = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        let this_arg = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        self.execute_callable(callee, Some(this_arg), args, realm, caller_strict)
    }

    fn execute_construct(
        &mut self,
        arg_count: usize,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        let args = self.pop_call_arguments(arg_count)?;
        let callee = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        self.execute_construct_value(callee, args, realm, caller_strict)
    }

    fn execute_construct_with_spread(
        &mut self,
        spread_flags: &[bool],
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        let raw_args = self.pop_call_arguments(spread_flags.len())?;
        let args = self.expand_spread_arguments(raw_args, spread_flags)?;
        let callee = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        self.execute_construct_value(callee, args, realm, caller_strict)
    }

    fn execute_construct_value(
        &mut self,
        callee: JsValue,
        args: Vec<JsValue>,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        match callee {
            JsValue::Function(closure_id) => {
                if self.closure_is_arrow(closure_id)? || self.closure_has_no_prototype(closure_id) {
                    return Err(VmError::NotCallable);
                }
                let constructed = self.create_object_value();
                let prototype = self.get_or_create_function_prototype_property(closure_id)?;
                if let Ok((prototype, prototype_value)) = self.parse_prototype_value(prototype) {
                    self.apply_prototype_components_to_value(
                        &constructed,
                        prototype,
                        prototype_value,
                    );
                }
                let result =
                    self.execute_closure_call(closure_id, args, Some(constructed.clone()), realm)?;
                if Self::is_object_like_value(&result) {
                    Ok(result)
                } else {
                    Ok(constructed)
                }
            }
            JsValue::Object(object_id) => {
                if self.is_class_constructor_object(object_id) {
                    self.construct_from_class_object(object_id, realm)
                } else {
                    Err(VmError::NotCallable)
                }
            }
            JsValue::NativeFunction(native) => {
                if matches!(native, NativeFunction::SymbolConstructor) {
                    return Err(VmError::NotCallable);
                }
                if matches!(native, NativeFunction::DateConstructor) {
                    return self.execute_date_constructor(&args);
                }
                if matches!(
                    native,
                    NativeFunction::NumberConstructor
                        | NativeFunction::BooleanConstructor
                        | NativeFunction::StringConstructor
                ) {
                    return self.execute_boxed_primitive_constructor(
                        native,
                        args,
                        realm,
                        caller_strict,
                    );
                }
                self.execute_native_call(native, args, realm, caller_strict)
            }
            JsValue::HostFunction(host_id) => {
                if let Some(HostFunction::BoundCall {
                    target,
                    this_arg: _,
                    mut bound_args,
                }) = self.host_functions.get(&host_id).cloned()
                {
                    bound_args.extend(args);
                    return self.execute_construct_value(target, bound_args, realm, caller_strict);
                }
                let constructed = self.create_object_value();
                self.install_constructor_property(&constructed, JsValue::HostFunction(host_id));
                let result =
                    self.execute_host_function_call(host_id, None, args, realm, caller_strict)?;
                if Self::is_object_like_value(&result) {
                    Ok(result)
                } else {
                    Ok(constructed)
                }
            }
            _ => Err(VmError::NotCallable),
        }
    }

    fn execute_super_constructor_call(
        &mut self,
        callee: JsValue,
        args: Vec<JsValue>,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        let this_binding_id = self
            .resolve_binding_id("this")
            .ok_or_else(|| VmError::UnknownIdentifier("this".to_string()))?;
        let this_value = self
            .bindings
            .get(&this_binding_id)
            .map(|binding| binding.value.clone())
            .ok_or(VmError::ScopeUnderflow)?;
        let already_initialized = !matches!(this_value, JsValue::Uninitialized);

        let pending_this = self
            .resolve_binding_id(DERIVED_THIS_BINDING)
            .and_then(|binding_id| {
                self.bindings
                    .get(&binding_id)
                    .map(|binding| binding.value.clone())
            })
            .unwrap_or(JsValue::Undefined);
        let super_this = if already_initialized {
            this_value.clone()
        } else {
            pending_this.clone()
        };
        let super_prototype_hint = self.prototype_components_of_value(&super_this);

        let initialized_this = match callee {
            JsValue::Function(closure_id) => {
                if self.closure_is_arrow(closure_id)? || self.closure_has_no_prototype(closure_id) {
                    return Err(VmError::NotCallable);
                }
                let result =
                    self.execute_closure_call(closure_id, args, Some(super_this.clone()), realm)?;
                if Self::is_object_like_value(&result) {
                    result
                } else {
                    super_this.clone()
                }
            }
            JsValue::Object(object_id) => {
                if self.is_class_constructor_object(object_id) {
                    self.construct_from_class_object(object_id, realm)?
                } else {
                    return Err(VmError::NotCallable);
                }
            }
            JsValue::NativeFunction(native) => {
                if matches!(native, NativeFunction::SymbolConstructor) {
                    return Err(VmError::NotCallable);
                }
                let result = if matches!(native, NativeFunction::DateConstructor) {
                    self.execute_date_constructor(&args)?
                } else if matches!(
                    native,
                    NativeFunction::NumberConstructor
                        | NativeFunction::BooleanConstructor
                        | NativeFunction::StringConstructor
                ) {
                    self.execute_boxed_primitive_constructor(native, args, realm, caller_strict)?
                } else {
                    self.execute_native_call(native, args, realm, caller_strict)?
                };
                if Self::is_object_like_value(&result) {
                    if let Some((prototype, prototype_value)) = super_prototype_hint.clone() {
                        self.apply_prototype_components_to_value(
                            &result,
                            prototype,
                            prototype_value,
                        );
                    }
                    result
                } else {
                    super_this.clone()
                }
            }
            JsValue::HostFunction(host_id) => {
                let result =
                    self.execute_host_function_call(host_id, None, args, realm, caller_strict)?;
                if Self::is_object_like_value(&result) {
                    if let Some((prototype, prototype_value)) = super_prototype_hint {
                        self.apply_prototype_components_to_value(
                            &result,
                            prototype,
                            prototype_value,
                        );
                    }
                    result
                } else {
                    super_this.clone()
                }
            }
            _ => return Err(VmError::NotCallable),
        };

        if already_initialized {
            return Err(VmError::UnknownIdentifier("this".to_string()));
        }

        if !Self::is_object_like_value(&initialized_this) {
            return Err(VmError::TypeError(
                "super constructor did not initialize this",
            ));
        }

        let this_binding = self
            .bindings
            .get_mut(&this_binding_id)
            .ok_or(VmError::ScopeUnderflow)?;
        this_binding.value = initialized_this.clone();
        Ok(initialized_this)
    }

    fn pop_call_arguments(&mut self, arg_count: usize) -> Result<Vec<JsValue>, VmError> {
        let mut args = Vec::with_capacity(arg_count);
        for _ in 0..arg_count {
            let value = self.stack.pop().ok_or(VmError::StackUnderflow)?;
            args.push(value);
        }
        args.reverse();
        Ok(args)
    }

    fn is_class_constructor_object(&self, object_id: ObjectId) -> bool {
        self.objects
            .get(&object_id)
            .and_then(|object| object.properties.get(CLASS_CONSTRUCTOR_MARKER))
            .is_some_and(|marker| matches!(marker, JsValue::Bool(true)))
    }

    fn construct_from_class_object(
        &mut self,
        class_object_id: ObjectId,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let prototype = self.get_object_property(class_object_id, "prototype", realm)?;
        let instance = self.create_object_value();
        let JsValue::Object(instance_id) = instance else {
            unreachable!();
        };

        if let JsValue::Object(prototype_id) = prototype {
            let prototype_object = self
                .objects
                .get(&prototype_id)
                .cloned()
                .ok_or(VmError::UnknownObject(prototype_id))?;
            let instance_object = self
                .objects
                .get_mut(&instance_id)
                .ok_or(VmError::UnknownObject(instance_id))?;
            for (key, value) in prototype_object.properties {
                instance_object.properties.insert(key, value);
            }
            for (key, value) in prototype_object.getters {
                instance_object.getters.insert(key, value);
            }
            for (key, value) in prototype_object.setters {
                instance_object.setters.insert(key, value);
            }
            for (key, attributes) in prototype_object.property_attributes {
                instance_object.property_attributes.insert(key, attributes);
            }
        }

        Ok(JsValue::Object(instance_id))
    }

    fn expand_spread_arguments(
        &self,
        args: Vec<JsValue>,
        spread_flags: &[bool],
    ) -> Result<Vec<JsValue>, VmError> {
        if args.len() != spread_flags.len() {
            return Err(VmError::TypeError("spread argument metadata mismatch"));
        }
        let mut expanded = Vec::new();
        for (arg, is_spread) in args.into_iter().zip(spread_flags.iter().copied()) {
            if is_spread {
                expanded.extend(self.collect_spread_arguments(arg)?);
            } else {
                expanded.push(arg);
            }
        }
        Ok(expanded)
    }

    fn collect_spread_arguments(&self, spread_arg: JsValue) -> Result<Vec<JsValue>, VmError> {
        match spread_arg {
            JsValue::Object(object_id) => {
                let object = self
                    .objects
                    .get(&object_id)
                    .ok_or(VmError::UnknownObject(object_id))?;
                let length = object
                    .properties
                    .get("length")
                    .map(|value| self.to_number(value))
                    .unwrap_or(0.0)
                    .max(0.0) as usize;
                let mut values = Vec::with_capacity(length);
                for index in 0..length {
                    let key = index.to_string();
                    values.push(
                        object
                            .properties
                            .get(&key)
                            .cloned()
                            .unwrap_or(JsValue::Undefined),
                    );
                }
                Ok(values)
            }
            JsValue::String(value) => Ok(self.js_string_iterator_values(&value)),
            _ => Err(VmError::TypeError("spread expects array-like object")),
        }
    }

    fn install_constructor_property(&mut self, target: &JsValue, constructor: JsValue) {
        let JsValue::Object(object_id) = target else {
            return;
        };
        if let Some(object) = self.objects.get_mut(object_id) {
            object
                .properties
                .insert("constructor".to_string(), constructor);
        }
    }

    fn execute_callable(
        &mut self,
        callee: JsValue,
        this_arg: Option<JsValue>,
        args: Vec<JsValue>,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        match callee {
            JsValue::NativeFunction(native) => {
                self.execute_native_call(native, args, realm, caller_strict)
            }
            JsValue::HostFunction(host_id) => {
                self.execute_host_function_call(host_id, this_arg, args, realm, caller_strict)
            }
            JsValue::Function(closure_id) => {
                if self.closure_is_class_constructor(closure_id) {
                    return Err(VmError::TypeError(
                        "class constructor cannot be invoked without 'new'",
                    ));
                }
                if self.closure_is_generator(closure_id)? {
                    if !self.closure_uses_yield_identifier(closure_id)? {
                        return self.execute_closure_call(closure_id, args, this_arg, realm);
                    }
                    return self
                        .create_generator_iterator_from_closure_call(closure_id, args, this_arg);
                }
                self.execute_closure_call(closure_id, args, this_arg, realm)
            }
            _ => Err(VmError::NotCallable),
        }
    }

    fn execute_closure_call(
        &mut self,
        closure_id: u64,
        args: Vec<JsValue>,
        this_arg: Option<JsValue>,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        self.execute_closure_call_internal(closure_id, args, this_arg, realm, None)
    }

    fn execute_closure_call_with_generator_resume(
        &mut self,
        closure_id: u64,
        args: Vec<JsValue>,
        this_arg: Option<JsValue>,
        realm: &Realm,
        resume_value: JsValue,
    ) -> Result<JsValue, VmError> {
        self.execute_closure_call_internal(closure_id, args, this_arg, realm, Some(resume_value))
    }

    fn execute_closure_call_internal(
        &mut self,
        closure_id: u64,
        args: Vec<JsValue>,
        this_arg: Option<JsValue>,
        realm: &Realm,
        generator_resume: Option<JsValue>,
    ) -> Result<JsValue, VmError> {
        let closure = self
            .closures
            .get(&closure_id)
            .cloned()
            .ok_or(VmError::UnknownClosure(closure_id))?;
        let function = closure
            .functions
            .get(closure.function_id)
            .cloned()
            .ok_or(VmError::UnknownFunction(closure.function_id))?;
        let is_arrow_function = self.function_is_arrow(&function);
        let is_derived_class_constructor = self.closure_is_derived_class_constructor(closure_id);
        let restrict_caller_arguments =
            closure.strict || self.closure_marks_restricted_caller_arguments(closure_id);
        let non_simple_params = self.function_has_non_simple_params(&function);
        let has_arguments_param = function.params.iter().any(|name| name == "arguments");
        let mapped_arguments_enabled =
            !restrict_caller_arguments && !is_arrow_function && !non_simple_params;
        let rest_param_index = self.function_rest_param_index(&function);
        let is_async_function = self.function_is_async(&function);

        let mut frame_scope: Scope = BTreeMap::new();
        let mut param_binding_ids = Vec::with_capacity(function.params.len());
        for (index, param_name) in function.params.iter().enumerate() {
            let value = if rest_param_index == Some(index) {
                let rest_values = if index < args.len() {
                    args[index..].to_vec()
                } else {
                    Vec::new()
                };
                self.create_array_from_values(rest_values)?
            } else {
                args.get(index).cloned().unwrap_or(JsValue::Undefined)
            };
            let binding_id = self.create_binding(value, true);
            frame_scope.insert(param_name.clone(), binding_id);
            param_binding_ids.push(binding_id);
        }
        if let Some(resume_value) = generator_resume {
            let yield_binding_id = self.create_binding(resume_value, true);
            frame_scope.insert("yield".to_string(), yield_binding_id);
        }
        if !is_arrow_function {
            if is_derived_class_constructor {
                let derived_this_value = this_arg.unwrap_or(JsValue::Undefined);
                let derived_this_binding_id = self.create_binding(derived_this_value, true);
                frame_scope.insert(DERIVED_THIS_BINDING.to_string(), derived_this_binding_id);
                let this_binding_id = self.create_binding(JsValue::Uninitialized, true);
                frame_scope.insert("this".to_string(), this_binding_id);
            } else {
                let this_value = if closure.strict {
                    this_arg.unwrap_or(JsValue::Undefined)
                } else {
                    self.coerce_this_for_sloppy(this_arg)
                };
                let this_binding_id = self.create_binding(this_value, true);
                frame_scope.insert("this".to_string(), this_binding_id);
            }
            if !has_arguments_param {
                let arguments_value = self.create_object_value();
                let arguments_id = match arguments_value {
                    JsValue::Object(id) => id,
                    _ => unreachable!(),
                };
                let strict_arguments_callee = restrict_caller_arguments;
                let throw_type_error = if strict_arguments_callee {
                    Some(self.create_host_function_value(HostFunction::ThrowTypeError))
                } else {
                    None
                };
                {
                    let object = self
                        .objects
                        .get_mut(&arguments_id)
                        .ok_or(VmError::UnknownObject(arguments_id))?;
                    object
                        .properties
                        .insert("length".to_string(), JsValue::Number(args.len() as f64));
                    if let Some(thrower) = throw_type_error {
                        object.getters.insert("callee".to_string(), thrower.clone());
                        object.setters.insert("callee".to_string(), thrower);
                    } else {
                        object
                            .properties
                            .insert("callee".to_string(), JsValue::Function(closure_id));
                    }
                    object.properties.insert(
                        "constructor".to_string(),
                        JsValue::NativeFunction(NativeFunction::ObjectConstructor),
                    );
                    for (index, arg) in args.iter().enumerate() {
                        let key = index.to_string();
                        object.properties.insert(key.clone(), arg.clone());
                        object
                            .property_attributes
                            .entry(key.clone())
                            .or_insert_with(PropertyAttributes::default);
                        if mapped_arguments_enabled {
                            if let Some(binding_id) = param_binding_ids.get(index) {
                                object.argument_mappings.insert(key, *binding_id);
                            }
                        }
                    }
                    object.property_attributes.insert(
                        "length".to_string(),
                        PropertyAttributes {
                            writable: true,
                            enumerable: false,
                            configurable: true,
                        },
                    );
                    object.property_attributes.insert(
                        "callee".to_string(),
                        PropertyAttributes {
                            writable: !strict_arguments_callee,
                            enumerable: false,
                            configurable: !strict_arguments_callee,
                        },
                    );
                }
                let arguments_binding_id = self.create_binding(arguments_value, true);
                frame_scope.insert("arguments".to_string(), arguments_binding_id);
            }
        }

        let saved_stack = std::mem::take(&mut self.stack);
        let saved_scopes = std::mem::take(&mut self.scopes);
        let saved_with_objects = std::mem::take(&mut self.with_objects);
        let saved_handlers = std::mem::take(&mut self.exception_handlers);
        let saved_pending_exception = self.pending_exception.take();
        let saved_identifier_references = std::mem::take(&mut self.identifier_references);
        let saved_var_scope_stack = std::mem::take(&mut self.var_scope_stack);
        let saved_param_init_body_scopes = std::mem::take(&mut self.param_init_body_scopes);
        self.gc_shadow_roots.push(GcShadowRoots {
            stack: saved_stack,
            scopes: saved_scopes,
            with_objects: saved_with_objects,
            pending_exception: saved_pending_exception,
            identifier_references: saved_identifier_references,
        });

        self.scopes = closure.captured_scopes;
        self.with_objects = closure.captured_with_objects;
        self.scopes.push(Rc::new(RefCell::new(frame_scope)));
        if non_simple_params {
            self.scopes.push(Rc::new(RefCell::new(BTreeMap::new())));
        }
        self.var_scope_stack = vec![self.scopes.len().saturating_sub(1)];
        self.stack = Vec::new();
        self.exception_handlers = Vec::new();
        self.pending_exception = None;
        self.identifier_references = Vec::new();
        self.param_init_body_scopes = Vec::new();
        self.eval_contexts.push(EvalContext {
            is_arrow_function,
            non_simple_params,
            has_arguments_param,
        });

        let signal = self.execute_code(
            &function.code,
            closure.functions.as_ref(),
            realm,
            true,
            closure.strict,
        );
        let _ = self.eval_contexts.pop();
        let mut async_rejection: Option<JsValue> = None;
        let mut value = match signal {
            Ok(ExecutionSignal::Return) => self.stack.pop().unwrap_or(JsValue::Undefined),
            Ok(ExecutionSignal::Halt) => JsValue::Undefined,
            Err(err) => {
                if is_async_function {
                    let rejection = match &err {
                        VmError::UncaughtException(exception) => Some(exception.clone()),
                        other => self.runtime_error_exception_value(&other),
                    };
                    if let Some(rejection) = rejection {
                        async_rejection = Some(rejection);
                        JsValue::Undefined
                    } else {
                        self.restore_caller_state(
                            saved_handlers,
                            saved_var_scope_stack,
                            saved_param_init_body_scopes,
                        );
                        return Err(err);
                    }
                } else {
                    self.restore_caller_state(
                        saved_handlers,
                        saved_var_scope_stack,
                        saved_param_init_body_scopes,
                    );
                    return Err(err);
                }
            }
        };

        if is_derived_class_constructor {
            if matches!(value, JsValue::Undefined) {
                let this_binding_id = match self.resolve_binding_id("this") {
                    Some(binding_id) => binding_id,
                    None => {
                        self.restore_caller_state(
                            saved_handlers,
                            saved_var_scope_stack,
                            saved_param_init_body_scopes,
                        );
                        return Err(VmError::UnknownIdentifier("this".to_string()));
                    }
                };
                let this_value = match self.bindings.get(&this_binding_id) {
                    Some(binding) => binding.value.clone(),
                    None => {
                        self.restore_caller_state(
                            saved_handlers,
                            saved_var_scope_stack,
                            saved_param_init_body_scopes,
                        );
                        return Err(VmError::ScopeUnderflow);
                    }
                };
                if matches!(this_value, JsValue::Uninitialized) {
                    self.restore_caller_state(
                        saved_handlers,
                        saved_var_scope_stack,
                        saved_param_init_body_scopes,
                    );
                    return Err(VmError::UnknownIdentifier("this".to_string()));
                }
                value = this_value;
            } else if !Self::is_object_like_value(&value) {
                self.restore_caller_state(
                    saved_handlers,
                    saved_var_scope_stack,
                    saved_param_init_body_scopes,
                );
                return Err(VmError::TypeError(
                    "derived class constructor must return object or undefined",
                ));
            }
        }

        if is_async_function {
            let promise = if let Some(rejection) = async_rejection {
                self.create_async_settled_promise(false, rejection)?
            } else {
                self.create_async_settled_promise(true, value)?
            };
            self.restore_caller_state(
                saved_handlers,
                saved_var_scope_stack,
                saved_param_init_body_scopes,
            );
            return Ok(promise);
        }

        self.restore_caller_state(
            saved_handlers,
            saved_var_scope_stack,
            saved_param_init_body_scopes,
        );
        Ok(value)
    }

    fn restore_caller_state(
        &mut self,
        saved_handlers: Vec<ExceptionHandler>,
        saved_var_scope_stack: Vec<usize>,
        saved_param_init_body_scopes: Vec<ScopeRef>,
    ) {
        let saved = self
            .gc_shadow_roots
            .pop()
            .expect("caller state shadow roots should be present");
        self.stack = saved.stack;
        self.scopes = saved.scopes;
        self.with_objects = saved.with_objects;
        self.exception_handlers = saved_handlers;
        self.pending_exception = saved.pending_exception;
        self.identifier_references = saved.identifier_references;
        self.var_scope_stack = saved_var_scope_stack;
        self.param_init_body_scopes = saved_param_init_body_scopes;
    }

    fn strict_this_string(&self, this_arg: Option<JsValue>) -> Result<String, VmError> {
        let value = this_arg.unwrap_or(JsValue::Undefined);
        match value {
            JsValue::String(value) => Ok(value),
            JsValue::Object(object_id) => match self.boxed_primitive_value(object_id) {
                Some(JsValue::String(value)) => Ok(value),
                _ => Err(VmError::TypeError(
                    "String.prototype method called on incompatible",
                )),
            },
            _ => Err(VmError::TypeError(
                "String.prototype method called on incompatible",
            )),
        }
    }

    fn coerce_this_string(&self, this_arg: Option<JsValue>) -> Result<String, VmError> {
        let value = this_arg.unwrap_or(JsValue::Undefined);
        match value {
            JsValue::Null | JsValue::Undefined => Err(VmError::TypeError(
                "String method called on null or undefined",
            )),
            JsValue::String(value) => Ok(value),
            JsValue::Object(object_id) => {
                if let Some(JsValue::String(value)) = self.boxed_primitive_value(object_id) {
                    Ok(value)
                } else {
                    Ok(self.coerce_to_string(&JsValue::Object(object_id)))
                }
            }
            other => Ok(self.coerce_to_string(&other)),
        }
    }

    fn strict_this_number(&self, this_arg: Option<JsValue>) -> Result<f64, VmError> {
        let value = this_arg.unwrap_or(JsValue::Undefined);
        match value {
            JsValue::Number(value) => Ok(value),
            JsValue::Object(object_id) => match self.boxed_primitive_value(object_id) {
                Some(JsValue::Number(value)) => Ok(value),
                _ => Err(VmError::TypeError(
                    "Number.prototype method called on incompatible",
                )),
            },
            _ => Err(VmError::TypeError(
                "Number.prototype method called on incompatible",
            )),
        }
    }

    fn strict_this_boolean(&self, this_arg: Option<JsValue>) -> Result<bool, VmError> {
        let value = this_arg.unwrap_or(JsValue::Undefined);
        match value {
            JsValue::Bool(value) => Ok(value),
            JsValue::Object(object_id) => match self.boxed_primitive_value(object_id) {
                Some(JsValue::Bool(value)) => Ok(value),
                _ => Err(VmError::TypeError(
                    "Boolean.prototype method called on incompatible",
                )),
            },
            _ => Err(VmError::TypeError(
                "Boolean.prototype method called on incompatible",
            )),
        }
    }

    fn has_object_marker(&self, object_id: ObjectId, marker: &str) -> Result<bool, VmError> {
        let object = self
            .objects
            .get(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        Ok(matches!(
            object.properties.get(marker),
            Some(JsValue::Bool(true))
        ))
    }

    fn strict_this_array_buffer(&self, this_arg: Option<JsValue>) -> Result<ObjectId, VmError> {
        match this_arg.unwrap_or(JsValue::Undefined) {
            JsValue::Object(object_id)
                if self.has_object_marker(object_id, "__arrayBufferTag")? =>
            {
                Ok(object_id)
            }
            _ => Err(VmError::TypeError(
                "ArrayBuffer.prototype method called on incompatible",
            )),
        }
    }

    fn strict_this_map(&self, this_arg: Option<JsValue>) -> Result<ObjectId, VmError> {
        match this_arg.unwrap_or(JsValue::Undefined) {
            JsValue::Object(object_id) if self.has_object_marker(object_id, "__mapTag")? => {
                Ok(object_id)
            }
            _ => Err(VmError::TypeError(
                "Map.prototype method called on incompatible",
            )),
        }
    }

    fn strict_this_set(&self, this_arg: Option<JsValue>) -> Result<ObjectId, VmError> {
        match this_arg.unwrap_or(JsValue::Undefined) {
            JsValue::Object(object_id) if self.has_object_marker(object_id, "__setTag")? => {
                Ok(object_id)
            }
            _ => Err(VmError::TypeError(
                "Set.prototype method called on incompatible",
            )),
        }
    }

    fn strict_this_regexp_object(&self, this_arg: Option<JsValue>) -> Result<ObjectId, VmError> {
        match this_arg.unwrap_or(JsValue::Undefined) {
            JsValue::Object(object_id) if self.has_object_marker(object_id, "__regexpTag")? => {
                Ok(object_id)
            }
            _ => Err(VmError::TypeError(
                "RegExp.prototype method called on incompatible",
            )),
        }
    }

    fn array_buffer_length(&self, object_id: ObjectId) -> Result<usize, VmError> {
        let object = self
            .objects
            .get(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        let length = object
            .properties
            .get("byteLength")
            .map(|value| self.to_number(value))
            .unwrap_or(0.0)
            .max(0.0);
        Ok(length as usize)
    }

    fn strict_this_date_object(&self, this_arg: Option<JsValue>) -> Result<ObjectId, VmError> {
        match this_arg.unwrap_or(JsValue::Undefined) {
            JsValue::Object(object_id) if self.is_date_object(object_id) => Ok(object_id),
            _ => Err(VmError::TypeError(
                "Date.prototype method called on incompatible",
            )),
        }
    }

    fn date_parts_for_object(
        &self,
        object_id: ObjectId,
        utc: bool,
    ) -> Result<(i32, u32, u32), VmError> {
        let object = self
            .objects
            .get(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        if !utc {
            let year = object
                .properties
                .get("__dateYear")
                .map(|value| self.to_number(value) as i32);
            let month = object
                .properties
                .get("__dateMonth")
                .map(|value| self.to_number(value) as u32);
            let day = object
                .properties
                .get("__dateDay")
                .map(|value| self.to_number(value) as u32);
            if let (Some(year), Some(month), Some(day)) = (year, month, day) {
                return Ok((year, month, day));
            }
        }
        let timestamp = object
            .properties
            .get("value")
            .map(|value| self.to_number(value))
            .unwrap_or(0.0);
        Ok(Self::utc_date_parts_from_timestamp(timestamp))
    }

    fn utc_date_parts_from_timestamp(timestamp_ms: f64) -> (i32, u32, u32) {
        let millis_per_day = 86_400_000.0;
        let days = (timestamp_ms / millis_per_day).floor() as i64;
        Self::civil_from_days(days)
    }

    // Howard Hinnant's civil-from-days algorithm.
    fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
        let z = days_since_epoch + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = mp + if mp < 10 { 3 } else { -9 };
        let year = y + if month <= 2 { 1 } else { 0 };
        (year as i32, (month as u32).saturating_sub(1), day as u32)
    }

    fn execute_string_replace(
        &mut self,
        receiver: String,
        args: &[JsValue],
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        let search_value = args
            .first()
            .map_or(String::new(), |value| self.coerce_to_string(value));
        let replacement = match args.get(1) {
            Some(JsValue::Function(_))
            | Some(JsValue::NativeFunction(_))
            | Some(JsValue::HostFunction(_)) => {
                let callback = args[1].clone();
                let callback_result = self.execute_callable(
                    callback,
                    Some(JsValue::Undefined),
                    vec![JsValue::String(search_value.clone())],
                    realm,
                    caller_strict,
                )?;
                self.coerce_to_string(&callback_result)
            }
            Some(value) => self.coerce_to_string(value),
            None => "undefined".to_string(),
        };
        if let Some(index) = receiver.find(&search_value) {
            let mut output = String::new();
            output.push_str(&receiver[..index]);
            output.push_str(&replacement);
            output.push_str(&receiver[index + search_value.len()..]);
            Ok(JsValue::String(output))
        } else {
            Ok(JsValue::String(receiver))
        }
    }

    fn execute_string_index_of(&self, receiver: String, args: &[JsValue]) -> JsValue {
        let search_value = args.first().map_or_else(
            || "undefined".to_string(),
            |value| self.coerce_to_string(value),
        );
        let start_index = args
            .get(1)
            .map(|value| self.to_number(value))
            .filter(|value| value.is_finite() && *value > 0.0)
            .map_or(0usize, |value| value as usize);
        let receiver_chars: Vec<char> = receiver.chars().collect();
        let search_chars: Vec<char> = search_value.chars().collect();
        let bounded_start = start_index.min(receiver_chars.len());
        if search_chars.is_empty() {
            return JsValue::Number(bounded_start as f64);
        }
        if search_chars.len() > receiver_chars.len().saturating_sub(bounded_start) {
            return JsValue::Number(-1.0);
        }
        for index in bounded_start..=receiver_chars.len() - search_chars.len() {
            if receiver_chars[index..index + search_chars.len()] == search_chars[..] {
                return JsValue::Number(index as f64);
            }
        }
        JsValue::Number(-1.0)
    }

    fn execute_string_split(
        &mut self,
        receiver: String,
        args: &[JsValue],
    ) -> Result<JsValue, VmError> {
        let separator = args.first().cloned().unwrap_or(JsValue::Undefined);
        let limit = args
            .get(1)
            .map(|value| self.to_number(value))
            .unwrap_or(f64::INFINITY);
        if limit <= 0.0 {
            return self.create_array_from_values(Vec::new());
        }
        let mut parts: Vec<JsValue> = if matches!(separator, JsValue::Undefined) {
            vec![JsValue::String(receiver)]
        } else {
            let sep = self.coerce_to_string(&separator);
            if sep.is_empty() {
                receiver
                    .chars()
                    .map(|ch| JsValue::String(ch.to_string()))
                    .collect()
            } else {
                receiver
                    .split(&sep)
                    .map(|part| JsValue::String(part.to_string()))
                    .collect()
            }
        };
        if limit.is_finite() {
            let cap = limit.max(0.0) as usize;
            if parts.len() > cap {
                parts.truncate(cap);
            }
        }
        self.create_array_from_values(parts)
    }

    fn execute_host_function_call(
        &mut self,
        host_id: u64,
        this_arg: Option<JsValue>,
        args: Vec<JsValue>,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        let host = self
            .host_functions
            .get(&host_id)
            .cloned()
            .ok_or(VmError::UnknownHostFunction(host_id))?;
        match host {
            HostFunction::BoundMethod { target, method } => match method {
                FunctionMethod::Call => {
                    let this_arg = args.first().cloned().unwrap_or(JsValue::Undefined);
                    let call_args = args.get(1..).map_or_else(Vec::new, |slice| slice.to_vec());
                    self.execute_callable(target, Some(this_arg), call_args, realm, caller_strict)
                }
                FunctionMethod::Apply => {
                    let this_arg = args.first().cloned().unwrap_or(JsValue::Undefined);
                    let call_args = self.collect_apply_arguments(args.get(1))?;
                    self.execute_callable(target, Some(this_arg), call_args, realm, caller_strict)
                }
                FunctionMethod::Bind => {
                    let this_arg = args.first().cloned().unwrap_or(JsValue::Undefined);
                    let bound_args = args.get(1..).map_or_else(Vec::new, |slice| slice.to_vec());
                    Ok(self.create_host_function_value(HostFunction::BoundCall {
                        target,
                        this_arg,
                        bound_args,
                    }))
                }
            },
            HostFunction::BoundCall {
                target,
                this_arg,
                mut bound_args,
            } => {
                bound_args.extend(args);
                self.execute_callable(target, Some(this_arg), bound_args, realm, caller_strict)
            }
            HostFunction::GeneratorFactory { producer } => {
                let produced_values = self.execute_callable(
                    producer,
                    Some(JsValue::Undefined),
                    args,
                    realm,
                    caller_strict,
                )?;
                self.create_generator_iterator_from_values(produced_values)
            }
            HostFunction::GeneratorIteratorNextThis => {
                self.execute_generator_iterator_next(this_arg, args.first().cloned(), realm)
            }
            HostFunction::StringReplaceThis => {
                let receiver = self.coerce_this_string(this_arg)?;
                self.execute_string_replace(receiver, &args, realm, caller_strict)
            }
            HostFunction::StringMatchThis => {
                let receiver = self.coerce_this_string(this_arg)?;
                let matcher = args.first().cloned().unwrap_or(JsValue::Undefined);
                let exec = self.get_property_from_receiver(matcher.clone(), "exec", realm)?;
                if !Self::is_callable_value(&exec) {
                    return Err(VmError::NotCallable);
                }
                self.execute_callable(
                    exec,
                    Some(matcher),
                    vec![JsValue::String(receiver)],
                    realm,
                    caller_strict,
                )
            }
            HostFunction::StringSearchThis => {
                let receiver = self.coerce_this_string(this_arg)?;
                let matcher = args.first().cloned().unwrap_or(JsValue::Undefined);
                let test = self.get_property_from_receiver(matcher.clone(), "test", realm)?;
                if !Self::is_callable_value(&test) {
                    return Err(VmError::NotCallable);
                }
                let matched = self.execute_callable(
                    test,
                    Some(matcher),
                    vec![JsValue::String(receiver)],
                    realm,
                    caller_strict,
                )?;
                Ok(JsValue::Number(if self.is_truthy(&matched) {
                    0.0
                } else {
                    -1.0
                }))
            }
            HostFunction::StringIndexOfThis => {
                let receiver = self.coerce_this_string(this_arg)?;
                Ok(self.execute_string_index_of(receiver, &args))
            }
            HostFunction::StringSplitThis => {
                let receiver = self.coerce_this_string(this_arg)?;
                self.execute_string_split(receiver, &args)
            }
            HostFunction::StringToLowerCaseThis => {
                let receiver = self.coerce_this_string(this_arg)?;
                Ok(JsValue::String(receiver.to_lowercase()))
            }
            HostFunction::StringToUpperCase => {
                let receiver = self.coerce_this_string(this_arg)?;
                Ok(JsValue::String(receiver.to_uppercase()))
            }
            HostFunction::StringTrim => {
                let receiver = self.coerce_this_string(this_arg)?;
                Ok(JsValue::String(receiver.trim().to_string()))
            }
            HostFunction::StringToString | HostFunction::StringValueOf => {
                let receiver = self.strict_this_string(this_arg)?;
                Ok(JsValue::String(receiver))
            }
            HostFunction::StringCharAt => {
                let receiver = self.coerce_this_string(this_arg)?;
                let receiver_chars: Vec<char> = receiver.chars().collect();
                let index = args
                    .first()
                    .map(|value| self.to_number(value))
                    .unwrap_or(0.0);
                let index = if index.is_finite() && index >= 0.0 {
                    index as usize
                } else {
                    0
                };
                Ok(receiver_chars
                    .get(index)
                    .map(|ch| JsValue::String(ch.to_string()))
                    .unwrap_or_else(|| JsValue::String(String::new())))
            }
            HostFunction::StringCharCodeAt => {
                let receiver = self.coerce_this_string(this_arg)?;
                let index = args
                    .first()
                    .map(|value| self.to_number(value))
                    .unwrap_or(0.0);
                let index = if index.is_finite() && index >= 0.0 {
                    index as usize
                } else {
                    0
                };
                Ok(Self::utf16_code_unit_at(&receiver, index)
                    .map(|unit| JsValue::Number(unit as f64))
                    .unwrap_or(JsValue::Number(f64::NAN)))
            }
            HostFunction::StringLastIndexOf => {
                let receiver = self.coerce_this_string(this_arg)?;
                let search_value = args.first().map_or_else(
                    || "undefined".to_string(),
                    |value| self.coerce_to_string(value),
                );
                if search_value.is_empty() {
                    return Ok(JsValue::Number(receiver.chars().count() as f64));
                }
                let start_index = args
                    .get(1)
                    .map(|value| self.to_number(value))
                    .filter(|value| value.is_finite() && *value >= 0.0)
                    .map(|value| value as usize)
                    .unwrap_or_else(|| receiver.chars().count());
                let receiver_chars: Vec<char> = receiver.chars().collect();
                let search_chars: Vec<char> = search_value.chars().collect();
                if search_chars.len() > receiver_chars.len() {
                    return Ok(JsValue::Number(-1.0));
                }
                let start =
                    start_index.min(receiver_chars.len().saturating_sub(search_chars.len()));
                for index in (0..=start).rev() {
                    if receiver_chars[index..index + search_chars.len()] == search_chars[..] {
                        return Ok(JsValue::Number(index as f64));
                    }
                }
                Ok(JsValue::Number(-1.0))
            }
            HostFunction::StringSubstring => {
                let receiver = self.coerce_this_string(this_arg)?;
                let chars: Vec<char> = receiver.chars().collect();
                let len = chars.len();
                let start = args
                    .first()
                    .map(|value| self.to_number(value))
                    .unwrap_or(0.0)
                    .max(0.0)
                    .min(len as f64) as usize;
                let end = args
                    .get(1)
                    .map(|value| self.to_number(value))
                    .unwrap_or(len as f64)
                    .max(0.0)
                    .min(len as f64) as usize;
                let (from, to) = if start <= end {
                    (start, end)
                } else {
                    (end, start)
                };
                Ok(JsValue::String(chars[from..to].iter().collect()))
            }
            HostFunction::NumberToString => {
                let number = self.strict_this_number(this_arg)?;
                Ok(JsValue::String(Self::coerce_number_to_string(number)))
            }
            HostFunction::NumberValueOf => {
                let number = self.strict_this_number(this_arg)?;
                Ok(JsValue::Number(number))
            }
            HostFunction::NumberToFixed => {
                let number = self.strict_this_number(this_arg)?;
                let digits = args
                    .first()
                    .map(|value| self.to_number(value))
                    .unwrap_or(0.0);
                let digits = if digits.is_finite() && digits >= 0.0 {
                    (digits as usize).min(100)
                } else {
                    0
                };
                if number.is_nan() {
                    return Ok(JsValue::String("NaN".to_string()));
                }
                if number.is_infinite() {
                    return Ok(JsValue::String(if number.is_sign_negative() {
                        "-Infinity".to_string()
                    } else {
                        "Infinity".to_string()
                    }));
                }
                Ok(JsValue::String(format!(
                    "{number:.precision$}",
                    precision = digits
                )))
            }
            HostFunction::NumberToExponential => {
                let number = self.strict_this_number(this_arg)?;
                if number.is_nan() {
                    return Ok(JsValue::String("NaN".to_string()));
                }
                if number.is_infinite() {
                    return Ok(JsValue::String(if number.is_sign_negative() {
                        "-Infinity".to_string()
                    } else {
                        "Infinity".to_string()
                    }));
                }
                let precision = match args.first() {
                    None | Some(JsValue::Undefined) => None,
                    Some(value) => {
                        let digits = self.to_number(value);
                        if !digits.is_finite() || digits < 0.0 || digits > 100.0 {
                            return Err(VmError::UncaughtException(self.create_error_exception(
                                NativeFunction::RangeErrorConstructor,
                                "RangeError",
                                "invalid number of digits".to_string(),
                            )));
                        }
                        Some(digits as usize)
                    }
                };
                let raw = if let Some(precision) = precision {
                    format!("{number:.precision$e}", precision = precision)
                } else {
                    format!("{number:e}")
                };
                Ok(JsValue::String(Self::normalize_exponent_string(raw)))
            }
            HostFunction::ArrayBufferSliceThis => {
                let receiver_id = self.strict_this_array_buffer(this_arg)?;
                let byte_length = self.array_buffer_length(receiver_id)?;
                let start = args
                    .first()
                    .map(|value| self.to_number(value))
                    .filter(|value| value.is_finite() && *value >= 0.0)
                    .map(|value| value as usize)
                    .unwrap_or(0)
                    .min(byte_length);
                let end = args
                    .get(1)
                    .map(|value| self.to_number(value))
                    .filter(|value| value.is_finite() && *value >= 0.0)
                    .map(|value| value as usize)
                    .unwrap_or(byte_length)
                    .min(byte_length);
                let sliced_length = end.saturating_sub(start) as f64;
                let constructor = self.get_property_from_receiver(
                    JsValue::Object(receiver_id),
                    "constructor",
                    realm,
                )?;
                self.execute_construct_value(
                    constructor,
                    vec![JsValue::Number(sliced_length)],
                    realm,
                    caller_strict,
                )
            }
            HostFunction::MapSetThis => {
                let receiver_id = self.strict_this_map(this_arg)?;
                let size = self
                    .objects
                    .get(&receiver_id)
                    .and_then(|object| object.properties.get("__mapSize"))
                    .map(|value| self.to_number(value))
                    .unwrap_or(0.0)
                    + 1.0;
                let object = self
                    .objects
                    .get_mut(&receiver_id)
                    .ok_or(VmError::UnknownObject(receiver_id))?;
                object
                    .properties
                    .insert("__mapSize".to_string(), JsValue::Number(size));
                object
                    .properties
                    .insert("size".to_string(), JsValue::Number(size));
                Ok(JsValue::Object(receiver_id))
            }
            HostFunction::SetAddThis => {
                let receiver_id = self.strict_this_set(this_arg)?;
                let size = self
                    .objects
                    .get(&receiver_id)
                    .and_then(|object| object.properties.get("__setSize"))
                    .map(|value| self.to_number(value))
                    .unwrap_or(0.0)
                    + 1.0;
                let object = self
                    .objects
                    .get_mut(&receiver_id)
                    .ok_or(VmError::UnknownObject(receiver_id))?;
                object
                    .properties
                    .insert("__setSize".to_string(), JsValue::Number(size));
                object
                    .properties
                    .insert("size".to_string(), JsValue::Number(size));
                Ok(JsValue::Object(receiver_id))
            }
            HostFunction::BooleanToString => {
                let value = self.strict_this_boolean(this_arg)?;
                Ok(JsValue::String(
                    if value { "true" } else { "false" }.to_string(),
                ))
            }
            HostFunction::BooleanValueOf => {
                let value = self.strict_this_boolean(this_arg)?;
                Ok(JsValue::Bool(value))
            }
            HostFunction::FunctionPrototype => Ok(JsValue::Undefined),
            HostFunction::JsonStringify => Ok(self.execute_json_stringify(args.first())),
            HostFunction::JsonParse => Ok(self.execute_json_parse(args.first())),
            HostFunction::ArrayPush(object_id) => {
                let mut length = self
                    .objects
                    .get(&object_id)
                    .and_then(|object| object.properties.get("length"))
                    .map(|value| self.to_number(value))
                    .unwrap_or(0.0)
                    .max(0.0) as usize;
                let object = self
                    .objects
                    .get_mut(&object_id)
                    .ok_or(VmError::UnknownObject(object_id))?;
                for arg in args {
                    let key = length.to_string();
                    object.properties.insert(key.clone(), arg);
                    object
                        .property_attributes
                        .entry(key)
                        .or_insert_with(PropertyAttributes::default);
                    length += 1;
                }
                object
                    .properties
                    .insert("length".to_string(), JsValue::Number(length as f64));
                object
                    .property_attributes
                    .entry("length".to_string())
                    .or_insert(PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: false,
                    });
                Ok(JsValue::Number(length as f64))
            }
            HostFunction::ArrayForEach(object_id) => {
                let callback = args.first().cloned().unwrap_or(JsValue::Undefined);
                if !Self::is_callable_value(&callback) {
                    return Err(VmError::NotCallable);
                }
                let callback_this = args.get(1).cloned().unwrap_or(JsValue::Undefined);
                let (length, indexed_values) = {
                    let object = self
                        .objects
                        .get(&object_id)
                        .ok_or(VmError::UnknownObject(object_id))?;
                    let length = object
                        .properties
                        .get("length")
                        .map(|value| self.to_number(value))
                        .unwrap_or(0.0)
                        .max(0.0) as usize;
                    let indexed_values = (0..length)
                        .map(|index| {
                            let key = index.to_string();
                            object
                                .properties
                                .get(&key)
                                .cloned()
                                .map(|value| (index, value))
                        })
                        .collect::<Vec<_>>();
                    (length, indexed_values)
                };
                for maybe_entry in indexed_values.into_iter().take(length) {
                    let Some((index, value)) = maybe_entry else {
                        continue;
                    };
                    let _ = self.execute_callable(
                        callback.clone(),
                        Some(callback_this.clone()),
                        vec![
                            value,
                            JsValue::Number(index as f64),
                            JsValue::Object(object_id),
                        ],
                        realm,
                        caller_strict,
                    )?;
                }
                Ok(JsValue::Undefined)
            }
            HostFunction::ArrayReduce(object_id) => {
                let callback = args.first().cloned().unwrap_or(JsValue::Undefined);
                if !Self::is_callable_value(&callback) {
                    return Err(VmError::NotCallable);
                }
                let length = self
                    .objects
                    .get(&object_id)
                    .map(|object| {
                        object
                            .properties
                            .get("length")
                            .map(|value| self.to_number(value))
                            .unwrap_or(0.0)
                            .max(0.0) as usize
                    })
                    .ok_or(VmError::UnknownObject(object_id))?;
                let mut index = 0usize;
                let mut accumulator = if args.len() >= 2 {
                    args[1].clone()
                } else {
                    let mut initial = None;
                    while index < length {
                        let key = index.to_string();
                        let value = self
                            .objects
                            .get(&object_id)
                            .and_then(|object| object.properties.get(&key).cloned());
                        if let Some(value) = value {
                            initial = Some(value);
                            index += 1;
                            break;
                        }
                        index += 1;
                    }
                    initial.ok_or(VmError::TypeError(
                        "Array.prototype.reduce of empty array with no initial value",
                    ))?
                };
                while index < length {
                    let key = index.to_string();
                    if let Some(value) = self
                        .objects
                        .get(&object_id)
                        .and_then(|object| object.properties.get(&key).cloned())
                    {
                        accumulator = self.execute_callable(
                            callback.clone(),
                            Some(JsValue::Undefined),
                            vec![
                                accumulator,
                                value,
                                JsValue::Number(index as f64),
                                JsValue::Object(object_id),
                            ],
                            realm,
                            caller_strict,
                        )?;
                    }
                    index += 1;
                }
                Ok(accumulator)
            }
            HostFunction::ArrayJoin(object_id) => {
                let separator = match args.first() {
                    None | Some(JsValue::Undefined) => ",".to_string(),
                    Some(value) => self.coerce_to_string(value),
                };
                self.execute_array_join(object_id, &separator)
            }
            HostFunction::ArrayJoinThis => {
                let receiver_id = match this_arg {
                    Some(JsValue::Object(id)) => id,
                    _ => {
                        return Err(VmError::TypeError(
                            "Array.prototype.join receiver must be object",
                        ));
                    }
                };
                let separator = match args.first() {
                    None | Some(JsValue::Undefined) => ",".to_string(),
                    Some(value) => self.coerce_to_string(value),
                };
                self.execute_array_join(receiver_id, &separator)
            }
            HostFunction::ArrayPopThis => {
                let receiver_id = match this_arg {
                    Some(JsValue::Object(id)) => id,
                    _ => {
                        return Err(VmError::TypeError(
                            "Array.prototype.pop receiver must be object",
                        ));
                    }
                };
                let length = self
                    .objects
                    .get(&receiver_id)
                    .and_then(|object| object.properties.get("length").cloned())
                    .map(|value| self.to_number(&value))
                    .unwrap_or(0.0)
                    .max(0.0) as usize;
                let value = {
                    let object = self
                        .objects
                        .get_mut(&receiver_id)
                        .ok_or(VmError::UnknownObject(receiver_id))?;
                    if length == 0 {
                        object
                            .properties
                            .insert("length".to_string(), JsValue::Number(0.0));
                        JsValue::Undefined
                    } else {
                        let index = length - 1;
                        let key = index.to_string();
                        let value = object.properties.remove(&key).unwrap_or(JsValue::Undefined);
                        object
                            .properties
                            .insert("length".to_string(), JsValue::Number(index as f64));
                        value
                    }
                };
                Ok(value)
            }
            HostFunction::ArrayConcatThis => {
                let receiver_id = match this_arg {
                    Some(JsValue::Object(id)) => id,
                    _ => {
                        return Err(VmError::TypeError(
                            "Array.prototype.concat receiver must be object",
                        ));
                    }
                };
                let mut concatenated = Vec::new();
                for candidate in std::iter::once(JsValue::Object(receiver_id)).chain(args) {
                    if let JsValue::Object(object_id) = candidate.clone() {
                        let maybe_elements = self.objects.get(&object_id).and_then(|object| {
                            if !object.properties.contains_key("length") {
                                return None;
                            }
                            let length = object
                                .properties
                                .get("length")
                                .map(|value| self.to_number(value))
                                .unwrap_or(0.0)
                                .max(0.0) as usize;
                            Some(
                                (0..length)
                                    .map(|index| {
                                        object
                                            .properties
                                            .get(&index.to_string())
                                            .cloned()
                                            .unwrap_or(JsValue::Undefined)
                                    })
                                    .collect::<Vec<_>>(),
                            )
                        });
                        if let Some(elements) = maybe_elements {
                            concatenated.extend(elements);
                            continue;
                        }
                    }
                    concatenated.push(candidate);
                }
                self.create_array_from_values(concatenated)
            }
            HostFunction::ArrayKeysThis => self.create_array_iterator_from_this(this_arg, "keys"),
            HostFunction::ArrayEntriesThis => {
                self.create_array_iterator_from_this(this_arg, "entries")
            }
            HostFunction::ArrayValuesThis => {
                self.create_array_iterator_from_this(this_arg, "values")
            }
            HostFunction::ArrayIteratorNextThis => {
                self.execute_array_iterator_next(this_arg, realm)
            }
            HostFunction::ArrayReverse(object_id) => self.execute_array_reverse(object_id),
            HostFunction::ArraySort(object_id) => {
                self.execute_array_sort(object_id, &args, realm, caller_strict)
            }
            HostFunction::RegExpTestThis => {
                let receiver_id = self.strict_this_regexp_object(this_arg)?;
                let input = args.first().map_or_else(
                    || "undefined".to_string(),
                    |value| self.coerce_to_string(value),
                );
                let matched = self.execute_regexp_test(receiver_id, &input)?;
                Ok(JsValue::Bool(matched))
            }
            HostFunction::RegExpExecThis => {
                let receiver_id = self.strict_this_regexp_object(this_arg)?;
                let input = args.first().map_or_else(
                    || "undefined".to_string(),
                    |value| self.coerce_to_string(value),
                );
                let matched = self.execute_regexp_test(receiver_id, &input)?;
                if !matched {
                    return Ok(JsValue::Null);
                }
                self.create_array_from_values(vec![JsValue::String(input)])
            }
            HostFunction::RegExpToStringThis => {
                let receiver_id = self.strict_this_regexp_object(this_arg)?;
                self.execute_regexp_to_string(receiver_id)
                    .map(JsValue::String)
            }
            HostFunction::HasOwnProperty { target } => {
                let target = this_arg.unwrap_or(target);
                let key = args
                    .first()
                    .map(|value| self.coerce_to_property_key(value))
                    .unwrap_or_default();
                Ok(JsValue::Bool(self.has_own_property(&target, &key)?))
            }
            HostFunction::IsPrototypeOf { target } => {
                let target = this_arg.unwrap_or(target);
                let value = args.first().cloned().unwrap_or(JsValue::Undefined);
                Ok(JsValue::Bool(self.object_is_prototype_of(target, value)?))
            }
            HostFunction::ObjectToString => {
                let tag = match this_arg {
                    Some(JsValue::Object(object_id))
                        if self.has_object_marker(object_id, "__uint8ArrayTag")? =>
                    {
                        "[object Uint8Array]"
                    }
                    _ => "[object Object]",
                };
                Ok(JsValue::String(tag.to_string()))
            }
            HostFunction::ObjectValueOf => Ok(this_arg.unwrap_or(JsValue::Undefined)),
            HostFunction::ErrorToStringThis => {
                let receiver = this_arg.unwrap_or(JsValue::Undefined);
                if !Self::is_object_like_value(&receiver) {
                    return Err(VmError::TypeError(
                        "Error.prototype.toString called on non-object",
                    ));
                }
                let name_value =
                    self.get_property_from_receiver(receiver.clone(), "name", realm)?;
                let message_value =
                    self.get_property_from_receiver(receiver.clone(), "message", realm)?;
                let name = if matches!(name_value, JsValue::Undefined) {
                    "Error".to_string()
                } else {
                    self.coerce_to_string(&name_value)
                };
                let message = if matches!(message_value, JsValue::Undefined) {
                    String::new()
                } else {
                    self.coerce_to_string(&message_value)
                };
                if name.is_empty() {
                    return Ok(JsValue::String(message));
                }
                if message.is_empty() {
                    return Ok(JsValue::String(name));
                }
                Ok(JsValue::String(format!("{name}: {message}")))
            }
            HostFunction::DateToString(object_id) => {
                let timestamp = self
                    .objects
                    .get(&object_id)
                    .and_then(|object| object.properties.get("value"))
                    .map(|value| self.to_number(value))
                    .unwrap_or(0.0);
                Ok(JsValue::String(format!(
                    "Date({})",
                    Self::coerce_number_to_string(timestamp)
                )))
            }
            HostFunction::DateValueOf(object_id) => {
                let timestamp = self
                    .objects
                    .get(&object_id)
                    .and_then(|object| object.properties.get("value"))
                    .map(|value| self.to_number(value))
                    .unwrap_or(0.0);
                Ok(JsValue::Number(timestamp))
            }
            HostFunction::DateGetFullYearThis => {
                let object_id = self.strict_this_date_object(this_arg)?;
                let (year, _, _) = self.date_parts_for_object(object_id, false)?;
                Ok(JsValue::Number(year as f64))
            }
            HostFunction::DateGetMonthThis => {
                let object_id = self.strict_this_date_object(this_arg)?;
                let (_, month, _) = self.date_parts_for_object(object_id, false)?;
                Ok(JsValue::Number(month as f64))
            }
            HostFunction::DateGetDateThis => {
                let object_id = self.strict_this_date_object(this_arg)?;
                let (_, _, day) = self.date_parts_for_object(object_id, false)?;
                Ok(JsValue::Number(day as f64))
            }
            HostFunction::DateGetUTCFullYearThis => {
                let object_id = self.strict_this_date_object(this_arg)?;
                let (year, _, _) = self.date_parts_for_object(object_id, true)?;
                Ok(JsValue::Number(year as f64))
            }
            HostFunction::DateGetUTCMonthThis => {
                let object_id = self.strict_this_date_object(this_arg)?;
                let (_, month, _) = self.date_parts_for_object(object_id, true)?;
                Ok(JsValue::Number(month as f64))
            }
            HostFunction::DateGetUTCDateThis => {
                let object_id = self.strict_this_date_object(this_arg)?;
                let (_, _, day) = self.date_parts_for_object(object_id, true)?;
                Ok(JsValue::Number(day as f64))
            }
            HostFunction::FunctionToString { target } => Ok(JsValue::String(format!(
                "{}() {{ [native code] }}",
                self.function_name_for_display(&target)
            ))),
            HostFunction::FunctionValueOf { target } => Ok(target),
            HostFunction::AssertSameValue => {
                let left = args.first().cloned().unwrap_or(JsValue::Undefined);
                let right = args.get(1).cloned().unwrap_or(JsValue::Undefined);
                if self.same_value(&left, &right) {
                    Ok(JsValue::Undefined)
                } else {
                    let detail = if args.len() >= 3 {
                        self.coerce_to_string(&args[2])
                    } else {
                        format!(
                            "Expected SameValue, got left={} right={}",
                            self.coerce_to_string(&left),
                            self.coerce_to_string(&right)
                        )
                    };
                    Err(self.assertion_failure(&detail))
                }
            }
            HostFunction::AssertNotSameValue => {
                let left = args.first().cloned().unwrap_or(JsValue::Undefined);
                let right = args.get(1).cloned().unwrap_or(JsValue::Undefined);
                if !self.same_value(&left, &right) {
                    Ok(JsValue::Undefined)
                } else {
                    let detail = if args.len() >= 3 {
                        self.coerce_to_string(&args[2])
                    } else {
                        format!(
                            "Expected values to differ but both were {}",
                            self.coerce_to_string(&left)
                        )
                    };
                    Err(self.assertion_failure(&detail))
                }
            }
            HostFunction::AssertThrows => {
                let callback = args.get(1).cloned().unwrap_or(JsValue::Undefined);
                match self.execute_callable(
                    callback,
                    Some(JsValue::Undefined),
                    Vec::new(),
                    realm,
                    caller_strict,
                ) {
                    Ok(_) => {
                        Err(self.assertion_failure("assert.throws expected callback to throw"))
                    }
                    Err(_) => Ok(JsValue::Undefined),
                }
            }
            HostFunction::AssertCompareArray => {
                let actual = args.first().cloned().unwrap_or(JsValue::Undefined);
                let expected = args.get(1).cloned().unwrap_or(JsValue::Undefined);
                if self.compare_array_like(&actual, &expected)? {
                    Ok(JsValue::Undefined)
                } else {
                    let detail = if args.len() >= 3 {
                        self.coerce_to_string(&args[2])
                    } else {
                        "assert.compareArray failed".to_string()
                    };
                    Err(self.assertion_failure(&detail))
                }
            }
            HostFunction::ThrowTypeError => {
                Err(VmError::TypeError("restricted function property access"))
            }
        }
    }

    fn collect_apply_arguments(
        &self,
        apply_args: Option<&JsValue>,
    ) -> Result<Vec<JsValue>, VmError> {
        let Some(apply_args) = apply_args else {
            return Ok(Vec::new());
        };
        match apply_args {
            JsValue::Null | JsValue::Undefined => Ok(Vec::new()),
            JsValue::Object(object_id) => {
                let object = self
                    .objects
                    .get(object_id)
                    .ok_or(VmError::UnknownObject(*object_id))?;
                let length = object
                    .properties
                    .get("length")
                    .map(|value| self.to_number(value))
                    .unwrap_or(0.0)
                    .max(0.0) as usize;
                let mut args = Vec::with_capacity(length);
                for index in 0..length {
                    let key = index.to_string();
                    args.push(
                        object
                            .properties
                            .get(&key)
                            .cloned()
                            .unwrap_or(JsValue::Undefined),
                    );
                }
                Ok(args)
            }
            _ => Ok(Vec::new()),
        }
    }

    fn execute_native_call(
        &mut self,
        native: NativeFunction,
        args: Vec<JsValue>,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        match native {
            NativeFunction::Eval => self.execute_eval_argument(
                args.first(),
                realm,
                caller_strict,
                EvalCallKind::Indirect,
            ),
            NativeFunction::FunctionConstructor => self.execute_function_constructor(&args, realm),
            NativeFunction::GeneratorFunctionConstructor => {
                self.execute_generator_function_constructor(&args, realm)
            }
            NativeFunction::ObjectConstructor => Ok(self.execute_object_constructor(&args)),
            NativeFunction::ArrayConstructor => Ok(self.execute_array_constructor(&args)),
            NativeFunction::ArrayIsArray => {
                let is_array = args.first().is_some_and(|value| match value {
                    JsValue::Object(object_id) => {
                        self.objects.get(object_id).is_some_and(|object| {
                            object.properties.contains_key("length")
                                && object.prototype == self.array_prototype_id
                        })
                    }
                    _ => false,
                });
                Ok(JsValue::Bool(is_array))
            }
            NativeFunction::ObjectKeys => self.execute_object_keys(&args),
            NativeFunction::ObjectGetOwnPropertyNames => {
                self.execute_object_get_own_property_names(&args)
            }
            NativeFunction::ObjectCreate => self.execute_object_create(&args),
            NativeFunction::ObjectSetPrototypeOf => self.execute_object_set_prototype_of(&args),
            NativeFunction::ObjectDefineProperty => {
                self.execute_object_define_property(&args, realm)
            }
            NativeFunction::ObjectDefineProperties => {
                self.execute_object_define_properties(&args, realm)
            }
            NativeFunction::ObjectGetOwnPropertyDescriptor => {
                self.execute_object_get_own_property_descriptor(&args)
            }
            NativeFunction::ObjectGetPrototypeOf => self.execute_object_get_prototype_of(&args),
            NativeFunction::ObjectPreventExtensions => {
                self.execute_object_prevent_extensions(&args)
            }
            NativeFunction::ObjectIsExtensible => self.execute_object_is_extensible(&args),
            NativeFunction::ObjectFreeze => self.execute_object_freeze(&args),
            NativeFunction::ObjectForInKeys => self.execute_object_for_in_keys(&args),
            NativeFunction::ObjectForOfValues => self.execute_object_for_of_values(&args, realm),
            NativeFunction::ObjectForOfIterator => {
                self.execute_object_for_of_iterator(&args, realm)
            }
            NativeFunction::ObjectForOfStep => self.execute_object_for_of_step(&args, realm),
            NativeFunction::ObjectForOfClose => self.execute_object_for_of_close(&args, realm),
            NativeFunction::ObjectGetTemplateObject => {
                self.execute_object_get_template_object(&args)
            }
            NativeFunction::ObjectTdzMarker => Ok(JsValue::Uninitialized),
            NativeFunction::NumberConstructor => {
                let value = args.first().cloned().unwrap_or(JsValue::Number(0.0));
                Ok(JsValue::Number(self.to_number(&value)))
            }
            NativeFunction::BooleanConstructor => {
                let value = args.first().cloned().unwrap_or(JsValue::Bool(false));
                Ok(JsValue::Bool(self.is_truthy(&value)))
            }
            NativeFunction::ArrayBufferConstructor => self.execute_array_buffer_constructor(&args),
            NativeFunction::DataViewConstructor => self.execute_data_view_constructor(&args),
            NativeFunction::MapConstructor => self.execute_map_constructor(&args, realm),
            NativeFunction::SetConstructor => self.execute_set_constructor(&args, realm),
            NativeFunction::PromiseConstructor => {
                self.execute_promise_constructor(&args, realm, caller_strict)
            }
            NativeFunction::Uint8ArrayConstructor => self.execute_uint8_array_constructor(&args),
            NativeFunction::DateConstructor => {
                let timestamp = args
                    .first()
                    .map(|value| self.to_number(value))
                    .unwrap_or(0.0);
                Ok(JsValue::String(format!(
                    "Date({})",
                    Self::coerce_number_to_string(timestamp)
                )))
            }
            NativeFunction::DateParse => {
                let value = args.first().cloned().unwrap_or(JsValue::Undefined);
                let text = self.coerce_to_string(&value);
                let parsed = text.trim().parse::<f64>().ok().unwrap_or(f64::NAN);
                Ok(JsValue::Number(parsed))
            }
            NativeFunction::DateUtc => {
                let value = args.first().cloned().unwrap_or(JsValue::Undefined);
                Ok(JsValue::Number(self.to_number(&value)))
            }
            NativeFunction::DatePrototypeMethod => Ok(JsValue::Number(f64::NAN)),
            NativeFunction::MathAbs => {
                let value = args.first().cloned().unwrap_or(JsValue::Number(f64::NAN));
                Ok(JsValue::Number(self.to_number(&value).abs()))
            }
            NativeFunction::MathAcos => {
                let value = args.first().cloned().unwrap_or(JsValue::Number(f64::NAN));
                Ok(JsValue::Number(self.to_number(&value).acos()))
            }
            NativeFunction::MathAsin => {
                let value = args.first().cloned().unwrap_or(JsValue::Number(f64::NAN));
                Ok(JsValue::Number(self.to_number(&value).asin()))
            }
            NativeFunction::MathAtan => {
                let value = args.first().cloned().unwrap_or(JsValue::Number(f64::NAN));
                Ok(JsValue::Number(self.to_number(&value).atan()))
            }
            NativeFunction::MathAtan2 => {
                let y = args.first().cloned().unwrap_or(JsValue::Number(f64::NAN));
                let x = args.get(1).cloned().unwrap_or(JsValue::Number(f64::NAN));
                Ok(JsValue::Number(
                    self.to_number(&y).atan2(self.to_number(&x)),
                ))
            }
            NativeFunction::MathCeil => {
                let value = args.first().cloned().unwrap_or(JsValue::Number(f64::NAN));
                Ok(JsValue::Number(self.to_number(&value).ceil()))
            }
            NativeFunction::MathCos => {
                let value = args.first().cloned().unwrap_or(JsValue::Number(f64::NAN));
                Ok(JsValue::Number(self.to_number(&value).cos()))
            }
            NativeFunction::MathExp => {
                let value = args.first().cloned().unwrap_or(JsValue::Number(f64::NAN));
                Ok(JsValue::Number(self.to_number(&value).exp()))
            }
            NativeFunction::MathFloor => {
                let value = args.first().cloned().unwrap_or(JsValue::Number(f64::NAN));
                Ok(JsValue::Number(self.to_number(&value).floor()))
            }
            NativeFunction::MathLog => {
                let value = args.first().cloned().unwrap_or(JsValue::Number(f64::NAN));
                Ok(JsValue::Number(self.to_number(&value).ln()))
            }
            NativeFunction::MathMax => {
                if args.is_empty() {
                    return Ok(JsValue::Number(f64::NEG_INFINITY));
                }
                let mut result = f64::NEG_INFINITY;
                for value in args {
                    let number = self.to_number(&value);
                    if number.is_nan() {
                        return Ok(JsValue::Number(f64::NAN));
                    }
                    result = result.max(number);
                }
                Ok(JsValue::Number(result))
            }
            NativeFunction::MathMin => {
                if args.is_empty() {
                    return Ok(JsValue::Number(f64::INFINITY));
                }
                let mut result = f64::INFINITY;
                for value in args {
                    let number = self.to_number(&value);
                    if number.is_nan() {
                        return Ok(JsValue::Number(f64::NAN));
                    }
                    result = result.min(number);
                }
                Ok(JsValue::Number(result))
            }
            NativeFunction::MathPow => {
                let base = args.first().cloned().unwrap_or(JsValue::Number(f64::NAN));
                let exponent = args.get(1).cloned().unwrap_or(JsValue::Number(f64::NAN));
                Ok(JsValue::Number(
                    self.to_number(&base).powf(self.to_number(&exponent)),
                ))
            }
            NativeFunction::MathRandom => {
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.subsec_nanos())
                    .unwrap_or(0);
                Ok(JsValue::Number((nanos as f64) / 1_000_000_000.0))
            }
            NativeFunction::MathRound => {
                let value = args.first().cloned().unwrap_or(JsValue::Number(f64::NAN));
                Ok(JsValue::Number(self.to_number(&value).round()))
            }
            NativeFunction::MathSin => {
                let value = args.first().cloned().unwrap_or(JsValue::Number(f64::NAN));
                Ok(JsValue::Number(self.to_number(&value).sin()))
            }
            NativeFunction::MathSqrt => {
                let value = args.first().cloned().unwrap_or(JsValue::Number(f64::NAN));
                Ok(JsValue::Number(self.to_number(&value).sqrt()))
            }
            NativeFunction::MathTan => {
                let value = args.first().cloned().unwrap_or(JsValue::Number(f64::NAN));
                Ok(JsValue::Number(self.to_number(&value).tan()))
            }
            NativeFunction::StringConstructor => {
                let value = args
                    .first()
                    .map_or(String::new(), |value| self.coerce_to_string(value));
                Ok(JsValue::String(value))
            }
            NativeFunction::StringFromCharCode => {
                let mut output = String::new();
                for value in args {
                    let code = (self.to_number(&value) as i64 as u32) & 0xFFFF;
                    if let Some(ch) = char::from_u32(code) {
                        output.push(ch);
                    } else {
                        output.push('\u{FFFD}');
                    }
                }
                Ok(JsValue::String(output))
            }
            NativeFunction::SymbolConstructor => {
                let description = match args.first() {
                    None | Some(JsValue::Undefined) => String::new(),
                    Some(value) => self.coerce_to_string(value),
                };
                let value = if description.is_empty() {
                    "Symbol()".to_string()
                } else {
                    format!("Symbol({description})")
                };
                Ok(JsValue::String(value))
            }
            NativeFunction::IsNaN => {
                let value = args.first().cloned().unwrap_or(JsValue::Number(f64::NAN));
                Ok(JsValue::Bool(self.to_number(&value).is_nan()))
            }
            NativeFunction::IsFinite => {
                let value = args.first().cloned().unwrap_or(JsValue::Number(f64::NAN));
                Ok(JsValue::Bool(self.to_number(&value).is_finite()))
            }
            NativeFunction::ParseInt => Ok(JsValue::Number(self.parse_int_baseline(&args))),
            NativeFunction::ParseFloat => Ok(JsValue::Number(self.parse_float_baseline(&args))),
            NativeFunction::Assert => {
                let condition = args.first().cloned().unwrap_or(JsValue::Undefined);
                if self.is_truthy(&condition) {
                    Ok(JsValue::Undefined)
                } else {
                    let detail = args.get(1).map_or_else(
                        || "assert() failed".to_string(),
                        |value| self.coerce_to_string(value),
                    );
                    Err(self.assertion_failure(&detail))
                }
            }
            NativeFunction::Test262Error => Ok(JsValue::String(
                self.format_error_constructor_result("Test262Error", &args),
            )),
            NativeFunction::ErrorConstructor => Ok(self.execute_error_constructor(
                NativeFunction::ErrorConstructor,
                "Error",
                &args,
            )),
            NativeFunction::TypeErrorConstructor => Ok(self.execute_error_constructor(
                NativeFunction::TypeErrorConstructor,
                "TypeError",
                &args,
            )),
            NativeFunction::ReferenceErrorConstructor => Ok(self.execute_error_constructor(
                NativeFunction::ReferenceErrorConstructor,
                "ReferenceError",
                &args,
            )),
            NativeFunction::SyntaxErrorConstructor => Ok(self.execute_error_constructor(
                NativeFunction::SyntaxErrorConstructor,
                "SyntaxError",
                &args,
            )),
            NativeFunction::EvalErrorConstructor => Ok(self.execute_error_constructor(
                NativeFunction::EvalErrorConstructor,
                "EvalError",
                &args,
            )),
            NativeFunction::RangeErrorConstructor => Ok(self.execute_error_constructor(
                NativeFunction::RangeErrorConstructor,
                "RangeError",
                &args,
            )),
            NativeFunction::URIErrorConstructor => Ok(self.execute_error_constructor(
                NativeFunction::URIErrorConstructor,
                "URIError",
                &args,
            )),
            NativeFunction::RegExpConstructor => {
                let pattern = args
                    .first()
                    .map_or(String::new(), |value| self.coerce_to_string(value));
                let flags = args
                    .get(1)
                    .map_or(String::new(), |value| self.coerce_to_string(value));
                self.create_regexp_value(pattern, flags)
            }
        }
    }

    fn format_error_constructor_result(&self, name: &str, args: &[JsValue]) -> String {
        match args.first() {
            Some(value) => format!("{name}: {}", self.coerce_to_string(value)),
            None => name.to_string(),
        }
    }

    fn execute_error_constructor(
        &mut self,
        constructor: NativeFunction,
        name: &str,
        args: &[JsValue],
    ) -> JsValue {
        let message = args
            .first()
            .map(|value| self.coerce_to_string(value))
            .unwrap_or_default();
        self.create_error_exception(constructor, name, message)
    }

    fn error_prototype_for_constructor(&mut self, constructor: NativeFunction) -> Option<ObjectId> {
        match constructor {
            NativeFunction::ErrorConstructor => match self.error_prototype_value() {
                JsValue::Object(id) => Some(id),
                _ => None,
            },
            NativeFunction::TypeErrorConstructor => match self.type_error_prototype_value() {
                JsValue::Object(id) => Some(id),
                _ => None,
            },
            NativeFunction::ReferenceErrorConstructor
            | NativeFunction::SyntaxErrorConstructor
            | NativeFunction::EvalErrorConstructor
            | NativeFunction::RangeErrorConstructor
            | NativeFunction::URIErrorConstructor => match self.error_prototype_value() {
                JsValue::Object(id) => Some(id),
                _ => None,
            },
            _ => None,
        }
    }

    fn create_regexp_value(&mut self, pattern: String, flags: String) -> Result<JsValue, VmError> {
        if self.regexp_prototype_id.is_none() {
            let _ = self.regexp_prototype_value();
        }
        let object = self.create_object_value();
        let object_id = match object {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        let global = flags.contains('g');
        let ignore_case = flags.contains('i');
        let multiline = flags.contains('m');
        let dot_all = flags.contains('s');
        let unicode = flags.contains('u');
        let sticky = flags.contains('y');

        let target = self
            .objects
            .get_mut(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        target.prototype = self.regexp_prototype_id;
        target.prototype_value = None;
        target
            .properties
            .insert("__regexpTag".to_string(), JsValue::Bool(true));
        target
            .properties
            .insert("source".to_string(), JsValue::String(pattern));
        target
            .properties
            .insert("flags".to_string(), JsValue::String(flags));
        target
            .properties
            .insert("global".to_string(), JsValue::Bool(global));
        target
            .properties
            .insert("ignoreCase".to_string(), JsValue::Bool(ignore_case));
        target
            .properties
            .insert("multiline".to_string(), JsValue::Bool(multiline));
        target
            .properties
            .insert("dotAll".to_string(), JsValue::Bool(dot_all));
        target
            .properties
            .insert("unicode".to_string(), JsValue::Bool(unicode));
        target
            .properties
            .insert("sticky".to_string(), JsValue::Bool(sticky));
        target
            .properties
            .insert("lastIndex".to_string(), JsValue::Number(0.0));
        target.property_attributes.insert(
            "lastIndex".to_string(),
            PropertyAttributes {
                writable: true,
                enumerable: false,
                configurable: false,
            },
        );
        Ok(JsValue::Object(object_id))
    }

    fn execute_regexp_to_string(&self, object_id: ObjectId) -> Result<String, VmError> {
        let object = self
            .objects
            .get(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        let source = object
            .properties
            .get("source")
            .map_or_else(String::new, |value| self.coerce_to_string(value));
        let flags = object
            .properties
            .get("flags")
            .map_or_else(String::new, |value| self.coerce_to_string(value));
        Ok(format!("/{source}/{flags}"))
    }

    fn execute_regexp_test(&self, object_id: ObjectId, input: &str) -> Result<bool, VmError> {
        let object = self
            .objects
            .get(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        let pattern = object
            .properties
            .get("source")
            .map_or_else(String::new, |value| self.coerce_to_string(value));
        let flags = object
            .properties
            .get("flags")
            .map_or_else(String::new, |value| self.coerce_to_string(value));
        let last_index = object
            .properties
            .get("lastIndex")
            .map(|value| self.to_number(value))
            .filter(|value| value.is_finite() && *value >= 0.0)
            .map(|value| value as usize)
            .unwrap_or(0usize);

        let sticky = flags.contains('y');
        let unicode = flags.contains('u');
        let normalized_pattern = Self::normalize_regexp_pattern(&pattern, unicode);
        let normalized_input = if unicode {
            Self::normalize_regexp_input_for_unicode(input)
        } else {
            input.to_string()
        };
        let mut builder = RegexBuilder::new(&normalized_pattern);
        builder.case_insensitive(flags.contains('i'));
        builder.multi_line(flags.contains('m'));
        builder.dot_matches_new_line(flags.contains('s'));
        builder.unicode(unicode);
        let Ok(regex) = builder.build() else {
            return Ok(false);
        };

        if sticky {
            Ok(regex
                .find_at(&normalized_input, last_index)
                .is_some_and(|m| m.start() == last_index))
        } else {
            Ok(regex.is_match(&normalized_input))
        }
    }

    fn normalize_regexp_pattern(pattern: &str, unicode: bool) -> String {
        let chars: Vec<char> = pattern.chars().collect();
        let mut normalized = String::with_capacity(pattern.len());
        let mut index = 0usize;
        let mut in_character_class = false;
        while index < chars.len() {
            let ch = chars[index];
            if ch == '\\' && index + 1 < chars.len() {
                let next = chars[index + 1];
                if next == '0' {
                    let has_decimal_follow = chars
                        .get(index + 2)
                        .is_some_and(|candidate| candidate.is_ascii_digit());
                    if !has_decimal_follow {
                        normalized.push_str("\\x00");
                        index += 2;
                        continue;
                    }
                }
                if next == 'u' {
                    if let Some((code_point, consumed)) = Self::parse_regex_u_escape(&chars, index)
                    {
                        if unicode
                            && (0xD800..=0xDBFF).contains(&code_point)
                            && let Some((low_code_point, low_consumed)) =
                                Self::parse_regex_u_escape(&chars, index + consumed)
                            && (0xDC00..=0xDFFF).contains(&low_code_point)
                        {
                            let combined = 0x10000
                                + (((code_point - 0xD800) << 10) | (low_code_point - 0xDC00));
                            if let Some(value) = char::from_u32(combined) {
                                Self::push_regex_literal_char(
                                    &mut normalized,
                                    value,
                                    in_character_class,
                                );
                                index += consumed + low_consumed;
                                continue;
                            }
                        }
                        if let Some(value) = char::from_u32(code_point) {
                            Self::push_regex_literal_char(
                                &mut normalized,
                                value,
                                in_character_class,
                            );
                        } else if let Some(placeholder) =
                            Self::surrogate_placeholder_from_code_unit(code_point)
                        {
                            Self::push_regex_literal_char(
                                &mut normalized,
                                placeholder,
                                in_character_class,
                            );
                        } else {
                            normalized.push('\\');
                            normalized.push('u');
                        }
                        index += consumed;
                        continue;
                    }
                }
                normalized.push('\\');
                normalized.push(next);
                index += 2;
                continue;
            }
            if ch == '[' {
                in_character_class = true;
                normalized.push(ch);
                index += 1;
                continue;
            }
            if ch == ']' {
                in_character_class = false;
                normalized.push(ch);
                index += 1;
                continue;
            }
            normalized.push(ch);
            index += 1;
        }
        normalized
    }

    fn push_regex_literal_char(target: &mut String, ch: char, in_character_class: bool) {
        let needs_escape = if in_character_class {
            matches!(ch, '\\' | ']' | '-' | '^')
        } else {
            matches!(
                ch,
                '\\' | '.' | '^' | '$' | '|' | '(' | ')' | '[' | ']' | '{' | '}' | '*' | '+' | '?'
            )
        };
        if needs_escape {
            target.push('\\');
        }
        target.push(ch);
    }

    fn normalize_regexp_input_for_unicode(input: &str) -> String {
        let mut normalized = String::with_capacity(input.len());
        let chars: Vec<char> = input.chars().collect();
        let mut index = 0usize;
        while index < chars.len() {
            let current = chars[index];
            if let Some(high) = Self::surrogate_code_unit_from_placeholder(current)
                && (0xD800..=0xDBFF).contains(&high)
                && let Some(next_char) = chars.get(index + 1).copied()
                && let Some(low) = Self::surrogate_code_unit_from_placeholder(next_char)
                && (0xDC00..=0xDFFF).contains(&low)
            {
                let combined = 0x10000 + (((high - 0xD800) << 10) | (low - 0xDC00));
                if let Some(value) = char::from_u32(combined) {
                    normalized.push(value);
                    index += 2;
                    continue;
                }
            }
            normalized.push(current);
            index += 1;
        }
        normalized
    }

    fn parse_regex_u_escape(chars: &[char], start: usize) -> Option<(u32, usize)> {
        if chars.get(start) != Some(&'\\') || chars.get(start + 1) != Some(&'u') {
            return None;
        }
        if chars.get(start + 2) == Some(&'{') {
            let mut end = start + 3;
            while end < chars.len() && chars[end] != '}' {
                end += 1;
            }
            if end >= chars.len() || chars[end] != '}' || end == start + 3 {
                return None;
            }
            let hex: String = chars[start + 3..end].iter().collect();
            if !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
                return None;
            }
            let value = u32::from_str_radix(&hex, 16).ok()?;
            if value > 0x10FFFF {
                return None;
            }
            return Some((value, end + 1 - start));
        }
        if start + 5 >= chars.len() {
            return None;
        }
        let hex: String = chars[start + 2..start + 6].iter().collect();
        if !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
            return None;
        }
        let value = u32::from_str_radix(&hex, 16).ok()?;
        Some((value, 6))
    }

    fn surrogate_placeholder_from_code_unit(code_unit: u32) -> Option<char> {
        if !(0xD800..=0xDFFF).contains(&code_unit) {
            return None;
        }
        char::from_u32(0xE000 + (code_unit - 0xD800))
    }

    fn surrogate_code_unit_from_placeholder(ch: char) -> Option<u32> {
        let code = ch as u32;
        if !(0xE000..=0xE7FF).contains(&code) {
            return None;
        }
        Some(0xD800 + (code - 0xE000))
    }

    fn execute_boxed_primitive_constructor(
        &mut self,
        native: NativeFunction,
        args: Vec<JsValue>,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        let primitive = self.execute_native_call(native, args, realm, caller_strict)?;
        let object = self.create_object_value();
        let object_id = match &object {
            JsValue::Object(id) => *id,
            _ => unreachable!(),
        };
        let prototype_id = match native {
            NativeFunction::NumberConstructor => match self.number_prototype_value() {
                JsValue::Object(id) => Some(id),
                _ => None,
            },
            NativeFunction::BooleanConstructor => match self.boolean_prototype_value() {
                JsValue::Object(id) => Some(id),
                _ => None,
            },
            NativeFunction::StringConstructor => match self.string_prototype_value() {
                JsValue::Object(id) => Some(id),
                _ => None,
            },
            _ => None,
        };
        let target = self
            .objects
            .get_mut(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        target.prototype = prototype_id;
        target.prototype_value = None;
        target
            .properties
            .insert(BOXED_PRIMITIVE_VALUE_KEY.to_string(), primitive);
        target.property_attributes.insert(
            BOXED_PRIMITIVE_VALUE_KEY.to_string(),
            PropertyAttributes {
                writable: false,
                enumerable: false,
                configurable: false,
            },
        );
        target
            .properties
            .insert("constructor".to_string(), JsValue::NativeFunction(native));
        Ok(object)
    }

    fn execute_eval_argument(
        &mut self,
        arg: Option<&JsValue>,
        realm: &Realm,
        caller_strict: bool,
        call_kind: EvalCallKind,
    ) -> Result<JsValue, VmError> {
        match arg {
            Some(JsValue::String(source)) => {
                self.execute_eval(source, realm, caller_strict, call_kind)
            }
            Some(value) => Ok(value.clone()),
            None => Ok(JsValue::Undefined),
        }
    }

    fn execute_eval(
        &mut self,
        source: &str,
        realm: &Realm,
        caller_strict: bool,
        call_kind: EvalCallKind,
    ) -> Result<JsValue, VmError> {
        let force_strict = matches!(call_kind, EvalCallKind::Direct) && caller_strict;
        let parse_source: Cow<'_, str> = if force_strict {
            Cow::Owned(format!("\"use strict\";\n{source}"))
        } else {
            Cow::Borrowed(source)
        };
        let allow_super_reference = matches!(call_kind, EvalCallKind::Direct)
            && (self.resolve_binding_id("super").is_some()
                || self.resolve_super_base_value().is_some());
        let script = parse_script_with_super(parse_source.as_ref(), allow_super_reference)
            .map_err(|err| {
                VmError::UncaughtException(JsValue::String(format!("SyntaxError: {}", err.message)))
            })?;
        if matches!(call_kind, EvalCallKind::Direct)
            && self.current_eval_context_rejects_arguments_declaration()
            && Self::script_declares_arguments_binding(&script)
        {
            return Err(VmError::UncaughtException(JsValue::String(
                "SyntaxError: invalid eval declaration for arguments".to_string(),
            )));
        }
        let chunk = compile_script(&script);
        let eval_strict = self.code_is_strict(&chunk.code);
        if !eval_strict
            && Self::script_declares_restricted_global_function(&script)
            && self.eval_targets_global_var_scope(call_kind)
        {
            return Err(VmError::TypeError("cannot declare global function in eval"));
        }
        let saved_scopes = self.scopes.clone();
        let saved_var_scope_stack = self.var_scope_stack.clone();
        let saved_with_objects = self.with_objects.clone();

        match call_kind {
            EvalCallKind::Direct => {
                if eval_strict {
                    self.scopes.push(Rc::new(RefCell::new(BTreeMap::new())));
                    self.var_scope_stack
                        .push(self.scopes.len().saturating_sub(1));
                }
            }
            EvalCallKind::Indirect => {
                let global_scope = self
                    .scopes
                    .first()
                    .cloned()
                    .ok_or(VmError::ScopeUnderflow)?;
                if eval_strict {
                    self.scopes = vec![global_scope, Rc::new(RefCell::new(BTreeMap::new()))];
                    self.var_scope_stack = vec![1];
                } else {
                    self.scopes = vec![global_scope];
                    self.var_scope_stack = vec![0];
                }
                self.with_objects.clear();
            }
        }

        let deletable_eval_bindings = matches!(call_kind, EvalCallKind::Direct)
            && !eval_strict
            && !self.eval_targets_global_var_scope(call_kind);
        if deletable_eval_bindings {
            self.eval_deletable_binding_depth = self.eval_deletable_binding_depth.saturating_add(1);
        }
        let result = self.execute_inline_chunk(&chunk, realm, false);
        if deletable_eval_bindings {
            self.eval_deletable_binding_depth = self.eval_deletable_binding_depth.saturating_sub(1);
        }
        self.scopes = saved_scopes;
        self.var_scope_stack = saved_var_scope_stack;
        self.with_objects = saved_with_objects;
        result
    }

    fn eval_targets_global_var_scope(&self, call_kind: EvalCallKind) -> bool {
        match call_kind {
            EvalCallKind::Indirect => true,
            EvalCallKind::Direct => self.var_scope_stack.last().copied() == Some(0),
        }
    }

    fn execute_function_constructor(
        &mut self,
        args: &[JsValue],
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let (params, body) = if let Some(last) = args.last() {
            let params = args[..args.len().saturating_sub(1)]
                .iter()
                .map(|arg| self.coerce_to_string(arg))
                .collect::<Vec<_>>()
                .join(", ");
            (params, self.coerce_to_string(last))
        } else {
            (String::new(), String::new())
        };
        let source = format!("(function({params}) {{\n{body}\n}})");
        let expr = parse_expression(&source).map_err(|err| {
            VmError::UncaughtException(JsValue::String(format!("SyntaxError: {}", err.message)))
        })?;
        let chunk = compile_expression(&expr);
        let value = self.execute_inline_chunk(&chunk, realm, false)?;

        if let JsValue::Function(closure_id) = value {
            if let Some(global_scope) = self.scopes.first().cloned() {
                if let Some(closure) = self.closures.get_mut(&closure_id) {
                    closure.captured_scopes = vec![global_scope];
                    closure.captured_with_objects = Vec::new();
                }
            }
            Ok(JsValue::Function(closure_id))
        } else {
            Ok(value)
        }
    }

    fn execute_generator_function_constructor(
        &mut self,
        args: &[JsValue],
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let body = args
            .last()
            .map(|value| self.coerce_to_string(value))
            .unwrap_or_default();
        let yield_expressions = Self::parse_generator_constructor_yield_expressions(&body)
            .map_err(|message| {
                VmError::UncaughtException(JsValue::String(format!("SyntaxError: {message}")))
            })?;
        let transformed_body = if yield_expressions.is_empty() {
            "return [];".to_string()
        } else {
            format!("return [{}];", yield_expressions.join(", "))
        };
        let mut transformed_args = args.to_vec();
        if transformed_args.is_empty() {
            transformed_args.push(JsValue::String(transformed_body));
        } else {
            let body_index = transformed_args.len().saturating_sub(1);
            transformed_args[body_index] = JsValue::String(transformed_body);
        }
        let producer = self.execute_function_constructor(&transformed_args, realm)?;
        Ok(self.create_host_function_value(HostFunction::GeneratorFactory { producer }))
    }

    fn parse_generator_constructor_yield_expressions(body: &str) -> Result<Vec<String>, String> {
        let mut expressions = Vec::new();
        for segment in body.split(';') {
            let trimmed = segment.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Some(expression) = trimmed.strip_prefix("yield") else {
                return Err("unsupported GeneratorFunction body".to_string());
            };
            let expression = expression.trim();
            if expression.is_empty() {
                return Err("invalid yield expression".to_string());
            }
            expressions.push(expression.to_string());
        }
        Ok(expressions)
    }

    fn create_generator_iterator_from_values(
        &mut self,
        values: JsValue,
    ) -> Result<JsValue, VmError> {
        let iterator = self.create_object_value();
        let iterator_id = match iterator {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        let next = self.create_host_function_value(HostFunction::GeneratorIteratorNextThis);
        let iterator_method = self.create_host_function_value(HostFunction::ObjectValueOf);
        let object = self
            .objects
            .get_mut(&iterator_id)
            .ok_or(VmError::UnknownObject(iterator_id))?;
        object.properties.insert(
            GENERATOR_ITERATOR_MARKER_KEY.to_string(),
            JsValue::Bool(true),
        );
        object
            .properties
            .insert(GENERATOR_VALUES_KEY.to_string(), values);
        object
            .properties
            .insert(GENERATOR_INDEX_KEY.to_string(), JsValue::Number(0.0));
        object.properties.insert("next".to_string(), next);
        object
            .properties
            .insert("Symbol.iterator".to_string(), iterator_method);
        object.property_attributes.insert(
            "next".to_string(),
            PropertyAttributes {
                writable: true,
                enumerable: false,
                configurable: true,
            },
        );
        object.property_attributes.insert(
            "Symbol.iterator".to_string(),
            PropertyAttributes {
                writable: true,
                enumerable: false,
                configurable: true,
            },
        );
        Ok(JsValue::Object(iterator_id))
    }

    fn create_generator_iterator_from_closure_call(
        &mut self,
        closure_id: u64,
        args: Vec<JsValue>,
        this_arg: Option<JsValue>,
    ) -> Result<JsValue, VmError> {
        let iterator = self.create_object_value();
        let iterator_id = match iterator {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        let next = self.create_host_function_value(HostFunction::GeneratorIteratorNextThis);
        let iterator_method = self.create_host_function_value(HostFunction::ObjectValueOf);
        let args_array = self.create_array_from_values(args)?;
        let this_is_missing = this_arg.is_none();
        let stored_this = this_arg.unwrap_or(JsValue::Undefined);
        let object = self
            .objects
            .get_mut(&iterator_id)
            .ok_or(VmError::UnknownObject(iterator_id))?;
        object.properties.insert(
            GENERATOR_ITERATOR_MARKER_KEY.to_string(),
            JsValue::Bool(true),
        );
        object
            .properties
            .insert(GENERATOR_VALUES_KEY.to_string(), JsValue::Undefined);
        object
            .properties
            .insert(GENERATOR_INDEX_KEY.to_string(), JsValue::Number(0.0));
        object.properties.insert(
            GENERATOR_PRODUCER_CLOSURE_KEY.to_string(),
            JsValue::String(closure_id.to_string()),
        );
        object
            .properties
            .insert(GENERATOR_PRODUCER_ARGS_KEY.to_string(), args_array);
        object
            .properties
            .insert(GENERATOR_PRODUCER_THIS_KEY.to_string(), stored_this);
        object.properties.insert(
            GENERATOR_PRODUCER_THIS_IS_MISSING_KEY.to_string(),
            JsValue::Bool(this_is_missing),
        );
        object.properties.insert("next".to_string(), next);
        object
            .properties
            .insert("Symbol.iterator".to_string(), iterator_method);
        object.property_attributes.insert(
            "next".to_string(),
            PropertyAttributes {
                writable: true,
                enumerable: false,
                configurable: true,
            },
        );
        object.property_attributes.insert(
            "Symbol.iterator".to_string(),
            PropertyAttributes {
                writable: true,
                enumerable: false,
                configurable: true,
            },
        );
        Ok(JsValue::Object(iterator_id))
    }

    fn execute_generator_iterator_next(
        &mut self,
        this_arg: Option<JsValue>,
        resume_value: Option<JsValue>,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let iterator_id = match this_arg.unwrap_or(JsValue::Undefined) {
            JsValue::Object(object_id) => object_id,
            _ => {
                return Err(VmError::TypeError(
                    "generator iterator next receiver must be object",
                ));
            }
        };
        let (
            values,
            index,
            is_generator_iterator,
            producer_closure,
            producer_args,
            producer_this,
            producer_this_is_missing,
        ) = {
            let iterator = self
                .objects
                .get(&iterator_id)
                .ok_or(VmError::UnknownObject(iterator_id))?;
            let is_generator_iterator = iterator
                .properties
                .get(GENERATOR_ITERATOR_MARKER_KEY)
                .is_some_and(|value| matches!(value, JsValue::Bool(true)));
            let producer_closure = iterator
                .properties
                .get(GENERATOR_PRODUCER_CLOSURE_KEY)
                .and_then(|value| match value {
                    JsValue::String(raw) => raw.parse::<u64>().ok(),
                    _ => None,
                });
            let producer_args = iterator
                .properties
                .get(GENERATOR_PRODUCER_ARGS_KEY)
                .cloned();
            let producer_this = iterator
                .properties
                .get(GENERATOR_PRODUCER_THIS_KEY)
                .cloned();
            let producer_this_is_missing = iterator
                .properties
                .get(GENERATOR_PRODUCER_THIS_IS_MISSING_KEY)
                .is_some_and(|value| matches!(value, JsValue::Bool(true)));
            let values = iterator
                .properties
                .get(GENERATOR_VALUES_KEY)
                .cloned()
                .unwrap_or(JsValue::Undefined);
            let index = iterator
                .properties
                .get(GENERATOR_INDEX_KEY)
                .map(|value| self.to_number(value))
                .unwrap_or(0.0)
                .max(0.0) as usize;
            (
                values,
                index,
                is_generator_iterator,
                producer_closure,
                producer_args,
                producer_this,
                producer_this_is_missing,
            )
        };
        if !is_generator_iterator {
            return Err(VmError::TypeError("incompatible generator iterator"));
        }

        let (value, done) = if let Some(producer_closure_id) = producer_closure {
            match index {
                0 => (JsValue::Undefined, false),
                1 => {
                    let producer_args = self.collect_apply_arguments(producer_args.as_ref())?;
                    let producer_this = if producer_this_is_missing {
                        None
                    } else {
                        Some(producer_this.unwrap_or(JsValue::Undefined))
                    };
                    let resume_value = resume_value.unwrap_or(JsValue::Undefined);
                    let produced = self.execute_closure_call_with_generator_resume(
                        producer_closure_id,
                        producer_args,
                        producer_this,
                        realm,
                        resume_value,
                    )?;
                    (produced, true)
                }
                _ => (JsValue::Undefined, true),
            }
        } else {
            match values {
                JsValue::Object(values_id) => {
                    let values_object = self
                        .objects
                        .get(&values_id)
                        .ok_or(VmError::UnknownObject(values_id))?;
                    let length = values_object
                        .properties
                        .get("length")
                        .map(|length| self.to_number(length))
                        .unwrap_or(0.0)
                        .max(0.0) as usize;
                    if index >= length {
                        (JsValue::Undefined, true)
                    } else {
                        (
                            values_object
                                .properties
                                .get(&index.to_string())
                                .cloned()
                                .unwrap_or(JsValue::Undefined),
                            false,
                        )
                    }
                }
                _ => (JsValue::Undefined, true),
            }
        };

        if let Some(iterator) = self.objects.get_mut(&iterator_id) {
            iterator.properties.insert(
                GENERATOR_INDEX_KEY.to_string(),
                JsValue::Number((index + 1) as f64),
            );
        }
        self.create_for_of_step_result(done, value)
    }

    fn create_array_iterator_from_this(
        &mut self,
        this_arg: Option<JsValue>,
        kind: &str,
    ) -> Result<JsValue, VmError> {
        let target_id = match this_arg.unwrap_or(JsValue::Undefined) {
            JsValue::Object(object_id) => object_id,
            _ => return Err(VmError::TypeError("Array iterator receiver must be object")),
        };
        let iterator = self.create_object_value();
        let iterator_id = match iterator {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        let next = self.create_host_function_value(HostFunction::ArrayIteratorNextThis);
        let iterator_method = self.create_host_function_value(HostFunction::ObjectValueOf);
        let object = self
            .objects
            .get_mut(&iterator_id)
            .ok_or(VmError::UnknownObject(iterator_id))?;
        object
            .properties
            .insert(ARRAY_ITERATOR_MARKER_KEY.to_string(), JsValue::Bool(true));
        object.properties.insert(
            ARRAY_ITERATOR_TARGET_KEY.to_string(),
            JsValue::Object(target_id),
        );
        object
            .properties
            .insert(ARRAY_ITERATOR_INDEX_KEY.to_string(), JsValue::Number(0.0));
        object.properties.insert(
            ARRAY_ITERATOR_KIND_KEY.to_string(),
            JsValue::String(kind.to_string()),
        );
        object.properties.insert("next".to_string(), next);
        object
            .properties
            .insert("Symbol.iterator".to_string(), iterator_method);
        object.property_attributes.insert(
            "next".to_string(),
            PropertyAttributes {
                writable: true,
                enumerable: false,
                configurable: true,
            },
        );
        object.property_attributes.insert(
            "Symbol.iterator".to_string(),
            PropertyAttributes {
                writable: true,
                enumerable: false,
                configurable: true,
            },
        );
        Ok(JsValue::Object(iterator_id))
    }

    fn execute_array_iterator_next(
        &mut self,
        this_arg: Option<JsValue>,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let iterator_id = match this_arg.unwrap_or(JsValue::Undefined) {
            JsValue::Object(object_id) => object_id,
            _ => {
                return Err(VmError::TypeError(
                    "Array iterator next receiver must be object",
                ));
            }
        };
        let (target_id, index, kind, is_array_iterator) = {
            let iterator = self
                .objects
                .get(&iterator_id)
                .ok_or(VmError::UnknownObject(iterator_id))?;
            let is_array_iterator = iterator
                .properties
                .get(ARRAY_ITERATOR_MARKER_KEY)
                .is_some_and(|value| matches!(value, JsValue::Bool(true)));
            let target_id = match iterator.properties.get(ARRAY_ITERATOR_TARGET_KEY) {
                Some(JsValue::Object(object_id)) => *object_id,
                _ => return self.create_for_of_step_result(true, JsValue::Undefined),
            };
            let index = iterator
                .properties
                .get(ARRAY_ITERATOR_INDEX_KEY)
                .map(|value| self.to_number(value))
                .unwrap_or(0.0)
                .max(0.0) as usize;
            let kind = iterator
                .properties
                .get(ARRAY_ITERATOR_KIND_KEY)
                .map(|value| self.coerce_to_string(value))
                .unwrap_or_else(|| "values".to_string());
            (target_id, index, kind, is_array_iterator)
        };
        if !is_array_iterator {
            return Err(VmError::TypeError("incompatible Array iterator"));
        }

        let length = self.array_length(target_id)?;
        if index >= length {
            return self.create_for_of_step_result(true, JsValue::Undefined);
        }

        let element = self.get_object_property(target_id, &index.to_string(), realm)?;
        let step_value = match kind.as_str() {
            "keys" => JsValue::Number(index as f64),
            "entries" => {
                self.create_array_from_values(vec![JsValue::Number(index as f64), element])?
            }
            _ => element,
        };
        if let Some(iterator) = self.objects.get_mut(&iterator_id) {
            iterator.properties.insert(
                ARRAY_ITERATOR_INDEX_KEY.to_string(),
                JsValue::Number((index + 1) as f64),
            );
        }
        self.create_for_of_step_result(false, step_value)
    }

    fn execute_object_constructor(&mut self, args: &[JsValue]) -> JsValue {
        match args.first() {
            None | Some(JsValue::Null) | Some(JsValue::Undefined) => self.create_object_value(),
            Some(JsValue::Object(id)) => JsValue::Object(*id),
            Some(value) => {
                let object = self.create_object_value();
                if let JsValue::Object(id) = object {
                    if let Some(target) = self.objects.get_mut(&id) {
                        target.properties.insert("value".to_string(), value.clone());
                    }
                    JsValue::Object(id)
                } else {
                    unreachable!()
                }
            }
        }
    }

    fn execute_array_constructor(&mut self, args: &[JsValue]) -> JsValue {
        let array = self.create_array_value();
        let object_id = match array {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };

        let object = self
            .objects
            .get_mut(&object_id)
            .expect("array object should exist");

        if args.len() == 1 {
            match args.first() {
                Some(JsValue::Number(length)) if length.is_finite() && *length >= 0.0 => {
                    let int_length = length.floor();
                    if (int_length - length).abs() <= f64::EPSILON {
                        object
                            .properties
                            .insert("length".to_string(), JsValue::Number(int_length));
                        return JsValue::Object(object_id);
                    }
                }
                Some(value) => {
                    object.properties.insert("0".to_string(), value.clone());
                    object
                        .properties
                        .insert("length".to_string(), JsValue::Number(1.0));
                    return JsValue::Object(object_id);
                }
                None => return JsValue::Object(object_id),
            }
        }

        for (index, value) in args.iter().enumerate() {
            object.properties.insert(index.to_string(), value.clone());
        }
        object
            .properties
            .insert("length".to_string(), JsValue::Number(args.len() as f64));
        JsValue::Object(object_id)
    }

    fn execute_array_buffer_constructor(&mut self, args: &[JsValue]) -> Result<JsValue, VmError> {
        let length = args
            .first()
            .map(|value| self.to_number(value))
            .filter(|value| value.is_finite() && *value >= 0.0)
            .unwrap_or(0.0)
            .floor() as usize;
        if self.array_buffer_prototype_id.is_none() {
            let _ = self.array_buffer_prototype_value();
        }
        let object = self.create_object_value();
        let object_id = match object {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        let target = self
            .objects
            .get_mut(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        target.prototype = self.array_buffer_prototype_id;
        target.prototype_value = None;
        target
            .properties
            .insert("__arrayBufferTag".to_string(), JsValue::Bool(true));
        target
            .properties
            .insert("byteLength".to_string(), JsValue::Number(length as f64));
        target.property_attributes.insert(
            "byteLength".to_string(),
            PropertyAttributes {
                writable: false,
                enumerable: false,
                configurable: true,
            },
        );
        Ok(JsValue::Object(object_id))
    }

    fn execute_data_view_constructor(&mut self, args: &[JsValue]) -> Result<JsValue, VmError> {
        let buffer = args.first().cloned().unwrap_or(JsValue::Undefined);
        let JsValue::Object(buffer_id) = buffer.clone() else {
            return Err(VmError::TypeError(
                "DataView constructor requires ArrayBuffer",
            ));
        };
        if !self.has_object_marker(buffer_id, "__arrayBufferTag")? {
            return Err(VmError::TypeError(
                "DataView constructor requires ArrayBuffer",
            ));
        }
        if self.data_view_prototype_id.is_none() {
            let _ = self.data_view_prototype_value();
        }
        let object = self.create_object_value();
        let object_id = match object {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        let target = self
            .objects
            .get_mut(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        target.prototype = self.data_view_prototype_id;
        target.prototype_value = None;
        target
            .properties
            .insert("__dataViewTag".to_string(), JsValue::Bool(true));
        target.properties.insert("buffer".to_string(), buffer);
        Ok(JsValue::Object(object_id))
    }

    fn execute_map_constructor(
        &mut self,
        args: &[JsValue],
        _realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let initial_size = match args.first() {
            None | Some(JsValue::Undefined) | Some(JsValue::Null) => 0.0,
            Some(JsValue::Object(object_id)) => self.array_length(*object_id)? as f64,
            Some(_) => 0.0,
        };
        if self.map_prototype_id.is_none() {
            let _ = self.map_prototype_value();
        }
        let object = self.create_object_value();
        let object_id = match object {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        let target = self
            .objects
            .get_mut(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        target.prototype = self.map_prototype_id;
        target.prototype_value = None;
        target
            .properties
            .insert("__mapTag".to_string(), JsValue::Bool(true));
        target
            .properties
            .insert("__mapSize".to_string(), JsValue::Number(initial_size));
        target
            .properties
            .insert("size".to_string(), JsValue::Number(initial_size));
        target.property_attributes.insert(
            "size".to_string(),
            PropertyAttributes {
                writable: false,
                enumerable: false,
                configurable: true,
            },
        );
        Ok(JsValue::Object(object_id))
    }

    fn execute_set_constructor(
        &mut self,
        args: &[JsValue],
        _realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let initial_size = match args.first() {
            None | Some(JsValue::Undefined) | Some(JsValue::Null) => 0.0,
            Some(JsValue::Object(object_id)) => self.array_length(*object_id)? as f64,
            Some(_) => 0.0,
        };
        if self.set_prototype_id.is_none() {
            let _ = self.set_prototype_value();
        }
        let object = self.create_object_value();
        let object_id = match object {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        let target = self
            .objects
            .get_mut(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        target.prototype = self.set_prototype_id;
        target.prototype_value = None;
        target
            .properties
            .insert("__setTag".to_string(), JsValue::Bool(true));
        target
            .properties
            .insert("__setSize".to_string(), JsValue::Number(initial_size));
        target
            .properties
            .insert("size".to_string(), JsValue::Number(initial_size));
        target.property_attributes.insert(
            "size".to_string(),
            PropertyAttributes {
                writable: false,
                enumerable: false,
                configurable: true,
            },
        );
        Ok(JsValue::Object(object_id))
    }

    fn execute_promise_constructor(
        &mut self,
        args: &[JsValue],
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        let executor = args.first().cloned().unwrap_or(JsValue::Undefined);
        if !Self::is_callable_value(&executor) {
            return Err(VmError::TypeError("Promise executor must be callable"));
        }
        if self.promise_prototype_id.is_none() {
            let _ = self.promise_prototype_value();
        }
        let object = self.create_object_value();
        let object_id = match object {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        {
            let target = self
                .objects
                .get_mut(&object_id)
                .ok_or(VmError::UnknownObject(object_id))?;
            target.prototype = self.promise_prototype_id;
            target.prototype_value = None;
            target
                .properties
                .insert("__promiseTag".to_string(), JsValue::Bool(true));
        }
        let resolve = self.create_host_function_value(HostFunction::FunctionPrototype);
        let reject = self.create_host_function_value(HostFunction::FunctionPrototype);
        let _ = self.execute_callable(
            executor,
            Some(JsValue::Undefined),
            vec![resolve, reject],
            realm,
            caller_strict,
        )?;
        Ok(JsValue::Object(object_id))
    }

    fn create_async_settled_promise(
        &mut self,
        fulfilled: bool,
        result: JsValue,
    ) -> Result<JsValue, VmError> {
        if self.promise_prototype_id.is_none() {
            let _ = self.promise_prototype_value();
        }
        let object = self.create_object_value();
        let object_id = match object {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        let target = self
            .objects
            .get_mut(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        target.prototype = self.promise_prototype_id;
        target.prototype_value = None;
        target
            .properties
            .insert("__promiseTag".to_string(), JsValue::Bool(true));
        target.properties.insert(
            "__asyncState".to_string(),
            JsValue::String(if fulfilled { "fulfilled" } else { "rejected" }.to_string()),
        );
        target
            .properties
            .insert("__asyncResult".to_string(), result);
        Ok(JsValue::Object(object_id))
    }

    fn execute_uint8_array_constructor(&mut self, args: &[JsValue]) -> Result<JsValue, VmError> {
        let length = args
            .first()
            .map(|value| self.to_number(value))
            .filter(|value| value.is_finite() && *value >= 0.0)
            .unwrap_or(0.0)
            .floor() as usize;
        if self.uint8_array_prototype_id.is_none() {
            let _ = self.uint8_array_prototype_value();
        }
        let object = self.create_object_value();
        let object_id = match object {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        let target = self
            .objects
            .get_mut(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        target.prototype = self.uint8_array_prototype_id;
        target.prototype_value = None;
        target
            .properties
            .insert("__uint8ArrayTag".to_string(), JsValue::Bool(true));
        target
            .properties
            .insert("length".to_string(), JsValue::Number(length as f64));
        target.property_attributes.insert(
            "length".to_string(),
            PropertyAttributes {
                writable: false,
                enumerable: false,
                configurable: false,
            },
        );
        target
            .properties
            .insert("byteLength".to_string(), JsValue::Number(length as f64));
        target.property_attributes.insert(
            "byteLength".to_string(),
            PropertyAttributes {
                writable: false,
                enumerable: false,
                configurable: false,
            },
        );
        for index in 0..length {
            let key = index.to_string();
            target.properties.insert(key.clone(), JsValue::Number(0.0));
            target.property_attributes.insert(
                key,
                PropertyAttributes {
                    writable: true,
                    enumerable: true,
                    configurable: false,
                },
            );
        }
        Ok(JsValue::Object(object_id))
    }

    fn execute_object_create(&mut self, args: &[JsValue]) -> Result<JsValue, VmError> {
        let (prototype, prototype_value) =
            self.parse_prototype_value(args.first().cloned().unwrap_or(JsValue::Undefined))?;

        let object = self.create_object_value();
        let object_id = match object {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        let target = self
            .objects
            .get_mut(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        target.prototype = prototype;
        target.prototype_value = prototype_value;
        Ok(JsValue::Object(object_id))
    }

    fn execute_object_set_prototype_of(&mut self, args: &[JsValue]) -> Result<JsValue, VmError> {
        let target = args.first().cloned().unwrap_or(JsValue::Undefined);

        let (prototype, prototype_value) =
            self.parse_prototype_value(args.get(1).cloned().unwrap_or(JsValue::Undefined))?;

        match target {
            JsValue::Object(target_id) => {
                let object = self
                    .objects
                    .get_mut(&target_id)
                    .ok_or(VmError::UnknownObject(target_id))?;
                object.prototype = prototype;
                object.prototype_value = prototype_value.clone();
                Ok(JsValue::Object(target_id))
            }
            JsValue::Function(closure_id) => {
                let closure_object = self.closure_objects.entry(closure_id).or_default();
                closure_object.prototype = prototype;
                closure_object.prototype_value = prototype_value.clone();
                closure_object.prototype_overridden = true;
                Ok(JsValue::Function(closure_id))
            }
            JsValue::HostFunction(host_id) => {
                let host_object = self.host_function_objects.entry(host_id).or_default();
                host_object.prototype = prototype;
                host_object.prototype_value = prototype_value;
                host_object.prototype_overridden = true;
                Ok(JsValue::HostFunction(host_id))
            }
            _ => Err(VmError::TypeError(
                "Object.setPrototypeOf target must be object",
            )),
        }
    }

    fn parse_prototype_value(
        &self,
        value: JsValue,
    ) -> Result<(Option<ObjectId>, Option<JsValue>), VmError> {
        match value {
            JsValue::Object(object_id) => Ok((Some(object_id), None)),
            JsValue::Null => Ok((None, None)),
            JsValue::Function(_) | JsValue::NativeFunction(_) | JsValue::HostFunction(_) => {
                Ok((None, Some(value)))
            }
            _ => Err(VmError::TypeError(
                "Object prototype may only be an Object or null",
            )),
        }
    }

    fn prototype_components_of_value(
        &self,
        value: &JsValue,
    ) -> Option<(Option<ObjectId>, Option<JsValue>)> {
        match value {
            JsValue::Object(object_id) => self
                .objects
                .get(object_id)
                .map(|object| (object.prototype, object.prototype_value.clone())),
            JsValue::Function(closure_id) => self
                .closure_objects
                .get(closure_id)
                .map(|object| (object.prototype, object.prototype_value.clone())),
            JsValue::HostFunction(host_id) => self
                .host_function_objects
                .get(host_id)
                .map(|object| (object.prototype, object.prototype_value.clone())),
            _ => None,
        }
    }

    fn apply_prototype_components_to_value(
        &mut self,
        value: &JsValue,
        prototype: Option<ObjectId>,
        prototype_value: Option<JsValue>,
    ) {
        match value {
            JsValue::Object(object_id) => {
                if let Some(object) = self.objects.get_mut(object_id) {
                    object.prototype = prototype;
                    object.prototype_value = prototype_value;
                }
            }
            JsValue::Function(closure_id) => {
                let object = self.closure_objects.entry(*closure_id).or_default();
                object.prototype = prototype;
                object.prototype_value = prototype_value;
                object.prototype_overridden = true;
            }
            JsValue::HostFunction(host_id) => {
                let object = self.host_function_objects.entry(*host_id).or_default();
                object.prototype = prototype;
                object.prototype_value = prototype_value;
                object.prototype_overridden = true;
            }
            _ => {}
        }
    }

    fn execute_date_constructor(&mut self, args: &[JsValue]) -> Result<JsValue, VmError> {
        let timestamp = if args.is_empty() {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_millis() as f64)
                .unwrap_or(0.0)
        } else {
            args.first()
                .map(|value| self.to_number(value))
                .unwrap_or(0.0)
        };
        let local_components = if args.len() >= 2 {
            let year = self.to_number(&args[0]) as i32;
            let month = self.to_number(&args[1]) as i32;
            let day = args
                .get(2)
                .map(|value| self.to_number(value))
                .unwrap_or(1.0) as i32;
            Some((year, month, day))
        } else {
            None
        };
        let object = self.create_object_value();
        let object_id = match object {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        let to_string = self.create_host_function_value(HostFunction::DateToString(object_id));
        let value_of = self.create_host_function_value(HostFunction::DateValueOf(object_id));
        if self.date_prototype_id.is_none() {
            let _ = self.date_prototype_value();
        }
        let target = self
            .objects
            .get_mut(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        target.prototype = self.date_prototype_id;
        target.prototype_value = None;
        target
            .properties
            .insert(DATE_OBJECT_MARKER_KEY.to_string(), JsValue::Bool(true));
        target
            .properties
            .insert("value".to_string(), JsValue::Number(timestamp));
        target.properties.insert("toString".to_string(), to_string);
        target.properties.insert("valueOf".to_string(), value_of);
        target.properties.insert(
            "constructor".to_string(),
            JsValue::NativeFunction(NativeFunction::DateConstructor),
        );
        target.property_attributes.insert(
            DATE_OBJECT_MARKER_KEY.to_string(),
            PropertyAttributes {
                writable: false,
                enumerable: false,
                configurable: false,
            },
        );
        target.property_attributes.insert(
            "value".to_string(),
            PropertyAttributes {
                writable: true,
                enumerable: false,
                configurable: false,
            },
        );
        if let Some((year, month, day)) = local_components {
            target
                .properties
                .insert("__dateYear".to_string(), JsValue::Number(year as f64));
            target
                .properties
                .insert("__dateMonth".to_string(), JsValue::Number(month as f64));
            target
                .properties
                .insert("__dateDay".to_string(), JsValue::Number(day as f64));
        }
        Ok(JsValue::Object(object_id))
    }

    fn execute_object_define_property(
        &mut self,
        args: &[JsValue],
        _realm: &Realm,
    ) -> Result<JsValue, VmError> {
        if let Some(JsValue::HostFunction(host_id)) = args.first() {
            if Some(*host_id) == self.function_prototype_host_id {
                let property = args
                    .get(1)
                    .map(|value| self.coerce_to_property_key(value))
                    .unwrap_or_default();
                if property == "prototype" {
                    let descriptor_id = match args.get(2) {
                        Some(JsValue::Object(id)) => Some(*id),
                        _ => None,
                    };
                    if let Some(descriptor_id) = descriptor_id {
                        let descriptor = self
                            .objects
                            .get(&descriptor_id)
                            .ok_or(VmError::UnknownObject(descriptor_id))?;
                        if let Some(getter) = descriptor.properties.get("get").cloned() {
                            if !matches!(getter, JsValue::Undefined)
                                && !Self::is_callable_value(&getter)
                            {
                                return Err(VmError::TypeError(
                                    "getter must be callable or undefined",
                                ));
                            }
                            self.function_prototype_prototype_getter =
                                if matches!(getter, JsValue::Undefined) {
                                    None
                                } else {
                                    Some(getter)
                                };
                        }
                    }
                    return Ok(JsValue::HostFunction(*host_id));
                }
            }
        }

        enum DefinePropertyTarget {
            Object(ObjectId),
            Function(u64),
            HostFunction(u64),
        }

        let target = match args.first() {
            Some(JsValue::Object(id)) => DefinePropertyTarget::Object(*id),
            Some(JsValue::Function(closure_id)) => {
                if !self.closures.contains_key(closure_id) {
                    return Err(VmError::UnknownClosure(*closure_id));
                }
                DefinePropertyTarget::Function(*closure_id)
            }
            Some(JsValue::HostFunction(host_id)) => {
                if !self.host_functions.contains_key(host_id) {
                    return Err(VmError::UnknownHostFunction(*host_id));
                }
                DefinePropertyTarget::HostFunction(*host_id)
            }
            _ => {
                return Err(VmError::TypeError(
                    "Object.defineProperty target must be object",
                ));
            }
        };
        let property = args
            .get(1)
            .map(|value| self.coerce_to_property_key(value))
            .unwrap_or_default();
        let descriptor_id = match args.get(2) {
            Some(JsValue::Object(id)) => Some(*id),
            _ => None,
        };

        let mut target_object = match target {
            DefinePropertyTarget::Object(target_id) => self
                .objects
                .get(&target_id)
                .cloned()
                .ok_or(VmError::UnknownObject(target_id))?,
            DefinePropertyTarget::Function(closure_id) => self
                .closure_objects
                .get(&closure_id)
                .cloned()
                .unwrap_or_default(),
            DefinePropertyTarget::HostFunction(host_id) => self
                .host_function_objects
                .get(&host_id)
                .cloned()
                .unwrap_or_default(),
        };

        if let Some(descriptor_id) = descriptor_id {
            let descriptor = self
                .objects
                .get(&descriptor_id)
                .ok_or(VmError::UnknownObject(descriptor_id))?
                .clone();

            let has_value = descriptor.properties.contains_key("value");
            let desc_value = descriptor
                .properties
                .get("value")
                .cloned()
                .unwrap_or(JsValue::Undefined);
            let has_get = descriptor.properties.contains_key("get");
            let desc_get = descriptor
                .properties
                .get("get")
                .cloned()
                .unwrap_or(JsValue::Undefined);
            let has_set = descriptor.properties.contains_key("set");
            let desc_set = descriptor
                .properties
                .get("set")
                .cloned()
                .unwrap_or(JsValue::Undefined);
            let desc_writable = descriptor
                .properties
                .get("writable")
                .map(|value| self.is_truthy(value));
            let desc_enumerable = descriptor
                .properties
                .get("enumerable")
                .map(|value| self.is_truthy(value));
            let desc_configurable = descriptor
                .properties
                .get("configurable")
                .map(|value| self.is_truthy(value));

            let (
                current_data_value,
                current_get,
                current_set,
                current_attributes,
                mapped_binding_id,
            ) = {
                (
                    target_object.properties.get(&property).cloned(),
                    target_object.getters.get(&property).cloned(),
                    target_object.setters.get(&property).cloned(),
                    target_object.property_attributes.get(&property).copied(),
                    target_object.argument_mappings.get(&property).copied(),
                )
            };

            let current_is_data = current_data_value.is_some();
            let current_is_accessor = current_get.is_some() || current_set.is_some();
            let current_attributes = current_attributes.unwrap_or(if current_is_accessor {
                PropertyAttributes {
                    writable: false,
                    enumerable: true,
                    configurable: true,
                }
            } else {
                PropertyAttributes::default()
            });

            if (current_is_data || current_is_accessor) && !current_attributes.configurable {
                if desc_configurable == Some(true) {
                    return Err(VmError::TypeError(
                        "cannot redefine non-configurable property",
                    ));
                }
                if let Some(enumerable) = desc_enumerable {
                    if enumerable != current_attributes.enumerable {
                        return Err(VmError::TypeError(
                            "cannot redefine non-configurable property",
                        ));
                    }
                }

                if current_is_data {
                    if has_get || has_set {
                        return Err(VmError::TypeError(
                            "cannot redefine non-configurable property",
                        ));
                    }
                    if !current_attributes.writable {
                        if desc_writable == Some(true) {
                            return Err(VmError::TypeError(
                                "cannot redefine non-configurable property",
                            ));
                        }
                        if has_value {
                            if let Some(current_value) = current_data_value.as_ref() {
                                if !self.same_value(current_value, &desc_value) {
                                    return Err(VmError::TypeError(
                                        "cannot redefine non-configurable property",
                                    ));
                                }
                            }
                        }
                    }
                } else {
                    if has_value || desc_writable.is_some() {
                        return Err(VmError::TypeError(
                            "cannot redefine non-configurable property",
                        ));
                    }
                    if has_get {
                        let current_get = current_get.unwrap_or(JsValue::Undefined);
                        if !self.same_value(&current_get, &desc_get) {
                            return Err(VmError::TypeError(
                                "cannot redefine non-configurable property",
                            ));
                        }
                    }
                    if has_set {
                        let current_set = current_set.unwrap_or(JsValue::Undefined);
                        if !self.same_value(&current_set, &desc_set) {
                            return Err(VmError::TypeError(
                                "cannot redefine non-configurable property",
                            ));
                        }
                    }
                }
            }

            let mut remove_mapping = false;
            if has_get || has_set {
                target_object.properties.remove(&property);
                if has_get {
                    if matches!(desc_get, JsValue::Undefined) {
                        target_object.getters.remove(&property);
                    } else {
                        target_object.getters.insert(property.clone(), desc_get);
                    }
                }
                if has_set {
                    if matches!(desc_set, JsValue::Undefined) {
                        target_object.setters.remove(&property);
                    } else {
                        target_object.setters.insert(property.clone(), desc_set);
                    }
                }
                let attributes = target_object
                    .property_attributes
                    .entry(property.clone())
                    .or_insert(PropertyAttributes {
                        writable: false,
                        enumerable: false,
                        configurable: false,
                    });
                attributes.writable = false;
                if let Some(enumerable) = desc_enumerable {
                    attributes.enumerable = enumerable;
                } else if !current_is_data && !current_is_accessor {
                    attributes.enumerable = false;
                }
                if let Some(configurable) = desc_configurable {
                    attributes.configurable = configurable;
                } else if !current_is_data && !current_is_accessor {
                    attributes.configurable = false;
                }
                remove_mapping = true;
            } else {
                target_object.getters.remove(&property);
                target_object.setters.remove(&property);
                if has_value {
                    target_object
                        .properties
                        .insert(property.clone(), desc_value.clone());
                } else if !current_is_data && !current_is_accessor {
                    target_object
                        .properties
                        .entry(property.clone())
                        .or_insert(JsValue::Undefined);
                }

                let attributes = target_object
                    .property_attributes
                    .entry(property.clone())
                    .or_insert(PropertyAttributes {
                        writable: false,
                        enumerable: false,
                        configurable: false,
                    });
                if !current_is_data && !current_is_accessor {
                    attributes.writable = desc_writable.unwrap_or(false);
                    attributes.enumerable = desc_enumerable.unwrap_or(false);
                    attributes.configurable = desc_configurable.unwrap_or(false);
                } else {
                    if let Some(writable) = desc_writable {
                        attributes.writable = writable;
                    }
                    if let Some(enumerable) = desc_enumerable {
                        attributes.enumerable = enumerable;
                    }
                    if let Some(configurable) = desc_configurable {
                        attributes.configurable = configurable;
                    }
                }
                if desc_writable == Some(false) {
                    remove_mapping = true;
                }
            }

            if has_value {
                if let Some(binding_id) = mapped_binding_id {
                    if let Some(binding) = self.bindings.get_mut(&binding_id) {
                        binding.value = desc_value;
                    }
                }
            }
            if remove_mapping {
                if !has_get && !has_set && !has_value {
                    if let Some(binding_id) = mapped_binding_id {
                        if let Some(binding) = self.bindings.get(&binding_id) {
                            target_object
                                .properties
                                .insert(property.clone(), binding.value.clone());
                        }
                    }
                }
                target_object.argument_mappings.remove(&property);
            }
        }

        match target {
            DefinePropertyTarget::Object(target_id) => {
                if target_object.properties.contains_key("length") {
                    if let Ok(index) = property.parse::<usize>() {
                        let current_length = target_object
                            .properties
                            .get("length")
                            .map(|value| self.to_number(value))
                            .unwrap_or(0.0)
                            .max(0.0) as usize;
                        if index >= current_length {
                            target_object
                                .properties
                                .insert("length".to_string(), JsValue::Number((index + 1) as f64));
                        }
                    }
                }
                let object = self
                    .objects
                    .get_mut(&target_id)
                    .ok_or(VmError::UnknownObject(target_id))?;
                *object = target_object;
                Ok(JsValue::Object(target_id))
            }
            DefinePropertyTarget::Function(closure_id) => {
                self.closure_objects.insert(closure_id, target_object);
                Ok(JsValue::Function(closure_id))
            }
            DefinePropertyTarget::HostFunction(host_id) => {
                self.host_function_objects.insert(host_id, target_object);
                Ok(JsValue::HostFunction(host_id))
            }
        }
    }

    fn execute_object_keys(&mut self, args: &[JsValue]) -> Result<JsValue, VmError> {
        let target_id = match args.first() {
            Some(JsValue::Object(id)) => *id,
            _ => return Err(VmError::TypeError("Object.keys target must be object")),
        };

        let object = self
            .objects
            .get(&target_id)
            .ok_or(VmError::UnknownObject(target_id))?;

        let mut keys = Vec::new();
        for key in object.properties.keys() {
            let enumerable = object
                .property_attributes
                .get(key)
                .map(|attrs| attrs.enumerable)
                .unwrap_or(true);
            if enumerable {
                keys.push(key.clone());
            }
        }
        for key in object.getters.keys() {
            if object.properties.contains_key(key) {
                continue;
            }
            let enumerable = object
                .property_attributes
                .get(key)
                .map(|attrs| attrs.enumerable)
                .unwrap_or(true);
            if enumerable {
                keys.push(key.clone());
            }
        }
        for key in object.setters.keys() {
            if object.properties.contains_key(key) || object.getters.contains_key(key) {
                continue;
            }
            let enumerable = object
                .property_attributes
                .get(key)
                .map(|attrs| attrs.enumerable)
                .unwrap_or(true);
            if enumerable {
                keys.push(key.clone());
            }
        }
        self.create_array_from_string_keys(keys)
    }

    fn execute_object_get_own_property_names(
        &mut self,
        args: &[JsValue],
    ) -> Result<JsValue, VmError> {
        let target_id = match args.first() {
            Some(JsValue::Object(id)) => *id,
            _ => {
                return Err(VmError::TypeError(
                    "Object.getOwnPropertyNames target must be object",
                ));
            }
        };

        let object = self
            .objects
            .get(&target_id)
            .ok_or(VmError::UnknownObject(target_id))?;

        let mut keys = Vec::new();
        for key in object.properties.keys() {
            keys.push(key.clone());
        }
        for key in object.getters.keys() {
            if !object.properties.contains_key(key) {
                keys.push(key.clone());
            }
        }
        for key in object.setters.keys() {
            if !object.properties.contains_key(key) && !object.getters.contains_key(key) {
                keys.push(key.clone());
            }
        }
        self.create_array_from_string_keys(keys)
    }

    fn execute_object_define_properties(
        &mut self,
        args: &[JsValue],
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let target = args.first().cloned().unwrap_or(JsValue::Undefined);
        if !matches!(target, JsValue::Object(_) | JsValue::Function(_)) {
            return Err(VmError::TypeError(
                "Object.defineProperties target must be object",
            ));
        }
        let descriptors_id = match args.get(1) {
            Some(JsValue::Object(id)) => *id,
            _ => {
                return Err(VmError::TypeError(
                    "Object.defineProperties descriptors must be object",
                ));
            }
        };
        let descriptor_entries = self
            .objects
            .get(&descriptors_id)
            .ok_or(VmError::UnknownObject(descriptors_id))?
            .properties
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>();
        for (property_name, descriptor) in descriptor_entries {
            let define_args = [target.clone(), JsValue::String(property_name), descriptor];
            let _ = self.execute_object_define_property(&define_args, realm)?;
        }
        Ok(target)
    }

    fn execute_object_for_in_keys(&mut self, args: &[JsValue]) -> Result<JsValue, VmError> {
        let target = args.first().cloned().unwrap_or(JsValue::Undefined);
        let keys = match target {
            JsValue::Object(object_id) => self.collect_for_in_keys(object_id)?,
            JsValue::Null | JsValue::Undefined => Vec::new(),
            _ => Vec::new(),
        };
        self.create_array_from_string_keys(keys)
    }

    fn execute_object_for_of_values(
        &mut self,
        args: &[JsValue],
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let target = args.first().cloned().unwrap_or(JsValue::Undefined);
        let values = self.collect_for_of_values(target, realm)?;
        self.create_array_from_values(values)
    }

    fn execute_object_for_of_iterator(
        &mut self,
        args: &[JsValue],
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let target = args.first().cloned().unwrap_or(JsValue::Undefined);
        match target {
            JsValue::String(value) => {
                self.create_for_of_snapshot_record(self.js_string_iterator_values(&value))
            }
            JsValue::Object(object_id) => {
                self.create_for_of_runtime_iterator_record(JsValue::Object(object_id), realm)
            }
            JsValue::Null | JsValue::Undefined => {
                Err(VmError::TypeError("for-of expects iterable"))
            }
            primitive => {
                let boxed = self.box_primitive_receiver(primitive);
                self.create_for_of_runtime_iterator_record(boxed, realm)
            }
        }
    }

    fn execute_object_for_of_step(
        &mut self,
        args: &[JsValue],
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let record = args.first().cloned().unwrap_or(JsValue::Undefined);
        let record_id = match record {
            JsValue::Object(id) => id,
            _ => {
                return Err(VmError::TypeError(
                    "for-of step expects iterator record object",
                ));
            }
        };
        let is_snapshot = self
            .objects
            .get(&record_id)
            .and_then(|object| object.properties.get("__forOfSnapshot"))
            .is_some_and(|value| matches!(value, JsValue::Bool(true)));
        if is_snapshot {
            let (values_id, index) = {
                let object = self
                    .objects
                    .get(&record_id)
                    .ok_or(VmError::UnknownObject(record_id))?;
                let values_id = match object.properties.get("__forOfValues") {
                    Some(JsValue::Object(id)) => *id,
                    _ => return self.create_for_of_step_result(true, JsValue::Undefined),
                };
                let index = object
                    .properties
                    .get("__forOfIndex")
                    .map(|value| self.to_number(value))
                    .unwrap_or(0.0)
                    .max(0.0) as usize;
                (values_id, index)
            };
            let length = self.array_length(values_id)?;
            if index >= length {
                return self.create_for_of_step_result(true, JsValue::Undefined);
            }
            let value = self.get_object_property(values_id, &index.to_string(), realm)?;
            let object = self
                .objects
                .get_mut(&record_id)
                .ok_or(VmError::UnknownObject(record_id))?;
            object.properties.insert(
                "__forOfIndex".to_string(),
                JsValue::Number((index + 1) as f64),
            );
            return self.create_for_of_step_result(false, value);
        }

        let (iterator, next_method) = {
            let object = self
                .objects
                .get(&record_id)
                .ok_or(VmError::UnknownObject(record_id))?;
            let iterator =
                object
                    .properties
                    .get("__forOfIterator")
                    .cloned()
                    .ok_or(VmError::TypeError(
                        "for-of iterator record missing iterator",
                    ))?;
            let next_method = object
                .properties
                .get("__forOfNext")
                .cloned()
                .ok_or(VmError::TypeError("for-of iterator record missing next"))?;
            (iterator, next_method)
        };
        let step_value = self.execute_callable(
            next_method,
            Some(iterator.clone()),
            Vec::new(),
            realm,
            false,
        )?;
        if !Self::is_object_like_value(&step_value) {
            return Err(VmError::TypeError(
                "for-of iterator next must return object",
            ));
        }
        let done = self
            .get_property_from_receiver(step_value.clone(), "done", realm)
            .map(|value| self.is_truthy(&value))?;
        if done {
            return self.create_for_of_step_result(true, JsValue::Undefined);
        }
        let value = self.get_property_from_receiver(step_value, "value", realm)?;
        self.create_for_of_step_result(false, value)
    }

    fn execute_object_for_of_close(
        &mut self,
        args: &[JsValue],
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let record = args.first().cloned().unwrap_or(JsValue::Undefined);
        let record_id = match record {
            JsValue::Object(id) => id,
            _ => {
                return Err(VmError::TypeError(
                    "for-of close expects iterator record object",
                ));
            }
        };
        let is_snapshot = self
            .objects
            .get(&record_id)
            .and_then(|object| object.properties.get("__forOfSnapshot"))
            .is_some_and(|value| matches!(value, JsValue::Bool(true)));
        if is_snapshot {
            return Ok(JsValue::Undefined);
        }
        let iterator = self
            .objects
            .get(&record_id)
            .and_then(|object| object.properties.get("__forOfIterator"))
            .cloned()
            .ok_or(VmError::TypeError(
                "for-of iterator record missing iterator",
            ))?;
        let return_method = self.get_property_from_receiver(iterator.clone(), "return", realm)?;
        if matches!(return_method, JsValue::Undefined | JsValue::Null) {
            return Ok(JsValue::Undefined);
        }
        if !Self::is_callable_value(&return_method) {
            return Err(VmError::TypeError(
                "for-of iterator return must be callable",
            ));
        }
        let result =
            self.execute_callable(return_method, Some(iterator), Vec::new(), realm, false)?;
        if !matches!(result, JsValue::Undefined) && !Self::is_object_like_value(&result) {
            return Err(VmError::TypeError(
                "for-of iterator return must return object",
            ));
        }
        Ok(JsValue::Undefined)
    }

    fn create_for_of_runtime_iterator_record(
        &mut self,
        target: JsValue,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let iterator_method =
            self.get_property_from_receiver(target.clone(), "Symbol.iterator", realm)?;
        if matches!(iterator_method, JsValue::Undefined | JsValue::Null) {
            return Err(VmError::TypeError("for-of expects iterable"));
        }
        if !Self::is_callable_value(&iterator_method) {
            return Err(VmError::TypeError("for-of expects iterable"));
        }
        let iterator =
            self.execute_callable(iterator_method, Some(target), Vec::new(), realm, false)?;
        if !Self::is_object_like_value(&iterator) {
            return Err(VmError::TypeError("for-of iterator must return object"));
        }
        let next_method = self.get_property_from_receiver(iterator.clone(), "next", realm)?;
        if !Self::is_callable_value(&next_method) {
            return Err(VmError::TypeError("for-of iterator next must be callable"));
        }
        let record = self.create_object_value();
        let record_id = match record {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        let object = self
            .objects
            .get_mut(&record_id)
            .ok_or(VmError::UnknownObject(record_id))?;
        object
            .properties
            .insert("__forOfSnapshot".to_string(), JsValue::Bool(false));
        object
            .properties
            .insert("__forOfIterator".to_string(), iterator);
        object
            .properties
            .insert("__forOfNext".to_string(), next_method);
        Ok(JsValue::Object(record_id))
    }

    fn create_for_of_snapshot_record(&mut self, values: Vec<JsValue>) -> Result<JsValue, VmError> {
        let values_array = self.create_array_from_values(values)?;
        let record = self.create_object_value();
        let record_id = match record {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        let object = self
            .objects
            .get_mut(&record_id)
            .ok_or(VmError::UnknownObject(record_id))?;
        object
            .properties
            .insert("__forOfSnapshot".to_string(), JsValue::Bool(true));
        object
            .properties
            .insert("__forOfValues".to_string(), values_array);
        object
            .properties
            .insert("__forOfIndex".to_string(), JsValue::Number(0.0));
        Ok(JsValue::Object(record_id))
    }

    fn create_for_of_step_result(
        &mut self,
        done: bool,
        value: JsValue,
    ) -> Result<JsValue, VmError> {
        let step = self.create_object_value();
        let step_id = match step {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        let object = self
            .objects
            .get_mut(&step_id)
            .ok_or(VmError::UnknownObject(step_id))?;
        object
            .properties
            .insert("done".to_string(), JsValue::Bool(done));
        object.properties.insert("value".to_string(), value);
        Ok(JsValue::Object(step_id))
    }

    fn collect_for_in_keys(&self, start_id: ObjectId) -> Result<Vec<String>, VmError> {
        let mut keys = Vec::new();
        let mut seen = BTreeSet::new();
        let mut current = Some(start_id);
        while let Some(object_id) = current {
            let object = self
                .objects
                .get(&object_id)
                .ok_or(VmError::UnknownObject(object_id))?;
            for key in object.properties.keys() {
                let enumerable = object
                    .property_attributes
                    .get(key)
                    .map(|attrs| attrs.enumerable)
                    .unwrap_or(true);
                let is_new = seen.insert(key.clone());
                if is_new && enumerable {
                    keys.push(key.clone());
                }
            }
            for key in object.getters.keys() {
                if object.properties.contains_key(key) {
                    continue;
                }
                let enumerable = object
                    .property_attributes
                    .get(key)
                    .map(|attrs| attrs.enumerable)
                    .unwrap_or(true);
                let is_new = seen.insert(key.clone());
                if is_new && enumerable {
                    keys.push(key.clone());
                }
            }
            for key in object.setters.keys() {
                if object.properties.contains_key(key) || object.getters.contains_key(key) {
                    continue;
                }
                let enumerable = object
                    .property_attributes
                    .get(key)
                    .map(|attrs| attrs.enumerable)
                    .unwrap_or(true);
                let is_new = seen.insert(key.clone());
                if is_new && enumerable {
                    keys.push(key.clone());
                }
            }
            current = object.prototype;
        }
        Ok(keys)
    }

    fn collect_for_of_values(
        &mut self,
        target: JsValue,
        realm: &Realm,
    ) -> Result<Vec<JsValue>, VmError> {
        match target {
            JsValue::Object(object_id) => {
                let length = self.array_length(object_id)?;
                let mut values = Vec::with_capacity(length);
                for index in 0..length {
                    let property = index.to_string();
                    values.push(self.get_object_property(object_id, &property, realm)?);
                }
                Ok(values)
            }
            JsValue::String(value) => Ok(value
                .chars()
                .map(|ch| JsValue::String(ch.to_string()))
                .collect()),
            _ => Ok(Vec::new()),
        }
    }

    fn create_array_from_string_keys(&mut self, keys: Vec<String>) -> Result<JsValue, VmError> {
        self.create_array_from_values(keys.into_iter().map(JsValue::String).collect())
    }

    fn create_array_from_values(&mut self, values: Vec<JsValue>) -> Result<JsValue, VmError> {
        let array = self.create_array_value();
        let array_id = match array {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        let array_object = self
            .objects
            .get_mut(&array_id)
            .ok_or(VmError::UnknownObject(array_id))?;
        for (index, value) in values.iter().enumerate() {
            let index_key = index.to_string();
            array_object
                .properties
                .insert(index_key.clone(), value.clone());
            array_object
                .property_attributes
                .entry(index_key)
                .or_insert_with(PropertyAttributes::default);
        }
        array_object
            .properties
            .insert("length".to_string(), JsValue::Number(values.len() as f64));
        array_object.property_attributes.insert(
            "length".to_string(),
            PropertyAttributes {
                writable: true,
                enumerable: false,
                configurable: false,
            },
        );
        Ok(JsValue::Object(array_id))
    }

    fn execute_object_get_own_property_descriptor(
        &mut self,
        args: &[JsValue],
    ) -> Result<JsValue, VmError> {
        let target = args.first().cloned().unwrap_or(JsValue::Undefined);
        let property = args
            .get(1)
            .map(|value| self.coerce_to_property_key(value))
            .unwrap_or_default();
        match target {
            JsValue::Object(target_id) => {
                self.execute_object_get_own_property_descriptor_for_object(target_id, &property)
            }
            JsValue::Function(closure_id) => {
                self.execute_object_get_own_property_descriptor_for_function(closure_id, &property)
            }
            JsValue::NativeFunction(native) => {
                self.execute_object_get_own_property_descriptor_for_native(native, &property)
            }
            JsValue::HostFunction(host_id) => {
                self.execute_object_get_own_property_descriptor_for_host(host_id, &property)
            }
            _ => Err(VmError::TypeError(
                "Object.getOwnPropertyDescriptor target must be object",
            )),
        }
    }

    fn execute_object_get_own_property_descriptor_for_object(
        &mut self,
        target_id: ObjectId,
        property: &str,
    ) -> Result<JsValue, VmError> {
        let (data_value, getter_value, setter_value, attributes, is_arguments_like) = {
            let object = self
                .objects
                .get(&target_id)
                .ok_or(VmError::UnknownObject(target_id))?;
            (
                object.properties.get(property).cloned(),
                object.getters.get(property).cloned(),
                object.setters.get(property).cloned(),
                object.property_attributes.get(property).copied(),
                object.properties.contains_key("callee")
                    && object.properties.contains_key("length"),
            )
        };

        if let Some(value) = data_value {
            let attributes = attributes.unwrap_or(PropertyAttributes {
                writable: true,
                enumerable: !(is_arguments_like && matches!(property, "length" | "callee")),
                configurable: true,
            });
            return Ok(self.create_descriptor_object(vec![
                ("value", value),
                ("writable", JsValue::Bool(attributes.writable)),
                ("enumerable", JsValue::Bool(attributes.enumerable)),
                ("configurable", JsValue::Bool(attributes.configurable)),
            ]));
        }
        if getter_value.is_some() || setter_value.is_some() {
            let attributes = attributes.unwrap_or(PropertyAttributes {
                writable: false,
                enumerable: true,
                configurable: true,
            });
            return Ok(self.create_descriptor_object(vec![
                ("get", getter_value.unwrap_or(JsValue::Undefined)),
                ("set", setter_value.unwrap_or(JsValue::Undefined)),
                ("enumerable", JsValue::Bool(attributes.enumerable)),
                ("configurable", JsValue::Bool(attributes.configurable)),
            ]));
        }
        Ok(JsValue::Undefined)
    }

    fn execute_object_get_own_property_descriptor_for_function(
        &mut self,
        closure_id: u64,
        property: &str,
    ) -> Result<JsValue, VmError> {
        if !self.closures.contains_key(&closure_id) {
            return Err(VmError::UnknownClosure(closure_id));
        }

        let (data_value, getter_value, setter_value, attributes) = self
            .closure_objects
            .get(&closure_id)
            .map(|object| {
                (
                    object.properties.get(property).cloned(),
                    object.getters.get(property).cloned(),
                    object.setters.get(property).cloned(),
                    object.property_attributes.get(property).copied(),
                )
            })
            .unwrap_or((None, None, None, None));

        if let Some(value) = data_value {
            let attributes = attributes.unwrap_or(PropertyAttributes::default());
            return Ok(self.create_descriptor_object(vec![
                ("value", value),
                ("writable", JsValue::Bool(attributes.writable)),
                ("enumerable", JsValue::Bool(attributes.enumerable)),
                ("configurable", JsValue::Bool(attributes.configurable)),
            ]));
        }
        if getter_value.is_some() || setter_value.is_some() {
            let attributes = attributes.unwrap_or(PropertyAttributes {
                writable: false,
                enumerable: true,
                configurable: true,
            });
            return Ok(self.create_descriptor_object(vec![
                ("get", getter_value.unwrap_or(JsValue::Undefined)),
                ("set", setter_value.unwrap_or(JsValue::Undefined)),
                ("enumerable", JsValue::Bool(attributes.enumerable)),
                ("configurable", JsValue::Bool(attributes.configurable)),
            ]));
        }

        if property == "length" {
            let closure = self
                .closures
                .get(&closure_id)
                .ok_or(VmError::UnknownClosure(closure_id))?;
            let function = closure
                .functions
                .get(closure.function_id)
                .ok_or(VmError::UnknownFunction(closure.function_id))?;
            return Ok(self.create_descriptor_object(vec![
                ("value", JsValue::Number(function.length as f64)),
                ("writable", JsValue::Bool(false)),
                ("enumerable", JsValue::Bool(false)),
                ("configurable", JsValue::Bool(true)),
            ]));
        }

        if property == "prototype"
            && !self.closure_is_arrow(closure_id)?
            && !self.closure_has_no_prototype(closure_id)
        {
            let prototype_value = self.get_or_create_function_prototype_property(closure_id)?;
            return Ok(self.create_descriptor_object(vec![
                ("value", prototype_value),
                ("writable", JsValue::Bool(true)),
                ("enumerable", JsValue::Bool(false)),
                ("configurable", JsValue::Bool(false)),
            ]));
        }

        Ok(JsValue::Undefined)
    }

    fn execute_object_get_own_property_descriptor_for_native(
        &mut self,
        native: NativeFunction,
        property: &str,
    ) -> Result<JsValue, VmError> {
        if !self.native_function_has_own_property(native, property) {
            return Ok(JsValue::Undefined);
        }
        let value = self.get_native_function_property(native, property);
        Ok(self.create_descriptor_object(vec![
            ("value", value),
            ("writable", JsValue::Bool(true)),
            ("enumerable", JsValue::Bool(false)),
            ("configurable", JsValue::Bool(true)),
        ]))
    }

    fn execute_object_get_own_property_descriptor_for_host(
        &mut self,
        host_id: u64,
        property: &str,
    ) -> Result<JsValue, VmError> {
        if !self.host_functions.contains_key(&host_id) {
            return Err(VmError::UnknownHostFunction(host_id));
        }

        let (data_value, getter_value, setter_value, attributes) = self
            .host_function_objects
            .get(&host_id)
            .map(|object| {
                (
                    object.properties.get(property).cloned(),
                    object.getters.get(property).cloned(),
                    object.setters.get(property).cloned(),
                    object.property_attributes.get(property).copied(),
                )
            })
            .unwrap_or((None, None, None, None));

        if let Some(value) = data_value {
            let attributes = attributes.unwrap_or(PropertyAttributes::default());
            return Ok(self.create_descriptor_object(vec![
                ("value", value),
                ("writable", JsValue::Bool(attributes.writable)),
                ("enumerable", JsValue::Bool(attributes.enumerable)),
                ("configurable", JsValue::Bool(attributes.configurable)),
            ]));
        }
        if getter_value.is_some() || setter_value.is_some() {
            let attributes = attributes.unwrap_or(PropertyAttributes {
                writable: false,
                enumerable: true,
                configurable: true,
            });
            return Ok(self.create_descriptor_object(vec![
                ("get", getter_value.unwrap_or(JsValue::Undefined)),
                ("set", setter_value.unwrap_or(JsValue::Undefined)),
                ("enumerable", JsValue::Bool(attributes.enumerable)),
                ("configurable", JsValue::Bool(attributes.configurable)),
            ]));
        }

        if !Self::host_function_has_default_property(property) {
            return Ok(JsValue::Undefined);
        }

        let empty_realm = Realm::default();
        let value = self.get_host_function_property(host_id, property, &empty_realm)?;
        Ok(self.create_descriptor_object(vec![
            ("value", value),
            ("writable", JsValue::Bool(true)),
            ("enumerable", JsValue::Bool(false)),
            ("configurable", JsValue::Bool(true)),
        ]))
    }

    fn execute_object_get_prototype_of(&mut self, args: &[JsValue]) -> Result<JsValue, VmError> {
        let target = args.first().cloned().unwrap_or(JsValue::Undefined);
        match target {
            JsValue::Object(target_id) => {
                let object = self
                    .objects
                    .get(&target_id)
                    .ok_or(VmError::UnknownObject(target_id))?;
                Ok(object
                    .prototype_value
                    .clone()
                    .or_else(|| object.prototype.map(JsValue::Object))
                    .unwrap_or(JsValue::Null))
            }
            value @ JsValue::Function(_) => self.get_prototype_of_value(&value),
            value @ (JsValue::NativeFunction(_) | JsValue::HostFunction(_)) => {
                self.get_prototype_of_value(&value)
            }
            _ => Err(VmError::TypeError(
                "Object.getPrototypeOf target must be object",
            )),
        }
    }

    fn execute_object_prevent_extensions(&mut self, args: &[JsValue]) -> Result<JsValue, VmError> {
        let target = args.first().cloned().unwrap_or(JsValue::Undefined);
        if let JsValue::Object(object_id) = target {
            let object = self
                .objects
                .get_mut(&object_id)
                .ok_or(VmError::UnknownObject(object_id))?;
            object.extensible = false;
            Ok(JsValue::Object(object_id))
        } else {
            Ok(target)
        }
    }

    fn execute_object_is_extensible(&mut self, args: &[JsValue]) -> Result<JsValue, VmError> {
        let target = args.first().cloned().unwrap_or(JsValue::Undefined);
        let extensible = match target {
            JsValue::Object(object_id) => self
                .objects
                .get(&object_id)
                .map(|object| object.extensible)
                .unwrap_or(false),
            JsValue::Function(_) | JsValue::NativeFunction(_) | JsValue::HostFunction(_) => true,
            _ => false,
        };
        Ok(JsValue::Bool(extensible))
    }

    fn execute_object_freeze(&mut self, args: &[JsValue]) -> Result<JsValue, VmError> {
        let target = args.first().cloned().unwrap_or(JsValue::Undefined);
        if let JsValue::Object(object_id) = target {
            let object = self
                .objects
                .get_mut(&object_id)
                .ok_or(VmError::UnknownObject(object_id))?;
            object.extensible = false;

            let mut keys = BTreeSet::new();
            keys.extend(object.properties.keys().cloned());
            keys.extend(object.getters.keys().cloned());
            keys.extend(object.setters.keys().cloned());
            for key in keys {
                let is_data = object.properties.contains_key(&key);
                let attributes = object.property_attributes.entry(key).or_default();
                attributes.configurable = false;
                if is_data {
                    attributes.writable = false;
                }
            }

            Ok(JsValue::Object(object_id))
        } else {
            Ok(target)
        }
    }

    fn execute_object_get_template_object(&mut self, args: &[JsValue]) -> Result<JsValue, VmError> {
        let site_id = args
            .first()
            .map(|value| self.to_number(value))
            .filter(|value| value.is_finite() && *value >= 0.0)
            .unwrap_or(0.0) as u64;
        if let Some(cached) = self.template_cache.get(&site_id).cloned() {
            return Ok(cached);
        }

        let cooked = args.get(1).cloned().unwrap_or(JsValue::Undefined);
        let raw = args.get(2).cloned().unwrap_or(JsValue::Undefined);
        let cooked_id = match cooked {
            JsValue::Object(object_id) => object_id,
            _ => return Err(VmError::TypeError("template cooked object must be object")),
        };
        let raw_id = match raw {
            JsValue::Object(object_id) => object_id,
            _ => return Err(VmError::TypeError("template raw object must be object")),
        };
        {
            let cooked_object = self
                .objects
                .get_mut(&cooked_id)
                .ok_or(VmError::UnknownObject(cooked_id))?;
            cooked_object
                .properties
                .insert("raw".to_string(), JsValue::Object(raw_id));
            cooked_object.property_attributes.insert(
                "raw".to_string(),
                PropertyAttributes {
                    writable: false,
                    enumerable: false,
                    configurable: false,
                },
            );
        }
        let _ = self.execute_object_freeze(&[JsValue::Object(raw_id)])?;
        let template_object = self.execute_object_freeze(&[JsValue::Object(cooked_id)])?;
        self.template_cache.insert(site_id, template_object.clone());
        Ok(template_object)
    }

    fn create_descriptor_object(&mut self, entries: Vec<(&str, JsValue)>) -> JsValue {
        let descriptor = self.create_object_value();
        let object_id = match descriptor {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        let object = self
            .objects
            .get_mut(&object_id)
            .expect("descriptor object should exist");
        for (key, value) in entries {
            object.properties.insert(key.to_string(), value);
        }
        JsValue::Object(object_id)
    }

    fn execute_inline_chunk(
        &mut self,
        chunk: &Chunk,
        realm: &Realm,
        force_strict: bool,
    ) -> Result<JsValue, VmError> {
        let stack_depth = self.stack.len();
        let saved_handlers = std::mem::take(&mut self.exception_handlers);
        let saved_pending_exception = self.pending_exception.take();
        let saved_identifier_references = std::mem::take(&mut self.identifier_references);
        self.exception_handlers = Vec::new();
        self.pending_exception = None;
        let strict = force_strict || self.code_is_strict(&chunk.code);
        let result = match self.execute_code(&chunk.code, &chunk.functions, realm, false, strict) {
            Ok(ExecutionSignal::Halt) => Ok(self.stack.pop().unwrap_or(JsValue::Undefined)),
            Ok(ExecutionSignal::Return) => Err(VmError::TopLevelReturn),
            Err(err) => Err(err),
        };
        self.exception_handlers = saved_handlers;
        self.pending_exception = saved_pending_exception;
        self.identifier_references = saved_identifier_references;
        self.stack.truncate(stack_depth);
        result
    }

    fn route_runtime_error_to_handler(
        &mut self,
        err: VmError,
        code_len: usize,
    ) -> Result<usize, VmError> {
        match err {
            VmError::UncaughtException(exception) => self.throw_to_handler(exception, code_len),
            other => {
                if self.exception_handlers.is_empty() {
                    return Err(other);
                }
                if let Some(exception) = self.runtime_error_exception_value(&other) {
                    self.throw_to_handler(exception, code_len)
                } else {
                    Err(other)
                }
            }
        }
    }

    fn runtime_error_exception_value(&mut self, err: &VmError) -> Option<JsValue> {
        match err {
            VmError::UnknownIdentifier(name) => Some(self.create_error_exception(
                NativeFunction::ReferenceErrorConstructor,
                "ReferenceError",
                format!("{name} is not defined"),
            )),
            VmError::TypeError(message) => Some(self.create_error_exception(
                NativeFunction::TypeErrorConstructor,
                "TypeError",
                (*message).to_string(),
            )),
            VmError::NotCallable => Some(self.create_error_exception(
                NativeFunction::TypeErrorConstructor,
                "TypeError",
                "NotCallable".to_string(),
            )),
            VmError::ImmutableBinding(name) => Some(self.create_error_exception(
                NativeFunction::TypeErrorConstructor,
                "TypeError",
                format!("immutable binding '{name}'"),
            )),
            VmError::VariableAlreadyDefined(name) => Some(self.create_error_exception(
                NativeFunction::SyntaxErrorConstructor,
                "SyntaxError",
                format!("Identifier '{name}' has already been declared"),
            )),
            _ => None,
        }
    }

    fn create_error_exception(
        &mut self,
        constructor: NativeFunction,
        name: &str,
        message: String,
    ) -> JsValue {
        let error = self.create_object_value();
        let JsValue::Object(object_id) = error else {
            unreachable!();
        };
        let prototype = self.error_prototype_for_constructor(constructor);
        if let Some(object) = self.objects.get_mut(&object_id) {
            object.prototype = prototype;
            object.prototype_value = None;
            object.properties.insert(
                "constructor".to_string(),
                JsValue::NativeFunction(constructor),
            );
            object
                .properties
                .insert("name".to_string(), JsValue::String(name.to_string()));
            object
                .properties
                .insert("message".to_string(), JsValue::String(message));
            object.property_attributes.insert(
                "constructor".to_string(),
                PropertyAttributes {
                    writable: true,
                    enumerable: false,
                    configurable: true,
                },
            );
            object.property_attributes.insert(
                "name".to_string(),
                PropertyAttributes {
                    writable: true,
                    enumerable: false,
                    configurable: true,
                },
            );
            object.property_attributes.insert(
                "message".to_string(),
                PropertyAttributes {
                    writable: true,
                    enumerable: false,
                    configurable: true,
                },
            );
        }
        JsValue::Object(object_id)
    }

    fn throw_to_handler(&mut self, exception: JsValue, code_len: usize) -> Result<usize, VmError> {
        while let Some(handler) = self.exception_handlers.pop() {
            self.unwind_to(handler.scope_depth, handler.stack_depth, handler.with_depth)?;

            if let Some(catch_target) = handler.catch_target {
                if catch_target >= code_len {
                    return Err(VmError::InvalidJump(catch_target));
                }
                self.pending_exception = Some(exception);
                return Ok(catch_target);
            }
            if let Some(finally_target) = handler.finally_target {
                if finally_target >= code_len {
                    return Err(VmError::InvalidJump(finally_target));
                }
                self.pending_exception = Some(exception);
                return Ok(finally_target);
            }
        }
        Err(VmError::UncaughtException(exception))
    }

    fn unwind_to(
        &mut self,
        scope_depth: usize,
        stack_depth: usize,
        with_depth: usize,
    ) -> Result<(), VmError> {
        while self.scopes.len() > scope_depth {
            self.scopes.pop();
        }
        if self.scopes.is_empty() {
            return Err(VmError::ScopeUnderflow);
        }
        if self.stack.len() < stack_depth {
            return Err(VmError::StackUnderflow);
        }
        self.stack.truncate(stack_depth);
        while self.with_objects.len() > with_depth {
            self.with_objects.pop();
        }
        Ok(())
    }

    fn create_binding(&mut self, value: JsValue, mutable: bool) -> BindingId {
        self.create_binding_with_flags(value, mutable, false)
    }

    fn create_binding_with_flags(
        &mut self,
        value: JsValue,
        mutable: bool,
        deletable: bool,
    ) -> BindingId {
        self.create_binding_with_behavior(value, mutable, deletable, false)
    }

    fn create_binding_with_behavior(
        &mut self,
        value: JsValue,
        mutable: bool,
        deletable: bool,
        sloppy_readonly_write_ignored: bool,
    ) -> BindingId {
        let id = self.next_binding_id;
        self.next_binding_id += 1;
        self.bindings.insert(
            id,
            Binding {
                value,
                mutable,
                deletable,
                sloppy_readonly_write_ignored,
            },
        );
        id
    }

    fn create_object_value(&mut self) -> JsValue {
        let id = self.allocate_object_id();
        self.objects.insert(
            id,
            JsObject {
                prototype: self.object_prototype_id,
                ..JsObject::default()
            },
        );
        self.gc_peak_objects = self.gc_peak_objects.max(self.objects.len());
        JsValue::Object(id)
    }

    fn create_array_value(&mut self) -> JsValue {
        let array = self.create_object_value();
        let JsValue::Object(object_id) = array else {
            unreachable!();
        };
        if self.array_prototype_id.is_none() {
            let _ = self.array_prototype_value();
        }
        if let Some(object) = self.objects.get_mut(&object_id) {
            object.prototype = self.array_prototype_id;
            object.prototype_value = None;
            object.properties.insert(
                "constructor".to_string(),
                JsValue::NativeFunction(NativeFunction::ArrayConstructor),
            );
            object.property_attributes.insert(
                "constructor".to_string(),
                PropertyAttributes {
                    writable: true,
                    enumerable: false,
                    configurable: true,
                },
            );
            object
                .properties
                .insert("length".to_string(), JsValue::Number(0.0));
            object.property_attributes.insert(
                "length".to_string(),
                PropertyAttributes {
                    writable: true,
                    enumerable: false,
                    configurable: false,
                },
            );
        }
        JsValue::Object(object_id)
    }

    fn instantiate_function(
        &mut self,
        function_id: usize,
        functions: &[CompiledFunction],
        enclosing_strict: bool,
    ) -> Result<JsValue, VmError> {
        let function = functions
            .get(function_id)
            .ok_or(VmError::UnknownFunction(function_id))?;
        let strict = enclosing_strict || self.function_is_strict(function);
        let closure_id = self.next_closure_id;
        self.next_closure_id += 1;
        let mut captured_scopes = self.scopes.clone();
        let captured_with_objects = self.with_objects.clone();
        if self.function_is_named_function_expression(function) && function.name != "<anonymous>" {
            let mut name_scope = BTreeMap::new();
            let function_name_binding = self.create_binding_with_behavior(
                JsValue::Function(closure_id),
                false,
                false,
                true,
            );
            name_scope.insert(function.name.clone(), function_name_binding);
            captured_scopes.push(Rc::new(RefCell::new(name_scope)));
        }
        self.closures.insert(
            closure_id,
            Closure {
                function_id,
                functions: Rc::new(functions.to_vec()),
                captured_scopes,
                captured_with_objects,
                strict,
            },
        );
        Ok(JsValue::Function(closure_id))
    }

    fn current_scope_ref(&self) -> Result<ScopeRef, VmError> {
        self.scopes.last().cloned().ok_or(VmError::ScopeUnderflow)
    }

    fn current_var_scope_ref(&self) -> Result<ScopeRef, VmError> {
        let Some(scope_index) = self.var_scope_stack.last().copied() else {
            return self.current_scope_ref();
        };
        self.scopes
            .get(scope_index)
            .cloned()
            .ok_or(VmError::ScopeUnderflow)
    }

    fn resolve_binding_id(&self, name: &str) -> Option<BindingId> {
        for scope_ref in self.scopes.iter().rev() {
            if let Some(binding_id) = scope_ref.borrow().get(name).copied() {
                return Some(binding_id);
            }
        }
        None
    }

    fn delete_binding_reference(&mut self, binding_id: BindingId) -> bool {
        let deletable = self
            .bindings
            .get(&binding_id)
            .is_some_and(|binding| binding.deletable);
        if !deletable {
            return false;
        }
        for scope_ref in &self.scopes {
            scope_ref
                .borrow_mut()
                .retain(|_, current_id| *current_id != binding_id);
        }
        self.bindings.remove(&binding_id);
        true
    }

    fn with_object_from_value(&mut self, value: JsValue) -> Result<JsValue, VmError> {
        match value {
            JsValue::Undefined | JsValue::Uninitialized | JsValue::Null => {
                Err(VmError::TypeError("with expects object"))
            }
            JsValue::Object(_)
            | JsValue::Function(_)
            | JsValue::NativeFunction(_)
            | JsValue::HostFunction(_)
            | JsValue::String(_) => Ok(value),
            primitive @ (JsValue::Number(_) | JsValue::Bool(_)) => {
                let object = self.create_object_value();
                let object_id = match object {
                    JsValue::Object(id) => id,
                    _ => unreachable!(),
                };
                let target = self
                    .objects
                    .get_mut(&object_id)
                    .ok_or(VmError::UnknownObject(object_id))?;
                target
                    .properties
                    .insert(BOXED_PRIMITIVE_VALUE_KEY.to_string(), primitive);
                Ok(JsValue::Object(object_id))
            }
        }
    }

    fn has_property_on_receiver(
        &mut self,
        receiver: &JsValue,
        property: &str,
        _realm: &Realm,
    ) -> Result<bool, VmError> {
        match receiver {
            JsValue::Object(object_id) => {
                let has_property =
                    self.object_has_property_in_chain(*object_id, property, _realm)?;
                if has_property {
                    return Ok(true);
                }
                let object = self
                    .objects
                    .get(object_id)
                    .ok_or(VmError::UnknownObject(*object_id))?;
                Ok(matches!(property, "hasOwnProperty" | "isPrototypeOf")
                    || (property == "push" && object.properties.contains_key("length"))
                    || (property == "forEach" && object.properties.contains_key("length"))
                    || (property == "reduce" && object.properties.contains_key("length"))
                    || (property == "join" && object.properties.contains_key("length"))
                    || (property == "reverse" && object.properties.contains_key("length"))
                    || (property == "sort" && object.properties.contains_key("length")))
            }
            JsValue::Function(closure_id) => {
                if self.closure_has_own_property(*closure_id, property) {
                    return Ok(true);
                }
                if property == "prototype" {
                    return Ok(!self.closure_is_arrow(*closure_id)?
                        && !self.closure_has_no_prototype(*closure_id));
                }
                Ok(matches!(
                    property,
                    "length"
                        | "call"
                        | "apply"
                        | "bind"
                        | "toString"
                        | "valueOf"
                        | "hasOwnProperty"
                        | "isPrototypeOf"
                        | "constructor"
                ))
            }
            JsValue::NativeFunction(native) => Ok(!matches!(
                self.get_native_function_property(*native, property),
                JsValue::Undefined
            )),
            JsValue::HostFunction(host_id) => {
                if !self.host_functions.contains_key(host_id) {
                    return Err(VmError::UnknownHostFunction(*host_id));
                }
                if self
                    .host_function_objects
                    .get(host_id)
                    .is_some_and(|object| {
                        object.properties.contains_key(property)
                            || object.getters.contains_key(property)
                            || object.setters.contains_key(property)
                    })
                {
                    return Ok(true);
                }
                if property == "isPrototypeOf" {
                    return Ok(true);
                }
                Ok(Self::host_function_has_default_property(property))
            }
            JsValue::String(_receiver) => {
                if property == "length" {
                    return Ok(true);
                }
                if property.parse::<usize>().is_ok() {
                    return Ok(true);
                }
                Ok(matches!(
                    property,
                    "replace"
                        | "match"
                        | "search"
                        | "split"
                        | "indexOf"
                        | "lastIndexOf"
                        | "substring"
                        | "toLowerCase"
                        | "toUpperCase"
                        | "trim"
                        | "toString"
                        | "valueOf"
                        | "charAt"
                        | "charCodeAt"
                ))
            }
            JsValue::Undefined
            | JsValue::Uninitialized
            | JsValue::Null
            | JsValue::Number(_)
            | JsValue::Bool(_) => Ok(false),
        }
    }

    fn object_has_property_in_chain(
        &mut self,
        object_id: ObjectId,
        property: &str,
        realm: &Realm,
    ) -> Result<bool, VmError> {
        let mut current = Some(object_id);
        while let Some(id) = current {
            let object = self.objects.get(&id).ok_or(VmError::UnknownObject(id))?;
            if object.properties.contains_key(property)
                || object.getters.contains_key(property)
                || object.setters.contains_key(property)
            {
                return Ok(true);
            }
            if let Some(next) = object.prototype {
                current = Some(next);
                continue;
            }
            if let Some(prototype_value) = object.prototype_value.clone() {
                return self.has_property_on_receiver(&prototype_value, property, realm);
            }
            current = None;
        }
        Ok(false)
    }

    fn get_property_from_receiver(
        &mut self,
        receiver: JsValue,
        property: &str,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        match receiver {
            JsValue::Object(object_id) => self.get_object_property(object_id, property, realm),
            JsValue::Function(closure_id) => {
                self.get_function_property(closure_id, property, realm)
            }
            JsValue::NativeFunction(native) => {
                if Self::is_restricted_function_property(property) {
                    return Err(VmError::TypeError("restricted function property access"));
                }
                Ok(self.get_native_function_property(native, property))
            }
            JsValue::HostFunction(host_id) => {
                if Self::is_restricted_function_property(property) {
                    return Err(VmError::TypeError("restricted function property access"));
                }
                self.get_host_function_property(host_id, property, realm)
            }
            JsValue::String(receiver) => Ok(self.get_string_property(&receiver, property)),
            primitive @ (JsValue::Number(_) | JsValue::Bool(_)) => {
                let boxed_receiver = self.box_primitive_receiver(primitive);
                let JsValue::Object(object_id) = boxed_receiver.clone() else {
                    unreachable!();
                };
                self.get_object_property_with_receiver(object_id, property, boxed_receiver, realm)
            }
            _ => Err(VmError::TypeError("property access expects object")),
        }
    }

    fn set_property_on_receiver(
        &mut self,
        receiver: JsValue,
        property: String,
        value: JsValue,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        match receiver {
            JsValue::Object(object_id) => {
                self.set_object_property(object_id, property, value, realm)
            }
            JsValue::Function(closure_id) => {
                if self.function_rejects_caller_arguments(closure_id)?
                    && matches!(property.as_str(), "caller" | "arguments")
                    && !self.closure_has_own_property(closure_id, &property)
                {
                    return Err(VmError::TypeError("restricted function property access"));
                }
                self.set_function_property(closure_id, property, value, realm)
            }
            JsValue::NativeFunction(_) | JsValue::HostFunction(_) => {
                if Self::is_restricted_function_property(&property) {
                    return Err(VmError::TypeError("restricted function property access"));
                }
                Ok(value)
            }
            _ => Err(VmError::TypeError("property write expects object")),
        }
    }

    fn get_property_from_base_with_receiver(
        &mut self,
        base: JsValue,
        property: &str,
        receiver: JsValue,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        match base {
            JsValue::Object(object_id) => {
                self.get_object_property_with_receiver(object_id, property, receiver, realm)
            }
            JsValue::Function(closure_id) => {
                self.get_function_property_with_receiver(closure_id, property, receiver, realm)
            }
            JsValue::NativeFunction(native) => {
                if Self::is_restricted_function_property(property) {
                    return Err(VmError::TypeError("restricted function property access"));
                }
                Ok(self.get_native_function_property(native, property))
            }
            JsValue::HostFunction(host_id) => {
                if Self::is_restricted_function_property(property) {
                    return Err(VmError::TypeError("restricted function property access"));
                }
                self.get_host_function_property(host_id, property, realm)
            }
            JsValue::String(base_string) => Ok(self.get_string_property(&base_string, property)),
            _ => Err(VmError::TypeError("property access expects object")),
        }
    }

    fn set_property_on_base_with_receiver(
        &mut self,
        base: JsValue,
        property: String,
        value: JsValue,
        receiver: JsValue,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        match base {
            JsValue::Object(object_id) => {
                self.set_object_property_with_receiver(object_id, property, value, receiver, realm)
            }
            JsValue::Function(_)
            | JsValue::NativeFunction(_)
            | JsValue::HostFunction(_)
            | JsValue::String(_) => self.set_property_on_receiver(receiver, property, value, realm),
            _ => Err(VmError::TypeError("property write expects object")),
        }
    }

    fn get_object_property_with_receiver(
        &mut self,
        object_id: ObjectId,
        property: &str,
        receiver: JsValue,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        if property == "hasOwnProperty" {
            return Ok(
                self.create_host_function_value(HostFunction::HasOwnProperty {
                    target: JsValue::Object(object_id),
                }),
            );
        }
        if property == "isPrototypeOf" {
            return Ok(
                self.create_host_function_value(HostFunction::IsPrototypeOf {
                    target: JsValue::Object(object_id),
                }),
            );
        }
        if let Some(JsValue::String(value)) = self.boxed_primitive_value(object_id) {
            if property == "length" {
                return Ok(JsValue::Number(Self::utf16_code_unit_length(&value) as f64));
            }
            if let Ok(index) = property.parse::<usize>() {
                return Ok(value
                    .chars()
                    .nth(index)
                    .map(|ch| JsValue::String(ch.to_string()))
                    .unwrap_or(JsValue::Undefined));
            }
        }
        if property == "push"
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.properties.contains_key("length"))
        {
            return Ok(self.create_host_function_value(HostFunction::ArrayPush(object_id)));
        }
        if property == "pop"
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.properties.contains_key("length"))
        {
            return Ok(self.create_host_function_value(HostFunction::ArrayPopThis));
        }
        if property == "concat"
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.properties.contains_key("length"))
        {
            return Ok(self.create_host_function_value(HostFunction::ArrayConcatThis));
        }
        if property == "forEach"
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.properties.contains_key("length"))
        {
            return Ok(self.create_host_function_value(HostFunction::ArrayForEach(object_id)));
        }
        if property == "reduce"
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.properties.contains_key("length"))
        {
            return Ok(self.create_host_function_value(HostFunction::ArrayReduce(object_id)));
        }
        if property == "join"
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.properties.contains_key("length"))
        {
            return Ok(self.create_host_function_value(HostFunction::ArrayJoin(object_id)));
        }
        if property == "reverse"
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.properties.contains_key("length"))
        {
            return Ok(self.create_host_function_value(HostFunction::ArrayReverse(object_id)));
        }
        if property == "sort"
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.properties.contains_key("length"))
        {
            return Ok(self.create_host_function_value(HostFunction::ArraySort(object_id)));
        }
        if property == "keys"
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.properties.contains_key("length"))
        {
            return Ok(self.create_host_function_value(HostFunction::ArrayKeysThis));
        }
        if property == "entries"
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.properties.contains_key("length"))
        {
            return Ok(self.create_host_function_value(HostFunction::ArrayEntriesThis));
        }
        if (property == "values" || property == "Symbol.iterator")
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.properties.contains_key("length"))
        {
            return Ok(self.create_host_function_value(HostFunction::ArrayValuesThis));
        }
        let mapped_binding = self
            .objects
            .get(&object_id)
            .and_then(|object| object.argument_mappings.get(property).copied());
        if let Some(binding_id) = mapped_binding {
            if let Some(binding) = self.bindings.get(&binding_id) {
                return Ok(binding.value.clone());
            }
        }
        let mut current_id = Some(object_id);
        while let Some(id) = current_id {
            let (getter, value, next, prototype_value) = {
                let object = self.objects.get(&id).ok_or(VmError::UnknownObject(id))?;
                (
                    object.getters.get(property).cloned(),
                    object.properties.get(property).cloned(),
                    object.prototype,
                    object.prototype_value.clone(),
                )
            };
            if let Some(getter) = getter {
                return self.execute_callable(
                    getter,
                    Some(receiver.clone()),
                    Vec::new(),
                    realm,
                    false,
                );
            }
            if let Some(value) = value {
                return Ok(value);
            }
            if let Some(next) = next {
                current_id = Some(next);
                continue;
            }
            if let Some(prototype_value) = prototype_value {
                return self.get_property_from_base_with_receiver(
                    prototype_value,
                    property,
                    receiver.clone(),
                    realm,
                );
            }
            current_id = None;
        }
        Ok(JsValue::Undefined)
    }

    fn set_object_property_with_receiver(
        &mut self,
        object_id: ObjectId,
        property: String,
        value: JsValue,
        receiver: JsValue,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let (own_setter, own_getter_exists, own_data_writable, own_prototype) = {
            let object = self
                .objects
                .get(&object_id)
                .ok_or(VmError::UnknownObject(object_id))?;
            (
                object.setters.get(&property).cloned(),
                object.getters.contains_key(&property),
                object.properties.get(&property).map(|_| {
                    object
                        .property_attributes
                        .get(&property)
                        .is_none_or(|attributes| attributes.writable)
                }),
                object.prototype,
            )
        };

        if let Some(setter) = own_setter {
            let _ =
                self.execute_callable(setter, Some(receiver), vec![value.clone()], realm, false)?;
            return Ok(value);
        }
        if own_getter_exists {
            return Ok(value);
        }
        if own_data_writable == Some(false) {
            return Ok(value);
        }

        if own_data_writable.is_none() {
            let mut current = own_prototype;
            while let Some(proto_id) = current {
                let (setter, getter_exists, data_writable, next) = {
                    let object = self
                        .objects
                        .get(&proto_id)
                        .ok_or(VmError::UnknownObject(proto_id))?;
                    (
                        object.setters.get(&property).cloned(),
                        object.getters.contains_key(&property),
                        object.properties.get(&property).map(|_| {
                            object
                                .property_attributes
                                .get(&property)
                                .is_none_or(|attributes| attributes.writable)
                        }),
                        object.prototype,
                    )
                };

                if let Some(setter) = setter {
                    let _ = self.execute_callable(
                        setter,
                        Some(receiver),
                        vec![value.clone()],
                        realm,
                        false,
                    )?;
                    return Ok(value);
                }
                if getter_exists {
                    return Ok(value);
                }
                if data_writable == Some(false) {
                    return Ok(value);
                }
                if data_writable == Some(true) {
                    break;
                }
                current = next;
            }
        }

        self.set_property_on_receiver(receiver, property, value, realm)
    }

    fn get_function_property_with_receiver(
        &mut self,
        closure_id: u64,
        property: &str,
        receiver: JsValue,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        if self.function_rejects_caller_arguments(closure_id)?
            && matches!(property, "caller" | "arguments")
            && !self.closure_has_own_property(closure_id, property)
        {
            return Err(VmError::TypeError("restricted function property access"));
        }
        if let Some(getter) = self
            .closure_objects
            .get(&closure_id)
            .and_then(|object| object.getters.get(property))
            .cloned()
        {
            return self.execute_callable(getter, Some(receiver), Vec::new(), realm, false);
        }
        self.get_function_property(closure_id, property, realm)
    }

    fn resolve_with_property_base_for_scope_depth(
        &mut self,
        name: &str,
        realm: &Realm,
        scope_depth: usize,
    ) -> Result<Option<JsValue>, VmError> {
        let candidates = self.with_objects.clone();
        for frame in candidates.iter().rev() {
            if frame.scope_depth == scope_depth
                && self.has_property_on_receiver(&frame.object, name, realm)?
            {
                return Ok(Some(frame.object.clone()));
            }
        }
        Ok(None)
    }

    fn resolve_binding_or_with_reference(
        &mut self,
        name: &str,
        realm: &Realm,
    ) -> Result<Option<IdentifierReference>, VmError> {
        let scope_count = self.scopes.len();
        for scope_index in (0..scope_count).rev() {
            let scope_depth_marker = scope_index + 1;
            if let Some(base) =
                self.resolve_with_property_base_for_scope_depth(name, realm, scope_depth_marker)?
            {
                return Ok(Some(IdentifierReference::Property {
                    base,
                    property: name.to_string(),
                    strict_on_missing: true,
                    with_base_object: true,
                }));
            }
            let scope_ref = self
                .scopes
                .get(scope_index)
                .cloned()
                .ok_or(VmError::ScopeUnderflow)?;
            if let Some(binding_id) = scope_ref.borrow().get(name).copied() {
                return Ok(Some(IdentifierReference::Binding {
                    name: name.to_string(),
                    binding_id,
                }));
            }
        }
        if let Some(base) = self.resolve_with_property_base_for_scope_depth(name, realm, 0)? {
            return Ok(Some(IdentifierReference::Property {
                base,
                property: name.to_string(),
                strict_on_missing: true,
                with_base_object: true,
            }));
        }
        Ok(None)
    }

    fn resolve_identifier_reference(
        &mut self,
        name: &str,
        realm: &Realm,
        _strict: bool,
    ) -> Result<IdentifierReference, VmError> {
        if let Some(reference) = self.resolve_binding_or_with_reference(name, realm)? {
            return Ok(reference);
        }
        if let Some(global_object_id) = self.global_object_id {
            let base = JsValue::Object(global_object_id);
            if self.has_property_on_receiver(&base, name, realm)? {
                return Ok(IdentifierReference::Property {
                    base,
                    property: name.to_string(),
                    strict_on_missing: true,
                    with_base_object: false,
                });
            }
            if let Some(value) = realm.resolve_identifier(name) {
                let _ =
                    self.set_object_property(global_object_id, name.to_string(), value, realm)?;
                return Ok(IdentifierReference::Property {
                    base,
                    property: name.to_string(),
                    strict_on_missing: false,
                    with_base_object: false,
                });
            }
        }
        Ok(IdentifierReference::Unresolvable {
            name: name.to_string(),
        })
    }

    fn load_identifier_reference_value(
        &mut self,
        reference: &IdentifierReference,
        realm: &Realm,
        _strict: bool,
    ) -> Result<JsValue, VmError> {
        match reference {
            IdentifierReference::Binding { name, binding_id } => {
                let binding = self
                    .bindings
                    .get(binding_id)
                    .ok_or(VmError::ScopeUnderflow)?;
                if matches!(binding.value, JsValue::Uninitialized) {
                    return Err(VmError::UnknownIdentifier(name.clone()));
                }
                Ok(binding.value.clone())
            }
            IdentifierReference::Property { base, property, .. } => {
                self.get_property_from_receiver(base.clone(), property, realm)
            }
            IdentifierReference::Unresolvable { name } => {
                if name == "super" {
                    if let Some(base) = self.resolve_super_base_value() {
                        return Ok(base);
                    }
                }
                if name == "undefined" {
                    return Ok(JsValue::Undefined);
                }
                if name == "NaN" {
                    return Ok(JsValue::Number(f64::NAN));
                }
                if name == "Infinity" {
                    return Ok(JsValue::Number(f64::INFINITY));
                }
                if name == "globalThis" {
                    return Ok(self.global_this_value());
                }
                if name == "Math" {
                    return self.math_object_value();
                }
                if name == "JSON" {
                    return self.json_object_value();
                }
                if name == "Reflect" {
                    return self.reflect_object_value();
                }
                if name == "this" {
                    return Ok(realm
                        .resolve_identifier(name)
                        .unwrap_or_else(|| self.global_this_value()));
                }
                if let Some(value) = realm.resolve_identifier(name) {
                    return Ok(value);
                }
                if let Some(global_object_id) = self.global_object_id {
                    let receiver = JsValue::Object(global_object_id);
                    if self.has_property_on_receiver(&receiver, name, realm)? {
                        return self.get_property_from_receiver(receiver, name, realm);
                    }
                }
                Err(VmError::UnknownIdentifier(name.clone()))
            }
        }
    }

    fn store_identifier_reference_value(
        &mut self,
        reference: IdentifierReference,
        value: JsValue,
        realm: &Realm,
        strict: bool,
    ) -> Result<JsValue, VmError> {
        match reference {
            IdentifierReference::Binding { name, binding_id } => {
                let should_write = {
                    let binding = self
                        .bindings
                        .get(&binding_id)
                        .ok_or(VmError::ScopeUnderflow)?;
                    if matches!(binding.value, JsValue::Uninitialized) {
                        return Err(VmError::UnknownIdentifier(name));
                    }
                    if !binding.mutable {
                        if binding.sloppy_readonly_write_ignored && !strict {
                            false
                        } else {
                            return Err(VmError::ImmutableBinding(name));
                        }
                    } else {
                        true
                    }
                };
                self.maybe_set_inferred_function_name(&value, &name);
                if should_write {
                    let binding = self
                        .bindings
                        .get_mut(&binding_id)
                        .ok_or(VmError::ScopeUnderflow)?;
                    binding.value = value.clone();
                    self.sync_global_property_from_binding(&name, binding_id)?;
                }
                Ok(value)
            }
            IdentifierReference::Property {
                base,
                property,
                strict_on_missing,
                ..
            } => {
                let still_exists = self.has_property_on_receiver(&base, &property, realm)?;
                if strict && strict_on_missing && !still_exists {
                    return Err(VmError::UnknownIdentifier(property));
                }
                self.set_property_on_receiver(base, property, value, realm)
            }
            IdentifierReference::Unresolvable { name } => {
                if strict {
                    return Err(VmError::UnknownIdentifier(name));
                }
                if let Some(global_object_id) = self.global_object_id {
                    let _ =
                        self.set_object_property(global_object_id, name, value.clone(), realm)?;
                } else {
                    let global_scope = self
                        .scopes
                        .first()
                        .cloned()
                        .ok_or(VmError::ScopeUnderflow)?;
                    let binding_id = self.create_binding(value.clone(), true);
                    global_scope.borrow_mut().insert(name, binding_id);
                }
                Ok(value)
            }
        }
    }

    fn seed_global_constant_properties(&mut self) -> Result<(), VmError> {
        let Some(global_object_id) = self.global_object_id else {
            return Ok(());
        };
        for (name, value) in [
            ("NaN", JsValue::Number(f64::NAN)),
            ("Infinity", JsValue::Number(f64::INFINITY)),
            ("undefined", JsValue::Undefined),
        ] {
            self.define_global_property_with_attributes(
                global_object_id,
                name,
                value,
                PropertyAttributes {
                    writable: false,
                    enumerable: false,
                    configurable: false,
                },
            )?;
        }
        Ok(())
    }

    fn seed_global_realm_properties(&mut self, realm: &Realm) -> Result<(), VmError> {
        let Some(global_object_id) = self.global_object_id else {
            return Ok(());
        };
        for (name, value) in realm.globals_entries() {
            if matches!(name, "NaN" | "Infinity" | "undefined") {
                continue;
            }
            self.define_global_property_with_attributes(
                global_object_id,
                name,
                value.clone(),
                PropertyAttributes {
                    writable: true,
                    enumerable: false,
                    configurable: true,
                },
            )?;
        }
        self.define_global_property_with_attributes(
            global_object_id,
            "globalThis",
            JsValue::Object(global_object_id),
            PropertyAttributes {
                writable: true,
                enumerable: false,
                configurable: true,
            },
        )?;
        Ok(())
    }

    fn define_global_property_with_attributes(
        &mut self,
        object_id: ObjectId,
        name: &str,
        value: JsValue,
        attributes: PropertyAttributes,
    ) -> Result<(), VmError> {
        let object = self
            .objects
            .get_mut(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        object.properties.insert(name.to_string(), value);
        object
            .property_attributes
            .insert(name.to_string(), attributes);
        Ok(())
    }

    fn define_global_var_property(&mut self, name: &str) -> Result<(), VmError> {
        if self.var_scope_stack.last().copied() != Some(0) {
            return Ok(());
        }
        let Some(global_object_id) = self.global_object_id else {
            return Ok(());
        };
        let binding_id = self
            .scopes
            .first()
            .and_then(|scope| scope.borrow().get(name).copied());
        let Some(binding_id) = binding_id else {
            return Ok(());
        };
        let value = self
            .bindings
            .get(&binding_id)
            .map(|binding| binding.value.clone())
            .unwrap_or(JsValue::Undefined);
        let already_defined = self
            .objects
            .get(&global_object_id)
            .is_some_and(|object| object.properties.contains_key(name));
        if already_defined {
            return Ok(());
        }
        let is_extensible = self
            .objects
            .get(&global_object_id)
            .is_some_and(|object| object.extensible);
        if !is_extensible {
            return Err(VmError::TypeError(
                "cannot declare global var on non-extensible global object",
            ));
        }
        self.define_global_property_with_attributes(
            global_object_id,
            name,
            value,
            PropertyAttributes {
                writable: true,
                enumerable: true,
                configurable: false,
            },
        )
    }

    fn sync_global_property_from_binding(
        &mut self,
        name: &str,
        binding_id: BindingId,
    ) -> Result<(), VmError> {
        let Some(global_object_id) = self.global_object_id else {
            return Ok(());
        };
        let is_global_binding = self
            .scopes
            .first()
            .is_some_and(|scope| scope.borrow().get(name) == Some(&binding_id));
        if !is_global_binding {
            return Ok(());
        }
        let value = self
            .bindings
            .get(&binding_id)
            .map(|binding| binding.value.clone())
            .ok_or(VmError::ScopeUnderflow)?;
        let object = self
            .objects
            .get_mut(&global_object_id)
            .ok_or(VmError::UnknownObject(global_object_id))?;
        if object.properties.contains_key(name) {
            object.properties.insert(name.to_string(), value);
        }
        Ok(())
    }

    fn sync_global_binding_from_property_write(
        &mut self,
        object_id: ObjectId,
        property: &str,
        value: &JsValue,
    ) -> Result<(), VmError> {
        if Some(object_id) != self.global_object_id {
            return Ok(());
        }
        let binding_id = self
            .scopes
            .first()
            .and_then(|scope| scope.borrow().get(property).copied());
        let Some(binding_id) = binding_id else {
            return Ok(());
        };
        let binding = self
            .bindings
            .get_mut(&binding_id)
            .ok_or(VmError::ScopeUnderflow)?;
        if binding.mutable {
            binding.value = value.clone();
        }
        Ok(())
    }

    fn global_this_value(&self) -> JsValue {
        self.global_object_id
            .map(JsValue::Object)
            .unwrap_or(JsValue::Undefined)
    }

    fn math_object_value(&mut self) -> Result<JsValue, VmError> {
        let global_object_id = self.global_object_id.ok_or(VmError::ScopeUnderflow)?;
        if let Some(existing) = self
            .objects
            .get(&global_object_id)
            .and_then(|object| object.properties.get("Math"))
            .cloned()
        {
            return Ok(existing);
        }

        let math = self.create_object_value();
        let math_id = match math {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        {
            let math_object = self
                .objects
                .get_mut(&math_id)
                .ok_or(VmError::UnknownObject(math_id))?;
            for (name, value) in [
                ("E", std::f64::consts::E),
                ("PI", std::f64::consts::PI),
                ("LN10", std::f64::consts::LN_10),
                ("LN2", std::f64::consts::LN_2),
                ("LOG10E", std::f64::consts::LOG10_E),
                ("LOG2E", std::f64::consts::LOG2_E),
                ("SQRT1_2", std::f64::consts::FRAC_1_SQRT_2),
                ("SQRT2", std::f64::consts::SQRT_2),
            ] {
                math_object
                    .properties
                    .insert(name.to_string(), JsValue::Number(value));
                math_object.property_attributes.insert(
                    name.to_string(),
                    PropertyAttributes {
                        writable: false,
                        enumerable: false,
                        configurable: false,
                    },
                );
            }
            for (name, native) in [
                ("abs", NativeFunction::MathAbs),
                ("acos", NativeFunction::MathAcos),
                ("asin", NativeFunction::MathAsin),
                ("atan", NativeFunction::MathAtan),
                ("atan2", NativeFunction::MathAtan2),
                ("ceil", NativeFunction::MathCeil),
                ("cos", NativeFunction::MathCos),
                ("exp", NativeFunction::MathExp),
                ("floor", NativeFunction::MathFloor),
                ("log", NativeFunction::MathLog),
                ("max", NativeFunction::MathMax),
                ("min", NativeFunction::MathMin),
                ("pow", NativeFunction::MathPow),
                ("random", NativeFunction::MathRandom),
                ("round", NativeFunction::MathRound),
                ("sin", NativeFunction::MathSin),
                ("sqrt", NativeFunction::MathSqrt),
                ("tan", NativeFunction::MathTan),
            ] {
                math_object
                    .properties
                    .insert(name.to_string(), JsValue::NativeFunction(native));
                math_object.property_attributes.insert(
                    name.to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
            }
        }

        let global_object = self
            .objects
            .get_mut(&global_object_id)
            .ok_or(VmError::UnknownObject(global_object_id))?;
        global_object
            .properties
            .insert("Math".to_string(), JsValue::Object(math_id));
        global_object.property_attributes.insert(
            "Math".to_string(),
            PropertyAttributes {
                writable: true,
                enumerable: false,
                configurable: true,
            },
        );
        Ok(JsValue::Object(math_id))
    }

    fn json_object_value(&mut self) -> Result<JsValue, VmError> {
        let global_object_id = self.global_object_id.ok_or(VmError::ScopeUnderflow)?;
        if let Some(existing) = self
            .objects
            .get(&global_object_id)
            .and_then(|object| object.properties.get("JSON"))
            .cloned()
        {
            return Ok(existing);
        }

        let json = self.create_object_value();
        let json_id = match json {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        let stringify = self.create_host_function_value(HostFunction::JsonStringify);
        let parse = self.create_host_function_value(HostFunction::JsonParse);
        {
            let json_object = self
                .objects
                .get_mut(&json_id)
                .ok_or(VmError::UnknownObject(json_id))?;
            json_object
                .properties
                .insert("stringify".to_string(), stringify);
            json_object.property_attributes.insert(
                "stringify".to_string(),
                PropertyAttributes {
                    writable: true,
                    enumerable: false,
                    configurable: true,
                },
            );
            json_object.properties.insert("parse".to_string(), parse);
            json_object.property_attributes.insert(
                "parse".to_string(),
                PropertyAttributes {
                    writable: true,
                    enumerable: false,
                    configurable: true,
                },
            );
        }

        let global_object = self
            .objects
            .get_mut(&global_object_id)
            .ok_or(VmError::UnknownObject(global_object_id))?;
        global_object
            .properties
            .insert("JSON".to_string(), JsValue::Object(json_id));
        global_object.property_attributes.insert(
            "JSON".to_string(),
            PropertyAttributes {
                writable: true,
                enumerable: false,
                configurable: true,
            },
        );
        Ok(JsValue::Object(json_id))
    }

    fn reflect_object_value(&mut self) -> Result<JsValue, VmError> {
        let global_object_id = self.global_object_id.ok_or(VmError::ScopeUnderflow)?;
        if let Some(existing) = self
            .objects
            .get(&global_object_id)
            .and_then(|object| object.properties.get("Reflect"))
            .cloned()
        {
            return Ok(existing);
        }

        let reflect = self.create_object_value();
        let reflect_id = match reflect {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        let global_object = self
            .objects
            .get_mut(&global_object_id)
            .ok_or(VmError::UnknownObject(global_object_id))?;
        global_object
            .properties
            .insert("Reflect".to_string(), JsValue::Object(reflect_id));
        global_object.property_attributes.insert(
            "Reflect".to_string(),
            PropertyAttributes {
                writable: true,
                enumerable: false,
                configurable: true,
            },
        );
        Ok(JsValue::Object(reflect_id))
    }

    fn object_prototype_value(&self) -> JsValue {
        self.object_prototype_id
            .map(JsValue::Object)
            .unwrap_or(JsValue::Undefined)
    }

    fn function_prototype_value(&mut self) -> JsValue {
        if let Some(id) = self.function_prototype_host_id {
            return JsValue::HostFunction(id);
        }
        let id = self.next_host_function_id;
        self.next_host_function_id += 1;
        self.host_functions
            .insert(id, HostFunction::FunctionPrototype);
        self.function_prototype_host_id = Some(id);
        JsValue::HostFunction(id)
    }

    fn generator_function_prototype_value(&mut self) -> JsValue {
        if let Some(id) = self.generator_function_prototype_id {
            return JsValue::Object(id);
        }
        let prototype = self.create_object_value();
        let function_prototype = self.function_prototype_value();
        if let JsValue::Object(id) = prototype {
            if let Some(object) = self.objects.get_mut(&id) {
                object.prototype = None;
                object.prototype_value = Some(function_prototype);
                object.properties.insert(
                    "constructor".to_string(),
                    JsValue::NativeFunction(NativeFunction::GeneratorFunctionConstructor),
                );
                object.property_attributes.insert(
                    "constructor".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
            }
            self.generator_function_prototype_id = Some(id);
        }
        prototype
    }

    fn array_prototype_value(&mut self) -> JsValue {
        if let Some(id) = self.array_prototype_id {
            return JsValue::Object(id);
        }
        let prototype = self.create_object_value();
        if let JsValue::Object(id) = prototype {
            let join = self.create_host_function_value(HostFunction::ArrayJoin(id));
            let to_string = self.create_host_function_value(HostFunction::ArrayJoinThis);
            let concat = self.create_host_function_value(HostFunction::ArrayConcatThis);
            let reverse = self.create_host_function_value(HostFunction::ArrayReverse(id));
            let sort = self.create_host_function_value(HostFunction::ArraySort(id));
            let reduce = self.create_host_function_value(HostFunction::ArrayReduce(id));
            if let Some(object) = self.objects.get_mut(&id) {
                object.properties.insert(
                    "constructor".to_string(),
                    JsValue::NativeFunction(NativeFunction::ArrayConstructor),
                );
                object.property_attributes.insert(
                    "constructor".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
                object
                    .properties
                    .insert("length".to_string(), JsValue::Number(0.0));
                object.property_attributes.insert(
                    "length".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: false,
                    },
                );
                object.properties.insert("join".to_string(), join.clone());
                object.property_attributes.insert(
                    "join".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
                object.properties.insert("toString".to_string(), to_string);
                object.property_attributes.insert(
                    "toString".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
                object.properties.insert("concat".to_string(), concat);
                object.property_attributes.insert(
                    "concat".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
                object.properties.insert("reverse".to_string(), reverse);
                object.property_attributes.insert(
                    "reverse".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
                object.properties.insert("sort".to_string(), sort);
                object.property_attributes.insert(
                    "sort".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
                object.properties.insert("reduce".to_string(), reduce);
                object.property_attributes.insert(
                    "reduce".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
            }
            self.array_prototype_id = Some(id);
        }
        prototype
    }

    fn string_prototype_value(&mut self) -> JsValue {
        if let Some(id) = self.string_prototype_id {
            return JsValue::Object(id);
        }
        let prototype = self.create_object_value();
        if let JsValue::Object(id) = prototype {
            let to_string = self.create_host_function_value(HostFunction::StringToString);
            let value_of = self.create_host_function_value(HostFunction::StringValueOf);
            let char_at = self.create_host_function_value(HostFunction::StringCharAt);
            let char_code_at = self.create_host_function_value(HostFunction::StringCharCodeAt);
            let index_of = self.create_host_function_value(HostFunction::StringIndexOfThis);
            let last_index_of = self.create_host_function_value(HostFunction::StringLastIndexOf);
            let split = self.create_host_function_value(HostFunction::StringSplitThis);
            let substring = self.create_host_function_value(HostFunction::StringSubstring);
            let to_lower_case =
                self.create_host_function_value(HostFunction::StringToLowerCaseThis);
            let to_upper_case = self.create_host_function_value(HostFunction::StringToUpperCase);
            let trim = self.create_host_function_value(HostFunction::StringTrim);
            let replace = self.create_host_function_value(HostFunction::StringReplaceThis);
            let match_fn = self.create_host_function_value(HostFunction::StringMatchThis);
            let search = self.create_host_function_value(HostFunction::StringSearchThis);
            if let Some(object) = self.objects.get_mut(&id) {
                object.properties.insert(
                    "constructor".to_string(),
                    JsValue::NativeFunction(NativeFunction::StringConstructor),
                );
                object.property_attributes.insert(
                    "constructor".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
                object
                    .properties
                    .insert("length".to_string(), JsValue::Number(0.0));
                object.property_attributes.insert(
                    "length".to_string(),
                    PropertyAttributes {
                        writable: false,
                        enumerable: false,
                        configurable: false,
                    },
                );
                for (name, value) in [
                    ("toString", to_string),
                    ("valueOf", value_of),
                    ("charAt", char_at),
                    ("charCodeAt", char_code_at),
                    ("indexOf", index_of),
                    ("lastIndexOf", last_index_of),
                    ("split", split),
                    ("substring", substring),
                    ("toLowerCase", to_lower_case),
                    ("toUpperCase", to_upper_case),
                    ("trim", trim),
                    ("replace", replace),
                    ("match", match_fn),
                    ("search", search),
                ] {
                    object.properties.insert(name.to_string(), value);
                    object.property_attributes.insert(
                        name.to_string(),
                        PropertyAttributes {
                            writable: true,
                            enumerable: false,
                            configurable: true,
                        },
                    );
                }
            }
            self.string_prototype_id = Some(id);
        }
        prototype
    }

    fn number_prototype_value(&mut self) -> JsValue {
        if let Some(id) = self.number_prototype_id {
            return JsValue::Object(id);
        }
        let prototype = self.create_object_value();
        if let JsValue::Object(id) = prototype {
            let to_string = self.create_host_function_value(HostFunction::NumberToString);
            let value_of = self.create_host_function_value(HostFunction::NumberValueOf);
            let to_fixed = self.create_host_function_value(HostFunction::NumberToFixed);
            let to_exponential = self.create_host_function_value(HostFunction::NumberToExponential);
            if let Some(object) = self.objects.get_mut(&id) {
                object.properties.insert(
                    "constructor".to_string(),
                    JsValue::NativeFunction(NativeFunction::NumberConstructor),
                );
                object.property_attributes.insert(
                    "constructor".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
                for (name, value) in [
                    ("toString", to_string),
                    ("valueOf", value_of),
                    ("toFixed", to_fixed),
                    ("toExponential", to_exponential),
                ] {
                    object.properties.insert(name.to_string(), value);
                    object.property_attributes.insert(
                        name.to_string(),
                        PropertyAttributes {
                            writable: true,
                            enumerable: false,
                            configurable: true,
                        },
                    );
                }
            }
            self.number_prototype_id = Some(id);
        }
        prototype
    }

    fn boolean_prototype_value(&mut self) -> JsValue {
        if let Some(id) = self.boolean_prototype_id {
            return JsValue::Object(id);
        }
        let prototype = self.create_object_value();
        if let JsValue::Object(id) = prototype {
            let to_string = self.create_host_function_value(HostFunction::BooleanToString);
            let value_of = self.create_host_function_value(HostFunction::BooleanValueOf);
            if let Some(object) = self.objects.get_mut(&id) {
                object.properties.insert(
                    "constructor".to_string(),
                    JsValue::NativeFunction(NativeFunction::BooleanConstructor),
                );
                object.property_attributes.insert(
                    "constructor".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
                for (name, value) in [("toString", to_string), ("valueOf", value_of)] {
                    object.properties.insert(name.to_string(), value);
                    object.property_attributes.insert(
                        name.to_string(),
                        PropertyAttributes {
                            writable: true,
                            enumerable: false,
                            configurable: true,
                        },
                    );
                }
            }
            self.boolean_prototype_id = Some(id);
        }
        prototype
    }

    fn error_prototype_value(&mut self) -> JsValue {
        if let Some(id) = self.error_prototype_id {
            return JsValue::Object(id);
        }
        let prototype = self.create_object_value();
        if let JsValue::Object(id) = prototype {
            let to_string = self.create_host_function_value(HostFunction::ErrorToStringThis);
            if let Some(object) = self.objects.get_mut(&id) {
                object.properties.insert(
                    "constructor".to_string(),
                    JsValue::NativeFunction(NativeFunction::ErrorConstructor),
                );
                object
                    .properties
                    .insert("name".to_string(), JsValue::String("Error".to_string()));
                object
                    .properties
                    .insert("message".to_string(), JsValue::String(String::new()));
                object.properties.insert("toString".to_string(), to_string);
                object.property_attributes.insert(
                    "constructor".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
                object.property_attributes.insert(
                    "name".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
                object.property_attributes.insert(
                    "message".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
                object.property_attributes.insert(
                    "toString".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
            }
            self.error_prototype_id = Some(id);
        }
        prototype
    }

    fn type_error_prototype_value(&mut self) -> JsValue {
        if let Some(id) = self.type_error_prototype_id {
            return JsValue::Object(id);
        }
        let prototype = self.create_object_value();
        if let JsValue::Object(id) = prototype {
            let error_proto = match self.error_prototype_value() {
                JsValue::Object(error_id) => Some(error_id),
                _ => None,
            };
            if let Some(object) = self.objects.get_mut(&id) {
                object.prototype = error_proto;
                object.prototype_value = None;
                object.properties.insert(
                    "constructor".to_string(),
                    JsValue::NativeFunction(NativeFunction::TypeErrorConstructor),
                );
                object.property_attributes.insert(
                    "constructor".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
            }
            self.type_error_prototype_id = Some(id);
        }
        prototype
    }

    fn date_prototype_value(&mut self) -> JsValue {
        if let Some(id) = self.date_prototype_id {
            return JsValue::Object(id);
        }
        let prototype = self.create_object_value();
        if let JsValue::Object(id) = prototype {
            let get_full_year = self.create_host_function_value(HostFunction::DateGetFullYearThis);
            let get_month = self.create_host_function_value(HostFunction::DateGetMonthThis);
            let get_date = self.create_host_function_value(HostFunction::DateGetDateThis);
            let get_utc_full_year =
                self.create_host_function_value(HostFunction::DateGetUTCFullYearThis);
            let get_utc_month = self.create_host_function_value(HostFunction::DateGetUTCMonthThis);
            let get_utc_date = self.create_host_function_value(HostFunction::DateGetUTCDateThis);
            if let Some(object) = self.objects.get_mut(&id) {
                object.properties.insert(
                    "constructor".to_string(),
                    JsValue::NativeFunction(NativeFunction::DateConstructor),
                );
                object.property_attributes.insert(
                    "constructor".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
                for method in [
                    "toString",
                    "valueOf",
                    "getTime",
                    "getFullYear",
                    "getUTCFullYear",
                    "getMonth",
                    "getUTCMonth",
                    "getDate",
                    "getUTCDate",
                    "getDay",
                    "getUTCDay",
                    "getHours",
                    "getUTCHours",
                    "getMinutes",
                    "getUTCMinutes",
                    "getSeconds",
                    "getUTCSeconds",
                    "getMilliseconds",
                    "getUTCMilliseconds",
                    "setTime",
                    "setMilliseconds",
                    "setUTCMilliseconds",
                    "setSeconds",
                    "setUTCSeconds",
                    "setMinutes",
                    "setUTCMinutes",
                    "setHours",
                    "setUTCHours",
                    "setDate",
                    "setUTCDate",
                    "setMonth",
                    "setUTCMonth",
                    "setFullYear",
                    "setUTCFullYear",
                    "toLocaleString",
                    "toUTCString",
                ] {
                    let value = match method {
                        "getFullYear" => get_full_year.clone(),
                        "getMonth" => get_month.clone(),
                        "getDate" => get_date.clone(),
                        "getUTCFullYear" => get_utc_full_year.clone(),
                        "getUTCMonth" => get_utc_month.clone(),
                        "getUTCDate" => get_utc_date.clone(),
                        _ => JsValue::NativeFunction(NativeFunction::DatePrototypeMethod),
                    };
                    object.properties.insert(method.to_string(), value);
                    object.property_attributes.insert(
                        method.to_string(),
                        PropertyAttributes {
                            writable: true,
                            enumerable: false,
                            configurable: true,
                        },
                    );
                }
            }
            self.date_prototype_id = Some(id);
        }
        prototype
    }

    fn regexp_prototype_value(&mut self) -> JsValue {
        if let Some(id) = self.regexp_prototype_id {
            return JsValue::Object(id);
        }
        let prototype = self.create_object_value();
        if let JsValue::Object(id) = prototype {
            let test = self.create_host_function_value(HostFunction::RegExpTestThis);
            let exec = self.create_host_function_value(HostFunction::RegExpExecThis);
            let to_string = self.create_host_function_value(HostFunction::RegExpToStringThis);
            if let Some(object) = self.objects.get_mut(&id) {
                object.properties.insert(
                    "constructor".to_string(),
                    JsValue::NativeFunction(NativeFunction::RegExpConstructor),
                );
                object.property_attributes.insert(
                    "constructor".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
                object.properties.insert("test".to_string(), test);
                object.property_attributes.insert(
                    "test".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
                object.properties.insert("exec".to_string(), exec);
                object.property_attributes.insert(
                    "exec".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
                object.properties.insert("toString".to_string(), to_string);
                object.property_attributes.insert(
                    "toString".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
            }
            self.regexp_prototype_id = Some(id);
        }
        prototype
    }

    fn array_buffer_prototype_value(&mut self) -> JsValue {
        if let Some(id) = self.array_buffer_prototype_id {
            return JsValue::Object(id);
        }
        let prototype = self.create_object_value();
        if let JsValue::Object(id) = prototype {
            let slice = self.create_host_function_value(HostFunction::ArrayBufferSliceThis);
            if let Some(object) = self.objects.get_mut(&id) {
                object.properties.insert(
                    "constructor".to_string(),
                    JsValue::NativeFunction(NativeFunction::ArrayBufferConstructor),
                );
                object.property_attributes.insert(
                    "constructor".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
                object.properties.insert("slice".to_string(), slice);
                object.property_attributes.insert(
                    "slice".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
            }
            self.array_buffer_prototype_id = Some(id);
        }
        prototype
    }

    fn data_view_prototype_value(&mut self) -> JsValue {
        if let Some(id) = self.data_view_prototype_id {
            return JsValue::Object(id);
        }
        let prototype = self.create_object_value();
        if let JsValue::Object(id) = prototype {
            if let Some(object) = self.objects.get_mut(&id) {
                object.properties.insert(
                    "constructor".to_string(),
                    JsValue::NativeFunction(NativeFunction::DataViewConstructor),
                );
                object.property_attributes.insert(
                    "constructor".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
            }
            self.data_view_prototype_id = Some(id);
        }
        prototype
    }

    fn map_prototype_value(&mut self) -> JsValue {
        if let Some(id) = self.map_prototype_id {
            return JsValue::Object(id);
        }
        let prototype = self.create_object_value();
        if let JsValue::Object(id) = prototype {
            let set = self.create_host_function_value(HostFunction::MapSetThis);
            if let Some(object) = self.objects.get_mut(&id) {
                object.properties.insert(
                    "constructor".to_string(),
                    JsValue::NativeFunction(NativeFunction::MapConstructor),
                );
                object.property_attributes.insert(
                    "constructor".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
                object.properties.insert("set".to_string(), set);
                object.property_attributes.insert(
                    "set".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
            }
            self.map_prototype_id = Some(id);
        }
        prototype
    }

    fn set_prototype_value(&mut self) -> JsValue {
        if let Some(id) = self.set_prototype_id {
            return JsValue::Object(id);
        }
        let prototype = self.create_object_value();
        if let JsValue::Object(id) = prototype {
            let add = self.create_host_function_value(HostFunction::SetAddThis);
            if let Some(object) = self.objects.get_mut(&id) {
                object.properties.insert(
                    "constructor".to_string(),
                    JsValue::NativeFunction(NativeFunction::SetConstructor),
                );
                object.property_attributes.insert(
                    "constructor".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
                object.properties.insert("add".to_string(), add);
                object.property_attributes.insert(
                    "add".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
            }
            self.set_prototype_id = Some(id);
        }
        prototype
    }

    fn promise_prototype_value(&mut self) -> JsValue {
        if let Some(id) = self.promise_prototype_id {
            return JsValue::Object(id);
        }
        let prototype = self.create_object_value();
        if let JsValue::Object(id) = prototype {
            if let Some(object) = self.objects.get_mut(&id) {
                object.properties.insert(
                    "constructor".to_string(),
                    JsValue::NativeFunction(NativeFunction::PromiseConstructor),
                );
                object.property_attributes.insert(
                    "constructor".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
            }
            self.promise_prototype_id = Some(id);
        }
        prototype
    }

    fn uint8_array_prototype_value(&mut self) -> JsValue {
        if let Some(id) = self.uint8_array_prototype_id {
            return JsValue::Object(id);
        }
        let prototype = self.create_object_value();
        if let JsValue::Object(id) = prototype {
            if let Some(object) = self.objects.get_mut(&id) {
                object.properties.insert(
                    "constructor".to_string(),
                    JsValue::NativeFunction(NativeFunction::Uint8ArrayConstructor),
                );
                object.property_attributes.insert(
                    "constructor".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
            }
            self.uint8_array_prototype_id = Some(id);
        }
        prototype
    }

    fn boxed_primitive_value(&self, object_id: ObjectId) -> Option<JsValue> {
        self.objects
            .get(&object_id)
            .and_then(|object| object.properties.get(BOXED_PRIMITIVE_VALUE_KEY))
            .cloned()
    }

    fn is_date_object(&self, object_id: ObjectId) -> bool {
        self.objects
            .get(&object_id)
            .and_then(|object| object.properties.get(DATE_OBJECT_MARKER_KEY))
            .is_some_and(|value| matches!(value, JsValue::Bool(true)))
    }

    fn primitive_for_add(
        &mut self,
        value: JsValue,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        match value {
            JsValue::Object(object_id) => {
                if let Some(boxed) = self.boxed_primitive_value(object_id) {
                    return Ok(boxed);
                }
                if self.objects.get(&object_id).is_some_and(|object| {
                    object.properties.contains_key("length")
                        && object.prototype == self.array_prototype_id
                }) {
                    return self.execute_array_join(object_id, ",");
                }
                self.ordinary_to_primitive_for_add(
                    object_id,
                    self.is_date_object(object_id),
                    realm,
                    caller_strict,
                )
            }
            JsValue::Function(closure_id) => {
                self.ordinary_to_primitive_for_function(closure_id, false, realm, caller_strict)
            }
            other => Ok(other),
        }
    }

    fn ordinary_to_primitive_for_add(
        &mut self,
        object_id: ObjectId,
        prefer_string: bool,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        let method_names = if prefer_string {
            ["toString", "valueOf"]
        } else {
            ["valueOf", "toString"]
        };
        let mut invoked_callable = false;

        for method_name in method_names {
            let method = self.get_object_property(object_id, method_name, realm)?;
            if !Self::is_callable_value(&method) {
                continue;
            }
            invoked_callable = true;
            let result = self.execute_callable(
                method,
                Some(JsValue::Object(object_id)),
                Vec::new(),
                realm,
                caller_strict,
            )?;
            if !matches!(result, JsValue::Object(_)) {
                return Ok(result);
            }
        }

        if invoked_callable {
            return Err(VmError::TypeError(
                "cannot convert object to primitive value",
            ));
        }

        Ok(JsValue::String("[object Object]".to_string()))
    }

    fn ordinary_to_primitive_for_property_key(
        &mut self,
        object_id: ObjectId,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        for method_name in ["toString", "valueOf"] {
            let method = self.get_object_property(object_id, method_name, realm)?;
            if !Self::is_callable_value(&method) {
                continue;
            }
            let result = self.execute_callable(
                method,
                Some(JsValue::Object(object_id)),
                Vec::new(),
                realm,
                caller_strict,
            )?;
            if !matches!(
                result,
                JsValue::Object(_)
                    | JsValue::Function(_)
                    | JsValue::NativeFunction(_)
                    | JsValue::HostFunction(_)
            ) {
                return Ok(result);
            }
        }
        Err(VmError::TypeError("cannot convert object to property key"))
    }

    fn ordinary_to_primitive_for_function(
        &mut self,
        closure_id: u64,
        prefer_string: bool,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        let method_names = if prefer_string {
            ["toString", "valueOf"]
        } else {
            ["valueOf", "toString"]
        };
        let mut invoked_callable = false;

        for method_name in method_names {
            let method = self.get_function_property(closure_id, method_name, realm)?;
            if !Self::is_callable_value(&method) {
                continue;
            }
            invoked_callable = true;
            let result = self.execute_callable(
                method,
                Some(JsValue::Function(closure_id)),
                Vec::new(),
                realm,
                caller_strict,
            )?;
            if !matches!(
                result,
                JsValue::Object(_)
                    | JsValue::Function(_)
                    | JsValue::NativeFunction(_)
                    | JsValue::HostFunction(_)
            ) {
                return Ok(result);
            }
        }

        if invoked_callable {
            return Err(VmError::TypeError(
                "cannot convert object to primitive value",
            ));
        }

        Ok(JsValue::String("[function]".to_string()))
    }

    fn coerce_this_for_sloppy(&mut self, this_arg: Option<JsValue>) -> JsValue {
        match this_arg {
            None | Some(JsValue::Null) | Some(JsValue::Undefined) => self.global_this_value(),
            Some(primitive @ (JsValue::Number(_) | JsValue::Bool(_) | JsValue::String(_))) => {
                self.box_primitive_receiver(primitive)
            }
            Some(value) => value,
        }
    }

    fn box_primitive_receiver(&mut self, primitive: JsValue) -> JsValue {
        let receiver = self.create_object_value();
        let object_id = match receiver {
            JsValue::Object(id) => id,
            _ => unreachable!(),
        };
        let (constructor, prototype_id) = match primitive {
            JsValue::Number(_) => (
                Some(NativeFunction::NumberConstructor),
                match self.number_prototype_value() {
                    JsValue::Object(id) => Some(id),
                    _ => None,
                },
            ),
            JsValue::Bool(_) => (
                Some(NativeFunction::BooleanConstructor),
                match self.boolean_prototype_value() {
                    JsValue::Object(id) => Some(id),
                    _ => None,
                },
            ),
            JsValue::String(_) => (
                Some(NativeFunction::StringConstructor),
                match self.string_prototype_value() {
                    JsValue::Object(id) => Some(id),
                    _ => None,
                },
            ),
            _ => (None, None),
        };
        if let Some(object) = self.objects.get_mut(&object_id) {
            object.prototype = prototype_id;
            object.prototype_value = None;
            object
                .properties
                .insert(BOXED_PRIMITIVE_VALUE_KEY.to_string(), primitive);
            object.property_attributes.insert(
                BOXED_PRIMITIVE_VALUE_KEY.to_string(),
                PropertyAttributes {
                    writable: false,
                    enumerable: false,
                    configurable: false,
                },
            );
            if let Some(constructor) = constructor {
                object.properties.insert(
                    "constructor".to_string(),
                    JsValue::NativeFunction(constructor),
                );
                object.property_attributes.insert(
                    "constructor".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
            }
        }
        JsValue::Object(object_id)
    }

    fn function_is_strict(&self, function: &CompiledFunction) -> bool {
        self.code_is_strict(&function.code)
    }

    fn function_is_arrow(&self, function: &CompiledFunction) -> bool {
        self.code_has_marker(&function.code, ARROW_FUNCTION_MARKER)
    }

    fn function_is_generator(&self, function: &CompiledFunction) -> bool {
        self.code_has_marker(&function.code, GENERATOR_FUNCTION_MARKER)
    }

    fn function_uses_yield_identifier(&self, function: &CompiledFunction) -> bool {
        function.code.iter().any(|opcode| {
            matches!(
                opcode,
                Opcode::LoadIdentifier(name)
                    | Opcode::ResolveIdentifierReference(name)
                    | Opcode::TypeofIdentifier(name)
                    if name == "yield"
            )
        })
    }

    fn function_is_async(&self, function: &CompiledFunction) -> bool {
        self.code_has_marker(&function.code, ASYNC_FUNCTION_MARKER)
    }

    fn function_is_named_function_expression(&self, function: &CompiledFunction) -> bool {
        self.code_has_marker(&function.code, NAMED_FUNCTION_EXPR_MARKER)
    }

    fn function_has_non_simple_params(&self, function: &CompiledFunction) -> bool {
        self.code_has_marker(&function.code, NON_SIMPLE_PARAMS_MARKER)
    }

    fn function_rest_param_index(&self, function: &CompiledFunction) -> Option<usize> {
        let mut cursor = 0usize;
        while cursor + 1 < function.code.len() {
            match (&function.code[cursor], &function.code[cursor + 1]) {
                (Opcode::LoadString(value), Opcode::Pop) => {
                    if let Some(raw_index) = value.strip_prefix(REST_PARAM_MARKER_PREFIX) {
                        if let Ok(index) = raw_index.parse::<usize>() {
                            return Some(index);
                        }
                    }
                    cursor += 2;
                }
                _ => cursor += 1,
            }
        }
        None
    }

    fn closure_is_arrow(&self, closure_id: u64) -> Result<bool, VmError> {
        let closure = self
            .closures
            .get(&closure_id)
            .ok_or(VmError::UnknownClosure(closure_id))?;
        let function = closure
            .functions
            .get(closure.function_id)
            .ok_or(VmError::UnknownFunction(closure.function_id))?;
        Ok(self.function_is_arrow(function))
    }

    fn closure_is_generator(&self, closure_id: u64) -> Result<bool, VmError> {
        let closure = self
            .closures
            .get(&closure_id)
            .ok_or(VmError::UnknownClosure(closure_id))?;
        let function = closure
            .functions
            .get(closure.function_id)
            .ok_or(VmError::UnknownFunction(closure.function_id))?;
        Ok(self.function_is_generator(function))
    }

    fn closure_uses_yield_identifier(&self, closure_id: u64) -> Result<bool, VmError> {
        let closure = self
            .closures
            .get(&closure_id)
            .ok_or(VmError::UnknownClosure(closure_id))?;
        let function = closure
            .functions
            .get(closure.function_id)
            .ok_or(VmError::UnknownFunction(closure.function_id))?;
        Ok(self.function_uses_yield_identifier(function))
    }

    fn closure_is_class_constructor(&self, closure_id: u64) -> bool {
        self.closure_objects
            .get(&closure_id)
            .and_then(|object| object.properties.get(CLASS_CONSTRUCTOR_MARKER))
            .is_some_and(|marker| matches!(marker, JsValue::Bool(true)))
    }

    fn closure_is_derived_class_constructor(&self, closure_id: u64) -> bool {
        self.closure_objects
            .get(&closure_id)
            .and_then(|object| object.properties.get(CLASS_DERIVED_CONSTRUCTOR_MARKER))
            .is_some_and(|marker| matches!(marker, JsValue::Bool(true)))
    }

    fn closure_marks_restricted_caller_arguments(&self, closure_id: u64) -> bool {
        self.closure_objects
            .get(&closure_id)
            .and_then(|object| object.properties.get(CLASS_HERITAGE_RESTRICTED_MARKER))
            .is_some_and(|marker| matches!(marker, JsValue::Bool(true)))
    }

    fn closure_has_no_prototype(&self, closure_id: u64) -> bool {
        if self
            .closure_objects
            .get(&closure_id)
            .and_then(|object| object.properties.get(CLASS_METHOD_NO_PROTOTYPE_MARKER))
            .is_some_and(|marker| matches!(marker, JsValue::Bool(true)))
        {
            return true;
        }
        let Some(closure) = self.closures.get(&closure_id) else {
            return false;
        };
        let Some(function) = closure.functions.get(closure.function_id) else {
            return false;
        };
        self.code_has_marker(&function.code, CLASS_METHOD_NO_PROTOTYPE_MARKER)
    }

    fn code_is_strict(&self, code: &[Opcode]) -> bool {
        let cursor = Self::skip_prologue_prefix(code);
        if matches!(code.get(cursor), Some(Opcode::MarkStrict)) {
            return true;
        }
        if cursor != 0 && matches!(code.first(), Some(Opcode::MarkStrict)) {
            return true;
        }
        false
    }

    fn code_has_marker(&self, code: &[Opcode], marker: &str) -> bool {
        let mut cursor = 0usize;
        while cursor + 1 < code.len() {
            match (&code[cursor], &code[cursor + 1]) {
                (Opcode::LoadString(value), Opcode::Pop) => {
                    if value == marker {
                        return true;
                    }
                    cursor += 2;
                }
                _ => cursor += 1,
            }
        }
        false
    }

    fn skip_prologue_prefix(code: &[Opcode]) -> usize {
        let mut cursor = 0usize;
        if matches!(code.get(cursor), Some(Opcode::MarkStrict)) {
            cursor += 1;
        }
        while cursor < code.len() {
            match &code[cursor] {
                Opcode::DefineFunction { .. } | Opcode::DefineVar(_) => cursor += 1,
                _ => break,
            }
        }
        while cursor + 1 < code.len() {
            match (&code[cursor], &code[cursor + 1]) {
                (Opcode::LoadUndefined, Opcode::DefineVariable { .. })
                | (Opcode::LoadUninitialized, Opcode::DefineVariable { .. }) => cursor += 2,
                _ => break,
            }
        }
        cursor
    }

    fn current_eval_context_rejects_arguments_declaration(&self) -> bool {
        let Some(context) = self.eval_contexts.last() else {
            return false;
        };
        (context.non_simple_params && !context.is_arrow_function) || context.has_arguments_param
    }

    fn script_declares_arguments_binding(script: &Script) -> bool {
        script
            .statements
            .iter()
            .any(Self::statement_declares_arguments_binding)
    }

    fn statement_declares_arguments_binding(statement: &Stmt) -> bool {
        match statement {
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Var,
                name: Identifier(name),
                ..
            }) => name == "arguments",
            Stmt::VariableDeclaration(VariableDeclaration {
                kind: BindingKind::Let | BindingKind::Const,
                ..
            }) => false,
            Stmt::VariableDeclarations(declarations) => declarations.iter().any(|declaration| {
                declaration.kind == BindingKind::Var && declaration.name.0 == "arguments"
            }),
            Stmt::FunctionDeclaration(declaration) => declaration.name.0 == "arguments",
            Stmt::Block(statements) => statements
                .iter()
                .any(Self::statement_declares_arguments_binding),
            Stmt::If {
                consequent,
                alternate,
                ..
            } => {
                if Self::statement_declares_arguments_binding(consequent) {
                    return true;
                }
                alternate
                    .as_ref()
                    .is_some_and(|alternate| Self::statement_declares_arguments_binding(alternate))
            }
            Stmt::While { body, .. }
            | Stmt::With { body, .. }
            | Stmt::DoWhile { body, .. }
            | Stmt::Labeled { body, .. } => Self::statement_declares_arguments_binding(body),
            Stmt::For {
                initializer, body, ..
            } => {
                let initializer_declares_arguments =
                    initializer
                        .as_ref()
                        .is_some_and(|initializer| match initializer {
                            ForInitializer::VariableDeclaration(declaration) => {
                                declaration.kind == BindingKind::Var
                                    && declaration.name.0 == "arguments"
                            }
                            ForInitializer::VariableDeclarations(declarations) => {
                                declarations.iter().any(|declaration| {
                                    declaration.kind == BindingKind::Var
                                        && declaration.name.0 == "arguments"
                                })
                            }
                            ForInitializer::Expression(_) => false,
                        });
                initializer_declares_arguments || Self::statement_declares_arguments_binding(body)
            }
            Stmt::Switch { cases, .. } => cases.iter().any(|case| {
                case.consequent
                    .iter()
                    .any(Self::statement_declares_arguments_binding)
            }),
            Stmt::Try {
                try_block,
                catch_block,
                finally_block,
                ..
            } => {
                if try_block
                    .iter()
                    .any(Self::statement_declares_arguments_binding)
                {
                    return true;
                }
                if catch_block.as_ref().is_some_and(|catch_block| {
                    catch_block
                        .iter()
                        .any(Self::statement_declares_arguments_binding)
                }) {
                    return true;
                }
                finally_block.as_ref().is_some_and(|finally_block| {
                    finally_block
                        .iter()
                        .any(Self::statement_declares_arguments_binding)
                })
            }
            Stmt::Empty
            | Stmt::Return(_)
            | Stmt::Expression(_)
            | Stmt::Throw(_)
            | Stmt::Break
            | Stmt::BreakLabel(_)
            | Stmt::Continue
            | Stmt::ContinueLabel(_) => false,
        }
    }

    fn script_declares_restricted_global_function(script: &Script) -> bool {
        script.statements.iter().any(|statement| {
            let Stmt::FunctionDeclaration(declaration) = statement else {
                return false;
            };
            matches!(
                declaration.name.0.as_str(),
                "NaN" | "Infinity" | "undefined"
            )
        })
    }

    fn create_host_function_value(&mut self, host: HostFunction) -> JsValue {
        let id = self.next_host_function_id;
        self.next_host_function_id += 1;
        self.host_functions.insert(id, host);
        JsValue::HostFunction(id)
    }

    fn shared_object_to_string_function(&mut self) -> JsValue {
        if let Some(host_id) = self.object_to_string_host_id {
            return JsValue::HostFunction(host_id);
        }
        let id = self.next_host_function_id;
        self.next_host_function_id += 1;
        self.host_functions.insert(id, HostFunction::ObjectToString);
        self.object_to_string_host_id = Some(id);
        JsValue::HostFunction(id)
    }

    fn get_object_property(
        &mut self,
        object_id: ObjectId,
        property: &str,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        if property == "hasOwnProperty" {
            return Ok(
                self.create_host_function_value(HostFunction::HasOwnProperty {
                    target: JsValue::Object(object_id),
                }),
            );
        }
        if property == "isPrototypeOf" {
            return Ok(
                self.create_host_function_value(HostFunction::IsPrototypeOf {
                    target: JsValue::Object(object_id),
                }),
            );
        }
        if let Some(JsValue::String(value)) = self.boxed_primitive_value(object_id) {
            if property == "length" {
                return Ok(JsValue::Number(Self::utf16_code_unit_length(&value) as f64));
            }
            if let Ok(index) = property.parse::<usize>() {
                return Ok(value
                    .chars()
                    .nth(index)
                    .map(|ch| JsValue::String(ch.to_string()))
                    .unwrap_or(JsValue::Undefined));
            }
        }
        if property == "push"
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.properties.contains_key("length"))
        {
            return Ok(self.create_host_function_value(HostFunction::ArrayPush(object_id)));
        }
        if property == "pop"
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.properties.contains_key("length"))
        {
            return Ok(self.create_host_function_value(HostFunction::ArrayPopThis));
        }
        if property == "concat"
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.properties.contains_key("length"))
        {
            return Ok(self.create_host_function_value(HostFunction::ArrayConcatThis));
        }
        if property == "forEach"
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.properties.contains_key("length"))
        {
            return Ok(self.create_host_function_value(HostFunction::ArrayForEach(object_id)));
        }
        if property == "reduce"
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.properties.contains_key("length"))
        {
            return Ok(self.create_host_function_value(HostFunction::ArrayReduce(object_id)));
        }
        if property == "join"
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.properties.contains_key("length"))
        {
            return Ok(self.create_host_function_value(HostFunction::ArrayJoin(object_id)));
        }
        if property == "reverse"
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.properties.contains_key("length"))
        {
            return Ok(self.create_host_function_value(HostFunction::ArrayReverse(object_id)));
        }
        if property == "sort"
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.properties.contains_key("length"))
        {
            return Ok(self.create_host_function_value(HostFunction::ArraySort(object_id)));
        }
        if property == "keys"
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.properties.contains_key("length"))
        {
            return Ok(self.create_host_function_value(HostFunction::ArrayKeysThis));
        }
        if property == "entries"
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.properties.contains_key("length"))
        {
            return Ok(self.create_host_function_value(HostFunction::ArrayEntriesThis));
        }
        if (property == "values" || property == "Symbol.iterator")
            && self
                .objects
                .get(&object_id)
                .is_some_and(|object| object.properties.contains_key("length"))
        {
            return Ok(self.create_host_function_value(HostFunction::ArrayValuesThis));
        }
        let mapped_binding = self
            .objects
            .get(&object_id)
            .and_then(|object| object.argument_mappings.get(property).copied());
        if let Some(binding_id) = mapped_binding {
            if let Some(binding) = self.bindings.get(&binding_id) {
                return Ok(binding.value.clone());
            }
        }
        let receiver = JsValue::Object(object_id);
        let mut current_id = Some(object_id);
        while let Some(id) = current_id {
            let (getter, value, next, prototype_value) = {
                let object = self.objects.get(&id).ok_or(VmError::UnknownObject(id))?;
                (
                    object.getters.get(property).cloned(),
                    object.properties.get(property).cloned(),
                    object.prototype,
                    object.prototype_value.clone(),
                )
            };
            if let Some(getter) = getter {
                return self.execute_callable(
                    getter,
                    Some(receiver.clone()),
                    Vec::new(),
                    realm,
                    false,
                );
            }
            if let Some(value) = value {
                return Ok(value);
            }
            if let Some(next) = next {
                current_id = Some(next);
                continue;
            }
            if let Some(prototype_value) = prototype_value {
                return self.get_property_from_base_with_receiver(
                    prototype_value,
                    property,
                    receiver.clone(),
                    realm,
                );
            }
            current_id = None;
        }
        Ok(JsValue::Undefined)
    }

    fn set_object_property(
        &mut self,
        object_id: ObjectId,
        property: String,
        value: JsValue,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let (mapped_binding_id, own_setter, own_getter_exists, own_data_writable, own_prototype) = {
            let object = self
                .objects
                .get(&object_id)
                .ok_or(VmError::UnknownObject(object_id))?;
            (
                object.argument_mappings.get(&property).copied(),
                object.setters.get(&property).cloned(),
                object.getters.contains_key(&property),
                object.properties.get(&property).map(|_| {
                    object
                        .property_attributes
                        .get(&property)
                        .is_none_or(|attributes| attributes.writable)
                }),
                object.prototype,
            )
        };

        if let Some(setter) = own_setter {
            let _ = self.execute_callable(
                setter,
                Some(JsValue::Object(object_id)),
                vec![value.clone()],
                realm,
                false,
            )?;
            return Ok(value);
        }
        if own_getter_exists {
            return Ok(value);
        }
        if own_data_writable == Some(false) {
            return Ok(value);
        }

        if own_data_writable.is_none() {
            let mut current = own_prototype;
            while let Some(proto_id) = current {
                let (setter, getter_exists, data_writable, next) = {
                    let object = self
                        .objects
                        .get(&proto_id)
                        .ok_or(VmError::UnknownObject(proto_id))?;
                    (
                        object.setters.get(&property).cloned(),
                        object.getters.contains_key(&property),
                        object.properties.get(&property).map(|_| {
                            object
                                .property_attributes
                                .get(&property)
                                .is_none_or(|attributes| attributes.writable)
                        }),
                        object.prototype,
                    )
                };

                if let Some(setter) = setter {
                    let _ = self.execute_callable(
                        setter,
                        Some(JsValue::Object(object_id)),
                        vec![value.clone()],
                        realm,
                        false,
                    )?;
                    return Ok(value);
                }
                if getter_exists {
                    return Ok(value);
                }
                if data_writable == Some(false) {
                    return Ok(value);
                }
                if data_writable == Some(true) {
                    break;
                }
                current = next;
            }
        }

        if let Some(binding_id) = mapped_binding_id {
            if let Some(binding) = self.bindings.get_mut(&binding_id) {
                binding.value = value.clone();
            }
        }
        let can_write_property = {
            let object = self
                .objects
                .get(&object_id)
                .ok_or(VmError::UnknownObject(object_id))?;
            object.extensible
                || object.properties.contains_key(&property)
                || object.getters.contains_key(&property)
                || object.setters.contains_key(&property)
        };
        if !can_write_property {
            return Ok(value);
        }
        if self.has_object_marker(object_id, "__uint8ArrayTag")? {
            if let Some(index) = Self::canonical_array_index(&property) {
                self.set_uint8_array_index_property(object_id, index, &value)?;
                return Ok(value);
            }
            if matches!(property.as_str(), "length" | "byteLength") {
                return Ok(value);
            }
        }
        if self.is_array_length_tracking_object(object_id)? {
            if property == "length" {
                self.set_array_length_property(object_id, &value)?;
                return Ok(value);
            }
            if let Some(index) = Self::canonical_array_index(&property) {
                self.set_array_index_property(object_id, index, property, value.clone())?;
                return Ok(value);
            }
        }
        let object = self
            .objects
            .get_mut(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        object.properties.insert(property.clone(), value.clone());
        object
            .property_attributes
            .entry(property.clone())
            .or_insert_with(PropertyAttributes::default);
        self.sync_global_binding_from_property_write(object_id, &property, &value)?;
        Ok(value)
    }

    fn set_closure_property(
        &mut self,
        closure_id: u64,
        property: String,
        value: JsValue,
    ) -> JsValue {
        let object = self.closure_objects.entry(closure_id).or_default();
        object.properties.insert(property.clone(), value.clone());
        object.property_attributes.entry(property).or_default();
        value
    }

    fn maybe_set_inferred_function_name(&mut self, value: &JsValue, binding_name: &str) {
        let JsValue::Function(closure_id) = value else {
            return;
        };
        if binding_name.starts_with("$__class_ctor_") {
            return;
        }
        let existing_name = self
            .closure_objects
            .get(closure_id)
            .and_then(|object| object.properties.get("name"));
        let should_keep_existing_name = match existing_name {
            Some(JsValue::String(name)) => {
                !name.is_empty() && name != "<anonymous>" && !name.starts_with("$__class_ctor_")
            }
            Some(_) => true,
            None => false,
        };
        if should_keep_existing_name {
            return;
        }
        let inferred = self
            .closures
            .get(closure_id)
            .and_then(|closure| closure.functions.get(closure.function_id))
            .map(|function| function.name.as_str())
            .filter(|name| !name.is_empty() && *name != "<anonymous>")
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| binding_name.to_string());
        let object = self.closure_objects.entry(*closure_id).or_default();
        object
            .properties
            .insert("name".to_string(), JsValue::String(inferred));
        object.property_attributes.insert(
            "name".to_string(),
            PropertyAttributes {
                writable: false,
                enumerable: false,
                configurable: true,
            },
        );
    }

    fn set_function_property(
        &mut self,
        closure_id: u64,
        property: String,
        value: JsValue,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        let (setter, getter_exists, data_writable) = self
            .closure_objects
            .get(&closure_id)
            .map(|object| {
                (
                    object.setters.get(&property).cloned(),
                    object.getters.contains_key(&property),
                    object.properties.get(&property).map(|_| {
                        object
                            .property_attributes
                            .get(&property)
                            .is_none_or(|attributes| attributes.writable)
                    }),
                )
            })
            .unwrap_or((None, false, None));

        if let Some(setter) = setter {
            let _ = self.execute_callable(
                setter,
                Some(JsValue::Function(closure_id)),
                vec![value.clone()],
                realm,
                false,
            )?;
            return Ok(value);
        }
        if getter_exists {
            return Ok(value);
        }
        if data_writable == Some(false) {
            return Ok(value);
        }

        Ok(self.set_closure_property(closure_id, property, value))
    }

    fn get_or_create_function_prototype_property(
        &mut self,
        closure_id: u64,
    ) -> Result<JsValue, VmError> {
        if !self.closures.contains_key(&closure_id) {
            return Err(VmError::UnknownClosure(closure_id));
        }
        if let Some(existing) = self
            .closure_objects
            .get(&closure_id)
            .and_then(|object| object.properties.get("prototype"))
            .cloned()
        {
            return Ok(existing);
        }

        let prototype = self.create_object_value();
        if let JsValue::Object(prototype_id) = prototype {
            if let Some(prototype_object) = self.objects.get_mut(&prototype_id) {
                prototype_object
                    .properties
                    .insert("constructor".to_string(), JsValue::Function(closure_id));
                prototype_object.property_attributes.insert(
                    "constructor".to_string(),
                    PropertyAttributes {
                        writable: true,
                        enumerable: false,
                        configurable: true,
                    },
                );
            }
        }

        let object = self.closure_objects.entry(closure_id).or_default();
        object
            .properties
            .insert("prototype".to_string(), prototype.clone());
        object.property_attributes.insert(
            "prototype".to_string(),
            PropertyAttributes {
                writable: true,
                enumerable: false,
                configurable: false,
            },
        );

        Ok(prototype)
    }

    fn closure_has_own_property(&self, closure_id: u64, property: &str) -> bool {
        self.closure_objects.get(&closure_id).is_some_and(|object| {
            object.properties.contains_key(property)
                || object.getters.contains_key(property)
                || object.setters.contains_key(property)
        })
    }

    fn function_rejects_caller_arguments(&self, closure_id: u64) -> Result<bool, VmError> {
        let closure = self
            .closures
            .get(&closure_id)
            .ok_or(VmError::UnknownClosure(closure_id))?;
        let function = closure
            .functions
            .get(closure.function_id)
            .ok_or(VmError::UnknownFunction(closure.function_id))?;
        Ok(closure.strict
            || self.function_is_arrow(function)
            || self.closure_is_class_constructor(closure_id)
            || self.closure_marks_restricted_caller_arguments(closure_id))
    }

    fn is_restricted_function_property(property: &str) -> bool {
        matches!(property, "caller" | "arguments")
    }

    fn has_own_property(&self, target: &JsValue, property: &str) -> Result<bool, VmError> {
        match target {
            JsValue::Object(object_id) => {
                let object = self
                    .objects
                    .get(object_id)
                    .ok_or(VmError::UnknownObject(*object_id))?;
                Ok(object.properties.contains_key(property)
                    || object.getters.contains_key(property)
                    || object.setters.contains_key(property))
            }
            JsValue::Function(closure_id) => {
                if self.closure_has_own_property(*closure_id, property) {
                    return Ok(true);
                }
                if property == "length" {
                    return Ok(true);
                }
                if property == "prototype" {
                    return Ok(!self.closure_is_arrow(*closure_id)?
                        && !self.closure_has_no_prototype(*closure_id));
                }
                Ok(false)
            }
            JsValue::NativeFunction(native) => {
                Ok(self.native_function_has_own_property(*native, property))
            }
            JsValue::HostFunction(host_id) => {
                if !self.host_functions.contains_key(host_id) {
                    return Err(VmError::UnknownHostFunction(*host_id));
                }
                if self
                    .host_function_objects
                    .get(host_id)
                    .is_some_and(|object| {
                        object.properties.contains_key(property)
                            || object.getters.contains_key(property)
                            || object.setters.contains_key(property)
                    })
                {
                    return Ok(true);
                }
                Ok(Self::host_function_has_default_property(property))
            }
            _ => Ok(false),
        }
    }

    fn host_function_has_default_property(property: &str) -> bool {
        matches!(
            property,
            "length"
                | "call"
                | "apply"
                | "bind"
                | "toString"
                | "valueOf"
                | "hasOwnProperty"
                | "constructor"
        )
    }

    fn native_function_has_own_property(&self, native: NativeFunction, property: &str) -> bool {
        matches!(
            (native, property),
            (NativeFunction::ObjectConstructor, "defineProperty")
                | (NativeFunction::ObjectConstructor, "defineProperties")
                | (NativeFunction::ObjectConstructor, "keys")
                | (NativeFunction::ObjectConstructor, "getOwnPropertyNames")
                | (NativeFunction::ObjectConstructor, "create")
                | (NativeFunction::ObjectConstructor, "setPrototypeOf")
                | (NativeFunction::ObjectConstructor, "preventExtensions")
                | (NativeFunction::ObjectConstructor, "__forInKeys")
                | (NativeFunction::ObjectConstructor, "__forOfValues")
                | (NativeFunction::ObjectConstructor, "__forOfIterator")
                | (NativeFunction::ObjectConstructor, "__forOfStep")
                | (NativeFunction::ObjectConstructor, "__forOfClose")
                | (NativeFunction::ObjectConstructor, "__getTemplateObject")
                | (
                    NativeFunction::ObjectConstructor,
                    "getOwnPropertyDescriptor"
                )
                | (NativeFunction::ArrayConstructor, "isArray")
                | (NativeFunction::ObjectConstructor, "getPrototypeOf")
                | (NativeFunction::ObjectConstructor, "isExtensible")
                | (NativeFunction::ObjectConstructor, "freeze")
                | (NativeFunction::ObjectConstructor, "toString")
                | (NativeFunction::ObjectConstructor, "valueOf")
                | (NativeFunction::DateConstructor, "parse")
                | (NativeFunction::DateConstructor, "UTC")
                | (NativeFunction::StringConstructor, "fromCharCode")
                | (NativeFunction::NumberConstructor, "NaN")
                | (NativeFunction::NumberConstructor, "POSITIVE_INFINITY")
                | (NativeFunction::NumberConstructor, "NEGATIVE_INFINITY")
                | (NativeFunction::NumberConstructor, "MAX_VALUE")
                | (NativeFunction::NumberConstructor, "MIN_VALUE")
                | (NativeFunction::SymbolConstructor, "iterator")
                | (NativeFunction::SymbolConstructor, "asyncIterator")
                | (NativeFunction::SymbolConstructor, "hasInstance")
                | (NativeFunction::SymbolConstructor, "isConcatSpreadable")
                | (NativeFunction::SymbolConstructor, "match")
                | (NativeFunction::SymbolConstructor, "matchAll")
                | (NativeFunction::SymbolConstructor, "replace")
                | (NativeFunction::SymbolConstructor, "search")
                | (NativeFunction::SymbolConstructor, "species")
                | (NativeFunction::SymbolConstructor, "split")
                | (NativeFunction::SymbolConstructor, "toPrimitive")
                | (NativeFunction::SymbolConstructor, "toStringTag")
                | (NativeFunction::SymbolConstructor, "unscopables")
                | (NativeFunction::Assert, "sameValue")
                | (NativeFunction::Assert, "notSameValue")
                | (NativeFunction::Assert, "throws")
                | (NativeFunction::Assert, "compareArray")
                | (NativeFunction::FunctionConstructor, "prototype")
                | (NativeFunction::GeneratorFunctionConstructor, "prototype")
                | (NativeFunction::Test262Error, "prototype")
                | (NativeFunction::ObjectConstructor, "prototype")
                | (NativeFunction::ArrayConstructor, "prototype")
                | (NativeFunction::StringConstructor, "prototype")
                | (NativeFunction::NumberConstructor, "prototype")
                | (NativeFunction::BooleanConstructor, "prototype")
                | (NativeFunction::ErrorConstructor, "prototype")
                | (NativeFunction::TypeErrorConstructor, "prototype")
                | (NativeFunction::ReferenceErrorConstructor, "prototype")
                | (NativeFunction::SyntaxErrorConstructor, "prototype")
                | (NativeFunction::EvalErrorConstructor, "prototype")
                | (NativeFunction::RangeErrorConstructor, "prototype")
                | (NativeFunction::URIErrorConstructor, "prototype")
                | (NativeFunction::DateConstructor, "prototype")
                | (NativeFunction::ArrayBufferConstructor, "prototype")
                | (NativeFunction::DataViewConstructor, "prototype")
                | (NativeFunction::MapConstructor, "prototype")
                | (NativeFunction::SetConstructor, "prototype")
                | (NativeFunction::PromiseConstructor, "prototype")
                | (NativeFunction::Uint8ArrayConstructor, "prototype")
                | (NativeFunction::RegExpConstructor, "prototype")
                | (NativeFunction::SymbolConstructor, "prototype")
                | (_, "length")
                | (_, "constructor")
        )
    }

    fn is_array_length_tracking_object(&self, object_id: ObjectId) -> Result<bool, VmError> {
        let object = self
            .objects
            .get(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        if !object.properties.contains_key("length") {
            return Ok(false);
        }
        let Some(attributes) = object.property_attributes.get("length") else {
            return Ok(false);
        };
        if attributes.enumerable || attributes.configurable {
            return Ok(false);
        }
        let Some(array_prototype_id) = self.array_prototype_id else {
            return Ok(false);
        };
        self.object_inherits_prototype(object_id, array_prototype_id)
    }

    fn object_inherits_prototype(
        &self,
        object_id: ObjectId,
        target_prototype: ObjectId,
    ) -> Result<bool, VmError> {
        let mut current = Some(object_id);
        while let Some(id) = current {
            if id == target_prototype {
                return Ok(true);
            }
            current = self
                .objects
                .get(&id)
                .ok_or(VmError::UnknownObject(id))?
                .prototype;
        }
        Ok(false)
    }

    fn canonical_array_index(property: &str) -> Option<usize> {
        if property.is_empty() {
            return None;
        }
        if property != "0" && property.starts_with('0') {
            return None;
        }
        let index = property.parse::<u64>().ok()?;
        if index >= u32::MAX as u64 {
            return None;
        }
        if index.to_string() != property {
            return None;
        }
        Some(index as usize)
    }

    fn to_valid_array_length_or_throw(&mut self, value: &JsValue) -> Result<usize, VmError> {
        let number = self.to_number(value);
        let integer = number.trunc();
        if !number.is_finite() || number < 0.0 || integer != number || number > u32::MAX as f64 {
            return Err(VmError::UncaughtException(self.create_error_exception(
                NativeFunction::RangeErrorConstructor,
                "RangeError",
                "Invalid array length".to_string(),
            )));
        }
        Ok(integer as usize)
    }

    fn set_array_length_property(
        &mut self,
        object_id: ObjectId,
        value: &JsValue,
    ) -> Result<(), VmError> {
        let requested_length = self.to_valid_array_length_or_throw(value)?;
        let current_length = self.array_length(object_id)?;
        {
            let object = self
                .objects
                .get_mut(&object_id)
                .ok_or(VmError::UnknownObject(object_id))?;
            if object
                .property_attributes
                .get("length")
                .is_some_and(|attributes| !attributes.writable)
            {
                return Ok(());
            }
            object.properties.insert(
                "length".to_string(),
                JsValue::Number(requested_length as f64),
            );
            object
                .property_attributes
                .entry("length".to_string())
                .or_insert(PropertyAttributes {
                    writable: true,
                    enumerable: false,
                    configurable: false,
                });
        }
        if requested_length >= current_length {
            return Ok(());
        }
        let object = self
            .objects
            .get_mut(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        for index in (requested_length..current_length).rev() {
            let key = index.to_string();
            if object
                .property_attributes
                .get(&key)
                .is_some_and(|attributes| !attributes.configurable)
            {
                object
                    .properties
                    .insert("length".to_string(), JsValue::Number((index + 1) as f64));
                return Ok(());
            }
            object.properties.remove(&key);
            object.getters.remove(&key);
            object.setters.remove(&key);
            object.property_attributes.remove(&key);
        }
        Ok(())
    }

    fn set_array_index_property(
        &mut self,
        object_id: ObjectId,
        index: usize,
        property: String,
        value: JsValue,
    ) -> Result<(), VmError> {
        let current_length = self.array_length(object_id)?;
        let next_length = (index + 1).max(current_length);
        let object = self
            .objects
            .get_mut(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        object.properties.insert(property.clone(), value);
        object
            .property_attributes
            .entry(property)
            .or_insert_with(PropertyAttributes::default);
        if next_length != current_length {
            object
                .properties
                .insert("length".to_string(), JsValue::Number(next_length as f64));
        }
        Ok(())
    }

    fn set_uint8_array_index_property(
        &mut self,
        object_id: ObjectId,
        index: usize,
        value: &JsValue,
    ) -> Result<(), VmError> {
        let length = self
            .objects
            .get(&object_id)
            .and_then(|object| object.properties.get("length"))
            .map(|value| self.to_number(value))
            .unwrap_or(0.0)
            .max(0.0) as usize;
        if index >= length {
            return Ok(());
        }
        let numeric = self.to_number(value);
        let clamped = if !numeric.is_finite() || numeric == 0.0 {
            0.0
        } else {
            let integer = numeric.trunc() as i64;
            let wrapped = ((integer % 256) + 256) % 256;
            wrapped as f64
        };
        let object = self
            .objects
            .get_mut(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        let key = index.to_string();
        object
            .properties
            .insert(key.clone(), JsValue::Number(clamped));
        object
            .property_attributes
            .entry(key)
            .or_insert(PropertyAttributes {
                writable: true,
                enumerable: true,
                configurable: false,
            });
        Ok(())
    }

    fn current_array_literal_target(&self) -> Result<ObjectId, VmError> {
        match self.stack.last() {
            Some(JsValue::Object(object_id)) => Ok(*object_id),
            Some(_) => Err(VmError::TypeError("array literal target expects object")),
            None => Err(VmError::StackUnderflow),
        }
    }

    fn array_push_value(&mut self, object_id: ObjectId, value: JsValue) -> Result<(), VmError> {
        let index = self.array_length(object_id)?;
        let object = self
            .objects
            .get_mut(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        let key = index.to_string();
        object.properties.insert(key.clone(), value);
        object
            .property_attributes
            .entry(key)
            .or_insert_with(PropertyAttributes::default);
        object
            .properties
            .insert("length".to_string(), JsValue::Number((index + 1) as f64));
        Ok(())
    }

    fn array_advance_length(&mut self, object_id: ObjectId, by: usize) -> Result<(), VmError> {
        let index = self.array_length(object_id)?;
        let object = self
            .objects
            .get_mut(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        object
            .properties
            .insert("length".to_string(), JsValue::Number((index + by) as f64));
        Ok(())
    }

    fn array_length(&self, object_id: ObjectId) -> Result<usize, VmError> {
        let object = self
            .objects
            .get(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        let length = object
            .properties
            .get("length")
            .map(|value| self.to_number(value))
            .unwrap_or(0.0)
            .max(0.0);
        Ok(length as usize)
    }

    fn execute_array_join(
        &mut self,
        object_id: ObjectId,
        separator: &str,
    ) -> Result<JsValue, VmError> {
        let length = self.array_length(object_id)?;
        let mut parts = Vec::with_capacity(length);
        for index in 0..length {
            let key = index.to_string();
            let value = self
                .objects
                .get(&object_id)
                .and_then(|object| object.properties.get(&key).cloned());
            let part = match value {
                None | Some(JsValue::Undefined) | Some(JsValue::Null) => String::new(),
                Some(JsValue::Object(nested_id))
                    if self.objects.get(&nested_id).is_some_and(|object| {
                        object.properties.contains_key("length")
                            && object.prototype == self.array_prototype_id
                    }) =>
                {
                    match self.execute_array_join(nested_id, ",")? {
                        JsValue::String(text) => text,
                        other => self.coerce_to_string(&other),
                    }
                }
                Some(value) => self.coerce_to_string(&value),
            };
            parts.push(part);
        }
        Ok(JsValue::String(parts.join(separator)))
    }

    fn execute_array_reverse(&mut self, object_id: ObjectId) -> Result<JsValue, VmError> {
        let length = self.array_length(object_id)?;
        let mut values = Vec::with_capacity(length);
        {
            let object = self
                .objects
                .get(&object_id)
                .ok_or(VmError::UnknownObject(object_id))?;
            for index in 0..length {
                values.push(object.properties.get(&index.to_string()).cloned());
            }
        }

        let object = self
            .objects
            .get_mut(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        for index in 0..length {
            let key = index.to_string();
            object.properties.remove(&key);
            object.property_attributes.remove(&key);
        }
        for (index, value) in values.into_iter().rev().enumerate() {
            let Some(value) = value else {
                continue;
            };
            let key = index.to_string();
            object.properties.insert(key.clone(), value);
            object
                .property_attributes
                .entry(key)
                .or_insert_with(PropertyAttributes::default);
        }
        Ok(JsValue::Object(object_id))
    }

    fn execute_array_sort(
        &mut self,
        object_id: ObjectId,
        args: &[JsValue],
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        let length = self.array_length(object_id)?;
        let mut values = Vec::new();
        {
            let object = self
                .objects
                .get(&object_id)
                .ok_or(VmError::UnknownObject(object_id))?;
            for index in 0..length {
                if let Some(value) = object.properties.get(&index.to_string()).cloned() {
                    values.push(value);
                }
            }
        }
        let compare_fn = match args.first() {
            None | Some(JsValue::Undefined) => None,
            Some(value) if Self::is_callable_value(value) => Some(value.clone()),
            Some(_) => return Err(VmError::TypeError("Array.prototype.sort comparefn")),
        };
        if let Some(compare_fn) = compare_fn {
            let values_len = values.len();
            for left_index in 0..values_len {
                for right_index in (left_index + 1)..values_len {
                    let comparison = self.execute_callable(
                        compare_fn.clone(),
                        Some(JsValue::Undefined),
                        vec![values[left_index].clone(), values[right_index].clone()],
                        realm,
                        caller_strict,
                    )?;
                    let comparison = self.to_number(&comparison);
                    if comparison.is_nan() {
                        continue;
                    }
                    if comparison > 0.0 {
                        values.swap(left_index, right_index);
                    }
                }
            }
        } else {
            values.sort_by_key(|value| self.coerce_to_string(value));
        }

        let object = self
            .objects
            .get_mut(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        for index in 0..length {
            let key = index.to_string();
            object.properties.remove(&key);
            object.property_attributes.remove(&key);
        }
        for (index, value) in values.into_iter().enumerate() {
            let key = index.to_string();
            object.properties.insert(key.clone(), value);
            object
                .property_attributes
                .entry(key)
                .or_insert_with(PropertyAttributes::default);
        }
        Ok(JsValue::Object(object_id))
    }

    fn evaluate_in_operator(
        &mut self,
        key: String,
        right: JsValue,
        realm: &Realm,
    ) -> Result<bool, VmError> {
        match right {
            JsValue::Object(object_id) => self.object_has_property_in_chain(object_id, &key, realm),
            JsValue::Function(closure_id) => {
                self.has_property_on_receiver(&JsValue::Function(closure_id), &key, realm)
            }
            JsValue::NativeFunction(native) => {
                self.has_property_on_receiver(&JsValue::NativeFunction(native), &key, realm)
            }
            JsValue::HostFunction(host_id) => {
                self.has_property_on_receiver(&JsValue::HostFunction(host_id), &key, realm)
            }
            _ => Err(VmError::TypeError("right-hand side of 'in' expects object")),
        }
    }

    fn evaluate_instanceof_operator(
        &mut self,
        left: JsValue,
        right: JsValue,
        realm: &Realm,
    ) -> Result<bool, VmError> {
        if !Self::is_callable_value(&right) {
            return Err(VmError::TypeError(
                "right-hand side of 'instanceof' is not callable",
            ));
        }

        if matches!(
            &right,
            JsValue::NativeFunction(NativeFunction::Test262Error)
        ) {
            return match left {
                JsValue::String(message) => {
                    Ok(Self::error_message_matches(&message, "Test262Error"))
                }
                JsValue::Object(object_id) => {
                    let constructor = self.get_object_property(object_id, "constructor", realm)?;
                    Ok(matches!(
                        constructor,
                        JsValue::NativeFunction(NativeFunction::Test262Error)
                    ))
                }
                _ => Ok(false),
            };
        }

        match left {
            JsValue::String(message) => match right {
                JsValue::NativeFunction(NativeFunction::ErrorConstructor) => {
                    Ok(Self::is_error_string(&message))
                }
                JsValue::NativeFunction(NativeFunction::TypeErrorConstructor) => {
                    Ok(Self::error_message_matches(&message, "TypeError"))
                }
                JsValue::NativeFunction(NativeFunction::ReferenceErrorConstructor) => {
                    Ok(Self::error_message_matches(&message, "ReferenceError"))
                }
                JsValue::NativeFunction(NativeFunction::SyntaxErrorConstructor) => {
                    Ok(Self::error_message_matches(&message, "SyntaxError"))
                }
                JsValue::NativeFunction(NativeFunction::EvalErrorConstructor) => {
                    Ok(Self::error_message_matches(&message, "EvalError"))
                }
                JsValue::NativeFunction(NativeFunction::RangeErrorConstructor) => {
                    Ok(Self::error_message_matches(&message, "RangeError"))
                }
                JsValue::NativeFunction(NativeFunction::URIErrorConstructor) => {
                    Ok(Self::error_message_matches(&message, "URIError"))
                }
                _ => Ok(false),
            },
            _ => {
                if !Self::is_object_like_value(&left) {
                    return Ok(false);
                }
                let prototype = if let JsValue::HostFunction(host_id) = right {
                    if Some(host_id) == self.function_prototype_host_id {
                        if let Some(getter) = self.function_prototype_prototype_getter.clone() {
                            self.execute_callable(
                                getter,
                                Some(JsValue::HostFunction(host_id)),
                                Vec::new(),
                                realm,
                                false,
                            )?
                        } else {
                            self.get_property_from_receiver(
                                JsValue::HostFunction(host_id),
                                "prototype",
                                realm,
                            )?
                        }
                    } else {
                        self.get_property_from_receiver(
                            JsValue::HostFunction(host_id),
                            "prototype",
                            realm,
                        )?
                    }
                } else {
                    self.get_property_from_receiver(right.clone(), "prototype", realm)?
                };
                if !Self::is_object_like_value(&prototype) {
                    return Err(VmError::TypeError(
                        "Function has non-object prototype in instanceof",
                    ));
                }
                if let JsValue::Object(object_id) = left {
                    let constructor = self.get_object_property(object_id, "constructor", realm)?;
                    if self.same_value(&constructor, &right) {
                        return Ok(true);
                    }
                }
                let mut current = self.get_prototype_of_value(&left)?;
                while Self::is_object_like_value(&current) {
                    if self.same_value(&current, &prototype) {
                        return Ok(true);
                    }
                    current = self.get_prototype_of_value(&current)?;
                }
                Ok(false)
            }
        }
    }

    fn is_callable_value(value: &JsValue) -> bool {
        matches!(
            value,
            JsValue::Function(_) | JsValue::NativeFunction(_) | JsValue::HostFunction(_)
        )
    }

    fn object_is_prototype_of(
        &mut self,
        prototype: JsValue,
        value: JsValue,
    ) -> Result<bool, VmError> {
        if !Self::is_object_like_value(&prototype) {
            return Ok(false);
        }
        let mut current = self.get_prototype_of_value(&value)?;
        while Self::is_object_like_value(&current) {
            if self.same_value(&current, &prototype) {
                return Ok(true);
            }
            current = self.get_prototype_of_value(&current)?;
        }
        Ok(false)
    }

    fn is_object_like_value(value: &JsValue) -> bool {
        matches!(
            value,
            JsValue::Object(_)
                | JsValue::Function(_)
                | JsValue::NativeFunction(_)
                | JsValue::HostFunction(_)
        )
    }

    fn get_prototype_of_value(&mut self, value: &JsValue) -> Result<JsValue, VmError> {
        match value {
            JsValue::Object(object_id) => {
                let object = self
                    .objects
                    .get(object_id)
                    .ok_or(VmError::UnknownObject(*object_id))?;
                Ok(object
                    .prototype_value
                    .clone()
                    .or_else(|| object.prototype.map(JsValue::Object))
                    .unwrap_or(JsValue::Null))
            }
            JsValue::Function(closure_id) => {
                if let Some(closure_object) = self.closure_objects.get(closure_id) {
                    if closure_object.prototype_overridden {
                        return Ok(closure_object
                            .prototype_value
                            .clone()
                            .or_else(|| closure_object.prototype.map(JsValue::Object))
                            .unwrap_or(JsValue::Null));
                    }
                    if let Some(parent) = closure_object
                        .properties
                        .get(CLASS_CONSTRUCTOR_PARENT_MARKER)
                        .cloned()
                    {
                        return Ok(parent);
                    }
                }
                if self.closure_is_generator(*closure_id)? {
                    return Ok(self.generator_function_prototype_value());
                }
                Ok(self.function_prototype_value())
            }
            JsValue::HostFunction(host_id) => {
                if let Some(host_object) = self.host_function_objects.get(host_id) {
                    if host_object.prototype_overridden {
                        return Ok(host_object
                            .prototype_value
                            .clone()
                            .or_else(|| host_object.prototype.map(JsValue::Object))
                            .unwrap_or(JsValue::Null));
                    }
                }
                if matches!(value, JsValue::HostFunction(host_id) if Some(*host_id) == self.function_prototype_host_id)
                {
                    Ok(self.object_prototype_value())
                } else {
                    Ok(self.function_prototype_value())
                }
            }
            JsValue::NativeFunction(_) => Ok(self.function_prototype_value()),
            _ => Ok(JsValue::Null),
        }
    }

    fn is_error_string(message: &str) -> bool {
        Self::error_message_matches(message, "TypeError")
            || Self::error_message_matches(message, "ReferenceError")
            || Self::error_message_matches(message, "SyntaxError")
            || Self::error_message_matches(message, "Test262Error")
            || Self::error_message_matches(message, "EvalError")
            || Self::error_message_matches(message, "RangeError")
            || Self::error_message_matches(message, "URIError")
            || Self::error_message_matches(message, "Error")
    }

    fn error_message_matches(message: &str, name: &str) -> bool {
        message == name || message.starts_with(&format!("{name}:"))
    }

    fn delete_property(&mut self, receiver: JsValue, property: String) -> Result<bool, VmError> {
        match receiver {
            JsValue::Object(object_id) => self.delete_object_property(object_id, &property),
            JsValue::Function(closure_id) => {
                Ok(self.delete_closure_property(closure_id, &property))
            }
            JsValue::HostFunction(host_id) => {
                Ok(self.delete_host_function_property(host_id, &property))
            }
            JsValue::NativeFunction(native) => {
                Ok(self.delete_native_function_property(native, &property))
            }
            JsValue::Null | JsValue::Undefined | JsValue::Uninitialized => {
                Err(VmError::TypeError("property access expects object"))
            }
            _ => Ok(true),
        }
    }

    fn delete_object_property(
        &mut self,
        object_id: ObjectId,
        property: &str,
    ) -> Result<bool, VmError> {
        let is_configurable = self
            .objects
            .get(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?
            .property_attributes
            .get(property)
            .map(|attributes| attributes.configurable)
            .unwrap_or(true);
        if !is_configurable {
            return Ok(false);
        }
        let object = self
            .objects
            .get_mut(&object_id)
            .ok_or(VmError::UnknownObject(object_id))?;
        object.properties.remove(property);
        object.getters.remove(property);
        object.setters.remove(property);
        object.property_attributes.remove(property);
        object.argument_mappings.remove(property);
        Ok(true)
    }

    fn delete_closure_property(&mut self, closure_id: u64, property: &str) -> bool {
        let Some(object) = self.closure_objects.get_mut(&closure_id) else {
            return true;
        };
        let is_configurable = object
            .property_attributes
            .get(property)
            .map(|attributes| attributes.configurable)
            .unwrap_or(true);
        if !is_configurable {
            return false;
        }
        object.properties.remove(property);
        object.getters.remove(property);
        object.setters.remove(property);
        object.property_attributes.remove(property);
        object.argument_mappings.remove(property);
        true
    }

    fn delete_host_function_property(&mut self, host_id: u64, property: &str) -> bool {
        if !self.host_functions.contains_key(&host_id) {
            return false;
        }
        let Some(object) = self.host_function_objects.get_mut(&host_id) else {
            return true;
        };
        let is_configurable = object
            .property_attributes
            .get(property)
            .map(|attributes| attributes.configurable)
            .unwrap_or(true);
        if !is_configurable {
            return false;
        }
        object.properties.remove(property);
        object.getters.remove(property);
        object.setters.remove(property);
        object.property_attributes.remove(property);
        object.argument_mappings.remove(property);
        true
    }

    fn delete_native_function_property(&self, native: NativeFunction, property: &str) -> bool {
        if !self.native_function_has_own_property(native, property) {
            return true;
        }
        !matches!(
            (native, property),
            (
                NativeFunction::NumberConstructor,
                "NaN" | "POSITIVE_INFINITY" | "NEGATIVE_INFINITY" | "MAX_VALUE" | "MIN_VALUE"
            )
        )
    }

    fn get_host_function_property(
        &mut self,
        host_id: u64,
        property: &str,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        if !self.host_functions.contains_key(&host_id) {
            return Err(VmError::UnknownHostFunction(host_id));
        }

        let (data_value, getter_value, has_setter) = self
            .host_function_objects
            .get(&host_id)
            .map(|object| {
                (
                    object.properties.get(property).cloned(),
                    object.getters.get(property).cloned(),
                    object.setters.contains_key(property),
                )
            })
            .unwrap_or((None, None, false));
        if let Some(getter) = getter_value {
            return self.execute_callable(
                getter,
                Some(JsValue::HostFunction(host_id)),
                Vec::new(),
                realm,
                false,
            );
        }
        if has_setter {
            return Ok(JsValue::Undefined);
        }
        if let Some(value) = data_value {
            return Ok(value);
        }

        match property {
            "hasOwnProperty" => Ok(
                self.create_host_function_value(HostFunction::HasOwnProperty {
                    target: JsValue::HostFunction(host_id),
                }),
            ),
            "isPrototypeOf" => Ok(
                self.create_host_function_value(HostFunction::IsPrototypeOf {
                    target: JsValue::HostFunction(host_id),
                }),
            ),
            "call" => Ok(self.create_host_function_value(HostFunction::BoundMethod {
                target: JsValue::HostFunction(host_id),
                method: FunctionMethod::Call,
            })),
            "apply" => Ok(self.create_host_function_value(HostFunction::BoundMethod {
                target: JsValue::HostFunction(host_id),
                method: FunctionMethod::Apply,
            })),
            "bind" => Ok(self.create_host_function_value(HostFunction::BoundMethod {
                target: JsValue::HostFunction(host_id),
                method: FunctionMethod::Bind,
            })),
            "length" => Ok(JsValue::Number(0.0)),
            "toString" => Ok(
                self.create_host_function_value(HostFunction::FunctionToString {
                    target: JsValue::HostFunction(host_id),
                }),
            ),
            "valueOf" => Ok(
                self.create_host_function_value(HostFunction::FunctionValueOf {
                    target: JsValue::HostFunction(host_id),
                }),
            ),
            "constructor" => Ok(JsValue::NativeFunction(NativeFunction::FunctionConstructor)),
            _ => Ok(JsValue::Undefined),
        }
    }

    fn get_string_property(&mut self, receiver: &str, property: &str) -> JsValue {
        match property {
            "length" => JsValue::Number(Self::utf16_code_unit_length(receiver) as f64),
            "replace" => self.create_host_function_value(HostFunction::StringReplaceThis),
            "match" => self.create_host_function_value(HostFunction::StringMatchThis),
            "search" => self.create_host_function_value(HostFunction::StringSearchThis),
            "indexOf" => self.create_host_function_value(HostFunction::StringIndexOfThis),
            "split" => self.create_host_function_value(HostFunction::StringSplitThis),
            "toLowerCase" => self.create_host_function_value(HostFunction::StringToLowerCaseThis),
            "toUpperCase" => self.create_host_function_value(HostFunction::StringToUpperCase),
            "toString" => self.create_host_function_value(HostFunction::StringToString),
            "valueOf" => self.create_host_function_value(HostFunction::StringValueOf),
            "charAt" => self.create_host_function_value(HostFunction::StringCharAt),
            "charCodeAt" => self.create_host_function_value(HostFunction::StringCharCodeAt),
            "lastIndexOf" => self.create_host_function_value(HostFunction::StringLastIndexOf),
            "substring" => self.create_host_function_value(HostFunction::StringSubstring),
            "trim" => self.create_host_function_value(HostFunction::StringTrim),
            "constructor" => JsValue::NativeFunction(NativeFunction::StringConstructor),
            _ => match property.parse::<usize>() {
                Ok(index) => receiver
                    .chars()
                    .nth(index)
                    .map(|ch| JsValue::String(ch.to_string()))
                    .unwrap_or(JsValue::Undefined),
                Err(_) => JsValue::Undefined,
            },
        }
    }

    fn utf16_code_unit_length(value: &str) -> usize {
        value.encode_utf16().count()
    }

    fn utf16_code_unit_at(value: &str, index: usize) -> Option<u16> {
        value.encode_utf16().nth(index)
    }

    fn get_function_property(
        &mut self,
        closure_id: u64,
        property: &str,
        realm: &Realm,
    ) -> Result<JsValue, VmError> {
        if self.function_rejects_caller_arguments(closure_id)?
            && matches!(property, "caller" | "arguments")
            && !self.closure_has_own_property(closure_id, property)
        {
            return Err(VmError::TypeError("restricted function property access"));
        }
        if let Some(getter) = self
            .closure_objects
            .get(&closure_id)
            .and_then(|object| object.getters.get(property))
            .cloned()
        {
            return self.execute_callable(
                getter,
                Some(JsValue::Function(closure_id)),
                Vec::new(),
                realm,
                false,
            );
        }
        if self
            .closure_objects
            .get(&closure_id)
            .is_some_and(|object| object.setters.contains_key(property))
        {
            return Ok(JsValue::Undefined);
        }
        if let Some(value) = self
            .closure_objects
            .get(&closure_id)
            .and_then(|object| object.properties.get(property))
            .cloned()
        {
            return Ok(value);
        }
        match property {
            "length" => {
                let closure = self
                    .closures
                    .get(&closure_id)
                    .ok_or(VmError::UnknownClosure(closure_id))?;
                let function = closure
                    .functions
                    .get(closure.function_id)
                    .ok_or(VmError::UnknownFunction(closure.function_id))?;
                Ok(JsValue::Number(function.length as f64))
            }
            "hasOwnProperty" => Ok(
                self.create_host_function_value(HostFunction::HasOwnProperty {
                    target: JsValue::Function(closure_id),
                }),
            ),
            "isPrototypeOf" => Ok(
                self.create_host_function_value(HostFunction::IsPrototypeOf {
                    target: JsValue::Function(closure_id),
                }),
            ),
            "call" => Ok(self.create_host_function_value(HostFunction::BoundMethod {
                target: JsValue::Function(closure_id),
                method: FunctionMethod::Call,
            })),
            "apply" => Ok(self.create_host_function_value(HostFunction::BoundMethod {
                target: JsValue::Function(closure_id),
                method: FunctionMethod::Apply,
            })),
            "bind" => Ok(self.create_host_function_value(HostFunction::BoundMethod {
                target: JsValue::Function(closure_id),
                method: FunctionMethod::Bind,
            })),
            "prototype" => {
                if self.closure_is_arrow(closure_id)? || self.closure_has_no_prototype(closure_id) {
                    Ok(JsValue::Undefined)
                } else {
                    self.get_or_create_function_prototype_property(closure_id)
                }
            }
            "constructor" => Ok(JsValue::NativeFunction(NativeFunction::FunctionConstructor)),
            "toString" => Ok(
                self.create_host_function_value(HostFunction::FunctionToString {
                    target: JsValue::Function(closure_id),
                }),
            ),
            "valueOf" => Ok(
                self.create_host_function_value(HostFunction::FunctionValueOf {
                    target: JsValue::Function(closure_id),
                }),
            ),
            _ => Ok(JsValue::Undefined),
        }
    }

    fn function_name_for_display(&self, value: &JsValue) -> String {
        match value {
            JsValue::Function(closure_id) => self
                .closures
                .get(closure_id)
                .and_then(|closure| closure.functions.get(closure.function_id))
                .map(|function| function.name.clone())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "anonymous".to_string()),
            JsValue::NativeFunction(_) | JsValue::HostFunction(_) => "native".to_string(),
            _ => "anonymous".to_string(),
        }
    }

    fn get_native_function_property(&mut self, native: NativeFunction, property: &str) -> JsValue {
        match (native, property) {
            (NativeFunction::NumberConstructor, "NaN") => JsValue::Number(f64::NAN),
            (NativeFunction::NumberConstructor, "POSITIVE_INFINITY") => {
                JsValue::Number(f64::INFINITY)
            }
            (NativeFunction::NumberConstructor, "NEGATIVE_INFINITY") => {
                JsValue::Number(f64::NEG_INFINITY)
            }
            (NativeFunction::NumberConstructor, "MAX_VALUE") => JsValue::Number(f64::MAX),
            (NativeFunction::NumberConstructor, "MIN_VALUE") => JsValue::Number(f64::from_bits(1)),
            (NativeFunction::ObjectConstructor, "defineProperty") => {
                JsValue::NativeFunction(NativeFunction::ObjectDefineProperty)
            }
            (NativeFunction::ObjectConstructor, "defineProperties") => {
                JsValue::NativeFunction(NativeFunction::ObjectDefineProperties)
            }
            (NativeFunction::ArrayConstructor, "isArray") => {
                JsValue::NativeFunction(NativeFunction::ArrayIsArray)
            }
            (NativeFunction::ObjectConstructor, "keys") => {
                JsValue::NativeFunction(NativeFunction::ObjectKeys)
            }
            (NativeFunction::ObjectConstructor, "getOwnPropertyNames") => {
                JsValue::NativeFunction(NativeFunction::ObjectGetOwnPropertyNames)
            }
            (NativeFunction::ObjectConstructor, "create") => {
                JsValue::NativeFunction(NativeFunction::ObjectCreate)
            }
            (NativeFunction::ObjectConstructor, "setPrototypeOf") => {
                JsValue::NativeFunction(NativeFunction::ObjectSetPrototypeOf)
            }
            (NativeFunction::ObjectConstructor, "preventExtensions") => {
                JsValue::NativeFunction(NativeFunction::ObjectPreventExtensions)
            }
            (NativeFunction::ObjectConstructor, "__forInKeys") => {
                JsValue::NativeFunction(NativeFunction::ObjectForInKeys)
            }
            (NativeFunction::ObjectConstructor, "__forOfValues") => {
                JsValue::NativeFunction(NativeFunction::ObjectForOfValues)
            }
            (NativeFunction::ObjectConstructor, "__forOfIterator") => {
                JsValue::NativeFunction(NativeFunction::ObjectForOfIterator)
            }
            (NativeFunction::ObjectConstructor, "__forOfStep") => {
                JsValue::NativeFunction(NativeFunction::ObjectForOfStep)
            }
            (NativeFunction::ObjectConstructor, "__forOfClose") => {
                JsValue::NativeFunction(NativeFunction::ObjectForOfClose)
            }
            (NativeFunction::ObjectConstructor, "__getTemplateObject") => {
                JsValue::NativeFunction(NativeFunction::ObjectGetTemplateObject)
            }
            (NativeFunction::ObjectConstructor, "__tdzMarker") => {
                JsValue::NativeFunction(NativeFunction::ObjectTdzMarker)
            }
            (NativeFunction::ObjectConstructor, "getOwnPropertyDescriptor") => {
                JsValue::NativeFunction(NativeFunction::ObjectGetOwnPropertyDescriptor)
            }
            (NativeFunction::ObjectConstructor, "getPrototypeOf") => {
                JsValue::NativeFunction(NativeFunction::ObjectGetPrototypeOf)
            }
            (NativeFunction::ObjectConstructor, "isExtensible") => {
                JsValue::NativeFunction(NativeFunction::ObjectIsExtensible)
            }
            (NativeFunction::ObjectConstructor, "freeze") => {
                JsValue::NativeFunction(NativeFunction::ObjectFreeze)
            }
            (NativeFunction::ObjectConstructor, "toString") => {
                self.create_host_function_value(HostFunction::FunctionToString {
                    target: JsValue::NativeFunction(NativeFunction::ObjectConstructor),
                })
            }
            (NativeFunction::ObjectConstructor, "valueOf") => {
                self.create_host_function_value(HostFunction::FunctionValueOf {
                    target: JsValue::NativeFunction(NativeFunction::ObjectConstructor),
                })
            }
            (NativeFunction::FunctionConstructor, "prototype") => self.function_prototype_value(),
            (NativeFunction::GeneratorFunctionConstructor, "prototype") => {
                self.generator_function_prototype_value()
            }
            (NativeFunction::ObjectConstructor, "prototype") => self.object_prototype_value(),
            (NativeFunction::ArrayConstructor, "prototype") => self.array_prototype_value(),
            (NativeFunction::StringConstructor, "prototype") => self.string_prototype_value(),
            (NativeFunction::NumberConstructor, "prototype") => self.number_prototype_value(),
            (NativeFunction::BooleanConstructor, "prototype") => self.boolean_prototype_value(),
            (NativeFunction::ErrorConstructor, "prototype") => self.error_prototype_value(),
            (NativeFunction::TypeErrorConstructor, "prototype") => {
                self.type_error_prototype_value()
            }
            (NativeFunction::Test262Error, "prototype") => self.error_prototype_value(),
            (NativeFunction::ReferenceErrorConstructor, "prototype")
            | (NativeFunction::SyntaxErrorConstructor, "prototype")
            | (NativeFunction::EvalErrorConstructor, "prototype")
            | (NativeFunction::RangeErrorConstructor, "prototype")
            | (NativeFunction::URIErrorConstructor, "prototype") => self.error_prototype_value(),
            (NativeFunction::DateConstructor, "prototype") => self.date_prototype_value(),
            (NativeFunction::ArrayBufferConstructor, "prototype") => {
                self.array_buffer_prototype_value()
            }
            (NativeFunction::DataViewConstructor, "prototype") => self.data_view_prototype_value(),
            (NativeFunction::MapConstructor, "prototype") => self.map_prototype_value(),
            (NativeFunction::SetConstructor, "prototype") => self.set_prototype_value(),
            (NativeFunction::PromiseConstructor, "prototype") => self.promise_prototype_value(),
            (NativeFunction::Uint8ArrayConstructor, "prototype") => {
                self.uint8_array_prototype_value()
            }
            (NativeFunction::RegExpConstructor, "prototype") => self.regexp_prototype_value(),
            (NativeFunction::SymbolConstructor, "prototype") => self.create_object_value(),
            (NativeFunction::DateConstructor, "parse") => {
                JsValue::NativeFunction(NativeFunction::DateParse)
            }
            (NativeFunction::DateConstructor, "UTC") => {
                JsValue::NativeFunction(NativeFunction::DateUtc)
            }
            (NativeFunction::SymbolConstructor, "iterator") => {
                JsValue::String("Symbol.iterator".to_string())
            }
            (NativeFunction::SymbolConstructor, "asyncIterator") => {
                JsValue::String("Symbol.asyncIterator".to_string())
            }
            (NativeFunction::SymbolConstructor, "hasInstance") => {
                JsValue::String("Symbol.hasInstance".to_string())
            }
            (NativeFunction::SymbolConstructor, "isConcatSpreadable") => {
                JsValue::String("Symbol.isConcatSpreadable".to_string())
            }
            (NativeFunction::SymbolConstructor, "match") => {
                JsValue::String("Symbol.match".to_string())
            }
            (NativeFunction::SymbolConstructor, "matchAll") => {
                JsValue::String("Symbol.matchAll".to_string())
            }
            (NativeFunction::SymbolConstructor, "replace") => {
                JsValue::String("Symbol.replace".to_string())
            }
            (NativeFunction::SymbolConstructor, "search") => {
                JsValue::String("Symbol.search".to_string())
            }
            (NativeFunction::SymbolConstructor, "species") => {
                JsValue::String("Symbol.species".to_string())
            }
            (NativeFunction::SymbolConstructor, "split") => {
                JsValue::String("Symbol.split".to_string())
            }
            (NativeFunction::SymbolConstructor, "toPrimitive") => {
                JsValue::String("Symbol.toPrimitive".to_string())
            }
            (NativeFunction::SymbolConstructor, "toStringTag") => {
                JsValue::String("Symbol.toStringTag".to_string())
            }
            (NativeFunction::SymbolConstructor, "unscopables") => {
                JsValue::String("Symbol.unscopables".to_string())
            }
            (NativeFunction::Assert, "sameValue") => {
                self.create_host_function_value(HostFunction::AssertSameValue)
            }
            (NativeFunction::Assert, "notSameValue") => {
                self.create_host_function_value(HostFunction::AssertNotSameValue)
            }
            (NativeFunction::Assert, "throws") => {
                self.create_host_function_value(HostFunction::AssertThrows)
            }
            (NativeFunction::Assert, "compareArray") => {
                self.create_host_function_value(HostFunction::AssertCompareArray)
            }
            (NativeFunction::StringConstructor, "fromCharCode") => {
                JsValue::NativeFunction(NativeFunction::StringFromCharCode)
            }
            (_, "toString") => self.create_host_function_value(HostFunction::FunctionToString {
                target: JsValue::NativeFunction(native),
            }),
            (_, "valueOf") => self.create_host_function_value(HostFunction::FunctionValueOf {
                target: JsValue::NativeFunction(native),
            }),
            (_, "constructor") => JsValue::NativeFunction(NativeFunction::FunctionConstructor),
            (_, "hasOwnProperty") => {
                self.create_host_function_value(HostFunction::HasOwnProperty {
                    target: JsValue::NativeFunction(native),
                })
            }
            (_, "isPrototypeOf") => self.create_host_function_value(HostFunction::IsPrototypeOf {
                target: JsValue::NativeFunction(native),
            }),
            (_, "call") => self.create_host_function_value(HostFunction::BoundMethod {
                target: JsValue::NativeFunction(native),
                method: FunctionMethod::Call,
            }),
            (_, "apply") => self.create_host_function_value(HostFunction::BoundMethod {
                target: JsValue::NativeFunction(native),
                method: FunctionMethod::Apply,
            }),
            (_, "bind") => self.create_host_function_value(HostFunction::BoundMethod {
                target: JsValue::NativeFunction(native),
                method: FunctionMethod::Bind,
            }),
            (_, "length") => JsValue::Number(1.0),
            _ => JsValue::Undefined,
        }
    }

    fn evaluate_add(&mut self, realm: &Realm, caller_strict: bool) -> Result<JsValue, VmError> {
        let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        let left = self.primitive_for_add(left, realm, caller_strict)?;
        let right = self.primitive_for_add(right, realm, caller_strict)?;
        Ok(match (left, right) {
            (JsValue::String(lhs), rhs) => {
                let rhs = self.coerce_to_string(&rhs);
                JsValue::String(format!("{lhs}{rhs}"))
            }
            (lhs, JsValue::String(rhs)) => {
                let lhs = self.coerce_to_string(&lhs);
                JsValue::String(format!("{lhs}{rhs}"))
            }
            (lhs, rhs) => JsValue::Number(self.to_number(&lhs) + self.to_number(&rhs)),
        })
    }

    fn primitive_for_numeric(
        &mut self,
        value: JsValue,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<JsValue, VmError> {
        match value {
            JsValue::Object(object_id) => {
                if let Some(boxed) = self.boxed_primitive_value(object_id) {
                    return Ok(boxed);
                }
                self.ordinary_to_primitive_for_add(object_id, false, realm, caller_strict)
            }
            JsValue::Function(closure_id) => {
                self.ordinary_to_primitive_for_function(closure_id, false, realm, caller_strict)
            }
            other => Ok(other),
        }
    }

    fn coerce_number_runtime(
        &mut self,
        value: JsValue,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<f64, VmError> {
        let primitive = self.primitive_for_numeric(value, realm, caller_strict)?;
        Ok(self.to_number(&primitive))
    }

    fn coerce_uint32_runtime(
        &mut self,
        value: JsValue,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<u32, VmError> {
        let number = self.coerce_number_runtime(value, realm, caller_strict)?;
        Ok(Self::to_uint32_number(number))
    }

    fn coerce_int32_runtime(
        &mut self,
        value: JsValue,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<i32, VmError> {
        let number = self.coerce_number_runtime(value, realm, caller_strict)?;
        Ok(Self::to_int32_number(number))
    }

    fn eval_numeric_binary(
        &mut self,
        realm: &Realm,
        caller_strict: bool,
        op: impl FnOnce(f64, f64) -> f64,
    ) -> Result<f64, VmError> {
        let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        let left = self.coerce_number_runtime(left, realm, caller_strict)?;
        let right = self.coerce_number_runtime(right, realm, caller_strict)?;
        Ok(op(left, right))
    }

    fn abstract_relational_compare(
        &mut self,
        left: JsValue,
        right: JsValue,
        left_first: bool,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<Option<bool>, VmError> {
        let (left_primitive, right_primitive) = if left_first {
            (
                self.primitive_for_numeric(left, realm, caller_strict)?,
                self.primitive_for_numeric(right, realm, caller_strict)?,
            )
        } else {
            let right_primitive = self.primitive_for_numeric(right, realm, caller_strict)?;
            let left_primitive = self.primitive_for_numeric(left, realm, caller_strict)?;
            (left_primitive, right_primitive)
        };

        if let (JsValue::String(left_string), JsValue::String(right_string)) =
            (&left_primitive, &right_primitive)
        {
            return Ok(Some(self.js_string_less_than(left_string, right_string)));
        }

        let left_number = self.to_number(&left_primitive);
        let right_number = self.to_number(&right_primitive);
        if left_number.is_nan() || right_number.is_nan() {
            return Ok(None);
        }
        Ok(Some(left_number < right_number))
    }

    fn string_to_js_code_units(&self, value: &str) -> Vec<u16> {
        let mut units = Vec::with_capacity(value.len());
        for ch in value.chars() {
            let scalar = ch as u32;
            if (SURROGATE_PLACEHOLDER_START..=SURROGATE_PLACEHOLDER_END).contains(&scalar) {
                units
                    .push((SURROGATE_START as u32 + (scalar - SURROGATE_PLACEHOLDER_START)) as u16);
                continue;
            }
            let mut buf = [0u16; 2];
            units.extend_from_slice(ch.encode_utf16(&mut buf));
        }
        units
    }

    fn js_string_iterator_values(&self, value: &str) -> Vec<JsValue> {
        let units = self.string_to_js_code_units(value);
        let mut values = Vec::with_capacity(units.len());
        let mut index = 0usize;
        while index < units.len() {
            let unit = units[index];
            if (0xD800..=0xDBFF).contains(&unit) && index + 1 < units.len() {
                let next = units[index + 1];
                if (0xDC00..=0xDFFF).contains(&next) {
                    let code_point =
                        0x1_0000 + (((unit as u32) - 0xD800) << 10) + ((next as u32) - 0xDC00);
                    if let Some(ch) = char::from_u32(code_point) {
                        values.push(JsValue::String(ch.to_string()));
                        index += 2;
                        continue;
                    }
                }
            }
            let ch = if (0xD800..=0xDFFF).contains(&unit) {
                let placeholder =
                    SURROGATE_PLACEHOLDER_START + ((unit as u32) - (SURROGATE_START as u32));
                char::from_u32(placeholder).unwrap_or('\u{FFFD}')
            } else {
                char::from_u32(unit as u32).unwrap_or('\u{FFFD}')
            };
            values.push(JsValue::String(ch.to_string()));
            index += 1;
        }
        values
    }

    fn js_string_less_than(&self, left: &str, right: &str) -> bool {
        self.string_to_js_code_units(left) < self.string_to_js_code_units(right)
    }

    fn eval_relational_operator(
        &mut self,
        realm: &Realm,
        caller_strict: bool,
        op: Opcode,
    ) -> Result<bool, VmError> {
        let right = self.stack.pop().ok_or(VmError::StackUnderflow)?;
        let left = self.stack.pop().ok_or(VmError::StackUnderflow)?;

        match op {
            Opcode::Lt => Ok(self
                .abstract_relational_compare(left, right, true, realm, caller_strict)?
                .unwrap_or(false)),
            Opcode::Gt => Ok(self
                .abstract_relational_compare(right, left, false, realm, caller_strict)?
                .unwrap_or(false)),
            Opcode::Le => {
                let compared =
                    self.abstract_relational_compare(right, left, false, realm, caller_strict)?;
                Ok(compared.is_some_and(|result| !result))
            }
            Opcode::Ge => {
                let compared =
                    self.abstract_relational_compare(left, right, true, realm, caller_strict)?;
                Ok(compared.is_some_and(|result| !result))
            }
            _ => unreachable!("invalid relational opcode"),
        }
    }

    fn strict_equality_compare(&self, left: &JsValue, right: &JsValue) -> bool {
        match (left, right) {
            (JsValue::Number(lhs), JsValue::Number(rhs)) => {
                !lhs.is_nan() && !rhs.is_nan() && lhs == rhs
            }
            _ => left == right,
        }
    }

    fn abstract_equality_compare(
        &mut self,
        left: JsValue,
        right: JsValue,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<bool, VmError> {
        if std::mem::discriminant(&left) == std::mem::discriminant(&right) {
            return Ok(self.strict_equality_compare(&left, &right));
        }

        match (left, right) {
            (JsValue::Null, JsValue::Undefined) | (JsValue::Undefined, JsValue::Null) => Ok(true),
            (JsValue::Number(number), JsValue::String(string)) => {
                let rhs = self.to_number(&JsValue::String(string));
                Ok(!number.is_nan() && !rhs.is_nan() && number == rhs)
            }
            (JsValue::String(string), JsValue::Number(number)) => {
                let lhs = self.to_number(&JsValue::String(string));
                Ok(!lhs.is_nan() && !number.is_nan() && lhs == number)
            }
            (JsValue::Bool(boolean), other) => {
                let number = if boolean { 1.0 } else { 0.0 };
                self.abstract_equality_compare(JsValue::Number(number), other, realm, caller_strict)
            }
            (other, JsValue::Bool(boolean)) => {
                let number = if boolean { 1.0 } else { 0.0 };
                self.abstract_equality_compare(other, JsValue::Number(number), realm, caller_strict)
            }
            (
                left @ (JsValue::Object(_) | JsValue::Function(_)),
                right @ (JsValue::String(_) | JsValue::Number(_)),
            ) => {
                let primitive = self.primitive_for_numeric(left, realm, caller_strict)?;
                self.abstract_equality_compare(primitive, right, realm, caller_strict)
            }
            (
                left @ (JsValue::String(_) | JsValue::Number(_)),
                right @ (JsValue::Object(_) | JsValue::Function(_)),
            ) => {
                let primitive = self.primitive_for_numeric(right, realm, caller_strict)?;
                self.abstract_equality_compare(left, primitive, realm, caller_strict)
            }
            _ => Ok(false),
        }
    }

    fn coerce_to_string(&self, value: &JsValue) -> String {
        match value {
            JsValue::Number(number) => Self::coerce_number_to_string(*number),
            JsValue::Bool(boolean) => boolean.to_string(),
            JsValue::Null => "null".to_string(),
            JsValue::String(value) => value.clone(),
            JsValue::Function(_) | JsValue::NativeFunction(_) | JsValue::HostFunction(_) => {
                "[function]".to_string()
            }
            JsValue::Object(object_id) => self
                .boxed_primitive_value(*object_id)
                .map_or("[object Object]".to_string(), |value| {
                    self.coerce_to_string(&value)
                }),
            JsValue::Uninitialized => "undefined".to_string(),
            JsValue::Undefined => "undefined".to_string(),
        }
    }

    fn coerce_number_to_string(number: f64) -> String {
        if number.is_nan() {
            return "NaN".to_string();
        }
        if number == 0.0 {
            return "0".to_string();
        }
        if number.is_infinite() {
            return if number.is_sign_positive() {
                "Infinity".to_string()
            } else {
                "-Infinity".to_string()
            };
        }
        let abs = number.abs();
        if !(1e-6..1e21).contains(&abs) {
            let scientific = format!("{:e}", number);
            let (mantissa_raw, exponent_raw) = scientific
                .split_once('e')
                .expect("scientific formatting must contain exponent");
            let mut mantissa = mantissa_raw.to_string();
            if mantissa.contains('.') {
                while mantissa.ends_with('0') {
                    mantissa.pop();
                }
                if mantissa.ends_with('.') {
                    mantissa.pop();
                }
            }
            return Self::normalize_exponent_string(format!("{mantissa}e{exponent_raw}"));
        }
        Self::normalize_exponent_string(number.to_string())
    }

    fn normalize_exponent_string(string: String) -> String {
        let Some(exponent_pos) = string.find('e') else {
            return string;
        };
        let mantissa = &string[..exponent_pos];
        let exponent_raw = &string[exponent_pos + 1..];
        let (sign, digits_raw) = if let Some(rest) = exponent_raw.strip_prefix('+') {
            ('+', rest)
        } else if let Some(rest) = exponent_raw.strip_prefix('-') {
            ('-', rest)
        } else {
            ('+', exponent_raw)
        };
        let digits = digits_raw.trim_start_matches('0');
        let digits = if digits.is_empty() { "0" } else { digits };
        format!("{mantissa}e{sign}{digits}")
    }

    fn typeof_value(&self, value: &JsValue) -> &'static str {
        match value {
            JsValue::Undefined => "undefined",
            JsValue::Uninitialized => "undefined",
            JsValue::Null => "object",
            JsValue::Bool(_) => "boolean",
            JsValue::Number(_) => "number",
            JsValue::String(_) => "string",
            JsValue::Function(_) | JsValue::NativeFunction(_) | JsValue::HostFunction(_) => {
                "function"
            }
            JsValue::Object(_) => "object",
        }
    }

    fn coerce_to_property_key(&self, value: &JsValue) -> String {
        self.coerce_to_string(value)
    }

    fn coerce_to_property_key_runtime(
        &mut self,
        value: JsValue,
        realm: &Realm,
        caller_strict: bool,
    ) -> Result<String, VmError> {
        match value {
            JsValue::Object(object_id) => self
                .ordinary_to_primitive_for_property_key(object_id, realm, caller_strict)
                .map(|primitive| self.coerce_to_string(&primitive)),
            JsValue::Function(closure_id) => {
                let to_string = self.get_function_property(closure_id, "toString", realm)?;
                if Self::is_callable_value(&to_string) {
                    let primitive = self.execute_callable(
                        to_string,
                        Some(JsValue::Function(closure_id)),
                        Vec::new(),
                        realm,
                        caller_strict,
                    )?;
                    Ok(self.coerce_to_string(&primitive))
                } else {
                    Ok(self.coerce_to_string(&JsValue::Function(closure_id)))
                }
            }
            JsValue::NativeFunction(native) => {
                let to_string = self.get_native_function_property(native, "toString");
                if Self::is_callable_value(&to_string) {
                    let primitive = self.execute_callable(
                        to_string,
                        Some(JsValue::NativeFunction(native)),
                        Vec::new(),
                        realm,
                        caller_strict,
                    )?;
                    Ok(self.coerce_to_string(&primitive))
                } else {
                    Ok(self.coerce_to_string(&JsValue::NativeFunction(native)))
                }
            }
            JsValue::HostFunction(host_id) => {
                let to_string = self.get_host_function_property(host_id, "toString", realm)?;
                if Self::is_callable_value(&to_string) {
                    let primitive = self.execute_callable(
                        to_string,
                        Some(JsValue::HostFunction(host_id)),
                        Vec::new(),
                        realm,
                        caller_strict,
                    )?;
                    Ok(self.coerce_to_string(&primitive))
                } else {
                    Ok(self.coerce_to_string(&JsValue::HostFunction(host_id)))
                }
            }
            primitive => Ok(self.coerce_to_string(&primitive)),
        }
    }

    fn execute_json_stringify(&self, value: Option<&JsValue>) -> JsValue {
        let Some(value) = value else {
            return JsValue::Undefined;
        };
        match value {
            JsValue::Undefined
            | JsValue::Function(_)
            | JsValue::NativeFunction(_)
            | JsValue::HostFunction(_) => JsValue::Undefined,
            JsValue::Null => JsValue::String("null".to_string()),
            JsValue::Bool(boolean) => JsValue::String(boolean.to_string()),
            JsValue::Number(number) => {
                if number.is_finite() {
                    JsValue::String(Self::coerce_number_to_string(*number))
                } else {
                    JsValue::String("null".to_string())
                }
            }
            JsValue::String(text) => {
                JsValue::String(format!("\"{}\"", Self::escape_json_string(text)))
            }
            JsValue::Object(object_id) => {
                if self
                    .objects
                    .get(object_id)
                    .is_some_and(|object| object.properties.contains_key("length"))
                {
                    JsValue::String("[]".to_string())
                } else {
                    JsValue::String("{}".to_string())
                }
            }
            JsValue::Uninitialized => JsValue::Undefined,
        }
    }

    fn execute_json_parse(&self, value: Option<&JsValue>) -> JsValue {
        let Some(value) = value else {
            return JsValue::Undefined;
        };
        let text = self.coerce_to_string(value);
        match text.trim() {
            "null" => JsValue::Null,
            "true" => JsValue::Bool(true),
            "false" => JsValue::Bool(false),
            source => source
                .parse::<f64>()
                .map(JsValue::Number)
                .unwrap_or(JsValue::Undefined),
        }
    }

    fn escape_json_string(value: &str) -> String {
        let mut escaped = String::with_capacity(value.len());
        for ch in value.chars() {
            match ch {
                '"' => escaped.push_str("\\\""),
                '\\' => escaped.push_str("\\\\"),
                '\n' => escaped.push_str("\\n"),
                '\r' => escaped.push_str("\\r"),
                '\t' => escaped.push_str("\\t"),
                '\u{08}' => escaped.push_str("\\b"),
                '\u{0C}' => escaped.push_str("\\f"),
                other => escaped.push(other),
            }
        }
        escaped
    }

    fn parse_int_baseline(&self, args: &[JsValue]) -> f64 {
        let input = args.first().map_or_else(
            || "undefined".to_string(),
            |value| self.coerce_to_string(value),
        );
        let mut source = input.trim_start();

        let mut sign = 1.0;
        if let Some(rest) = source.strip_prefix('-') {
            sign = -1.0;
            source = rest;
        } else if let Some(rest) = source.strip_prefix('+') {
            source = rest;
        }

        let mut radix = match args.get(1) {
            None | Some(JsValue::Undefined) => 0,
            Some(value) => {
                let number = self.to_number(value);
                if number == 0.0 || !number.is_finite() {
                    0
                } else {
                    Self::to_int32_number(number)
                }
            }
        };

        let mut allow_hex_prefix = true;
        if radix != 0 {
            if !(2..=36).contains(&radix) {
                return f64::NAN;
            }
            allow_hex_prefix = radix == 16;
        }

        if allow_hex_prefix {
            if let Some(rest) = source
                .strip_prefix("0x")
                .or_else(|| source.strip_prefix("0X"))
            {
                source = rest;
                if radix == 0 {
                    radix = 16;
                }
            }
        }

        if radix == 0 {
            radix = 10;
        }

        let mut value = 0.0;
        let mut found_digit = false;
        for ch in source.chars() {
            let Some(digit) = Self::parse_int_digit(ch) else {
                break;
            };
            if digit >= radix as u32 {
                break;
            }
            found_digit = true;
            value = value * (radix as f64) + (digit as f64);
        }

        if !found_digit { f64::NAN } else { sign * value }
    }

    fn parse_float_baseline(&self, args: &[JsValue]) -> f64 {
        let input = args.first().map_or_else(
            || "undefined".to_string(),
            |value| self.coerce_to_string(value),
        );
        let source = input.trim_start();
        if source.is_empty() {
            return f64::NAN;
        }

        if let Some(rest) = source.strip_prefix('+') {
            if rest.starts_with("Infinity") {
                return f64::INFINITY;
            }
        } else if let Some(rest) = source.strip_prefix('-') {
            if rest.starts_with("Infinity") {
                return f64::NEG_INFINITY;
            }
        } else if source.starts_with("Infinity") {
            return f64::INFINITY;
        }

        let mut candidate = String::new();
        for ch in source.chars() {
            if matches!(ch, '0'..='9' | '+' | '-' | '.' | 'e' | 'E') {
                candidate.push(ch);
            } else {
                break;
            }
        }

        if candidate.is_empty() {
            return f64::NAN;
        }

        let mut best: Option<(usize, f64)> = None;
        for end in 1..=candidate.len() {
            if !candidate.is_char_boundary(end) {
                continue;
            }
            let prefix = &candidate[..end];
            if let Ok(value) = prefix.parse::<f64>() {
                best = Some((end, value));
            }
        }

        best.map(|(_, value)| value).unwrap_or(f64::NAN)
    }

    fn parse_int_digit(ch: char) -> Option<u32> {
        match ch {
            '0'..='9' => Some((ch as u32) - ('0' as u32)),
            'a'..='z' => Some((ch as u32) - ('a' as u32) + 10),
            'A'..='Z' => Some((ch as u32) - ('A' as u32) + 10),
            _ => None,
        }
    }

    fn resolve_super_base_value(&self) -> Option<JsValue> {
        let binding_id = self.resolve_binding_id("this")?;
        let this_value = self.bindings.get(&binding_id)?.value.clone();
        match this_value {
            JsValue::Object(object_id) => {
                let object = self.objects.get(&object_id)?;
                Some(
                    object
                        .prototype_value
                        .clone()
                        .or_else(|| object.prototype.map(JsValue::Object))
                        .unwrap_or(JsValue::Null),
                )
            }
            _ => None,
        }
    }

    fn to_number(&self, value: &JsValue) -> f64 {
        match value {
            JsValue::Number(number) => *number,
            JsValue::Bool(boolean) => {
                if *boolean {
                    1.0
                } else {
                    0.0
                }
            }
            JsValue::Null => 0.0,
            JsValue::String(value) => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    0.0
                } else if let Some(hex) = trimmed
                    .strip_prefix("0x")
                    .or_else(|| trimmed.strip_prefix("0X"))
                {
                    if hex.is_empty() {
                        f64::NAN
                    } else {
                        u64::from_str_radix(hex, 16)
                            .map(|number| number as f64)
                            .unwrap_or(f64::NAN)
                    }
                } else {
                    let parsed = trimmed.parse::<f64>().unwrap_or(f64::NAN);
                    if parsed.is_infinite()
                        && !matches!(trimmed, "Infinity" | "+Infinity" | "-Infinity")
                    {
                        f64::NAN
                    } else {
                        parsed
                    }
                }
            }
            JsValue::Function(_) | JsValue::NativeFunction(_) | JsValue::HostFunction(_) => {
                f64::NAN
            }
            JsValue::Object(object_id) => self
                .boxed_primitive_value(*object_id)
                .map_or(f64::NAN, |value| self.to_number(&value)),
            JsValue::Uninitialized => f64::NAN,
            JsValue::Undefined => f64::NAN,
        }
    }

    fn to_uint32_number(number: f64) -> u32 {
        if !number.is_finite() || number == 0.0 {
            return 0;
        }
        let modulo = 4_294_967_296_f64;
        let mut int = number.trunc() % modulo;
        if int < 0.0 {
            int += modulo;
        }
        int as u32
    }

    fn to_int32_number(number: f64) -> i32 {
        let uint = Self::to_uint32_number(number);
        if uint >= 0x8000_0000 {
            (uint as i64 - 0x1_0000_0000_i64) as i32
        } else {
            uint as i32
        }
    }

    fn same_value(&self, left: &JsValue, right: &JsValue) -> bool {
        match (left, right) {
            (JsValue::Number(lhs), JsValue::Number(rhs)) => {
                if lhs.is_nan() && rhs.is_nan() {
                    return true;
                }
                if lhs == rhs {
                    if *lhs == 0.0 {
                        return lhs.is_sign_positive() == rhs.is_sign_positive();
                    }
                    return true;
                }
                false
            }
            (JsValue::Bool(lhs), JsValue::Bool(rhs)) => lhs == rhs,
            (JsValue::Null, JsValue::Null) => true,
            (JsValue::String(lhs), JsValue::String(rhs)) => lhs == rhs,
            (JsValue::Function(lhs), JsValue::Function(rhs)) => lhs == rhs,
            (JsValue::NativeFunction(lhs), JsValue::NativeFunction(rhs)) => lhs == rhs,
            (JsValue::HostFunction(lhs), JsValue::HostFunction(rhs)) => lhs == rhs,
            (JsValue::Object(lhs), JsValue::Object(rhs)) => lhs == rhs,
            (JsValue::Undefined, JsValue::Undefined) => true,
            _ => false,
        }
    }

    fn assertion_failure(&self, detail: &str) -> VmError {
        VmError::UncaughtException(JsValue::String(format!("Assertion failed: {detail}")))
    }

    fn compare_array_like(&self, actual: &JsValue, expected: &JsValue) -> Result<bool, VmError> {
        let actual_id = match actual {
            JsValue::Object(id) => *id,
            _ => return Ok(false),
        };
        let expected_id = match expected {
            JsValue::Object(id) => *id,
            _ => return Ok(false),
        };

        let actual_object = self
            .objects
            .get(&actual_id)
            .ok_or(VmError::UnknownObject(actual_id))?;
        let expected_object = self
            .objects
            .get(&expected_id)
            .ok_or(VmError::UnknownObject(expected_id))?;

        let actual_length = actual_object
            .properties
            .get("length")
            .map(|value| self.to_number(value))
            .unwrap_or(0.0)
            .max(0.0) as usize;
        let expected_length = expected_object
            .properties
            .get("length")
            .map(|value| self.to_number(value))
            .unwrap_or(0.0)
            .max(0.0) as usize;
        if actual_length != expected_length {
            return Ok(false);
        }

        for index in 0..actual_length {
            let key = index.to_string();
            let actual_value = actual_object
                .properties
                .get(&key)
                .cloned()
                .unwrap_or(JsValue::Undefined);
            let expected_value = expected_object
                .properties
                .get(&key)
                .cloned()
                .unwrap_or(JsValue::Undefined);
            if !self.same_value(&actual_value, &expected_value) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn is_truthy(&self, value: &JsValue) -> bool {
        match value {
            JsValue::Undefined => false,
            JsValue::Uninitialized => false,
            JsValue::Null => false,
            JsValue::Bool(boolean) => *boolean,
            JsValue::Number(number) => *number != 0.0 && !number.is_nan(),
            JsValue::String(value) => !value.is_empty(),
            JsValue::Function(_) | JsValue::NativeFunction(_) | JsValue::HostFunction(_) => true,
            JsValue::Object(_) => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GcStats, Vm, VmError};
    use bytecode::{Chunk, CompiledFunction, Opcode, compile_script};
    use parser::parse_script;
    use runtime::{JsValue, NativeFunction, Realm};

    fn empty_chunk(code: Vec<Opcode>) -> Chunk {
        Chunk {
            code,
            functions: vec![],
        }
    }

    #[test]
    fn executes_addition() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::LoadNumber(2.0),
            Opcode::Add,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(3.0)));
    }

    #[test]
    fn duplicates_top_stack_value() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::Dup,
            Opcode::Add,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(2.0)));
    }

    #[test]
    fn concatenates_when_string_is_present_in_addition() {
        let chunk = empty_chunk(vec![
            Opcode::LoadString("qjs".to_string()),
            Opcode::LoadNumber(1.0),
            Opcode::Add,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::String("qjs1".to_string())));
    }

    #[test]
    fn executes_mixed_arithmetic() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(20.0),
            Opcode::LoadNumber(5.0),
            Opcode::Div,
            Opcode::LoadNumber(2.0),
            Opcode::Mul,
            Opcode::LoadNumber(3.0),
            Opcode::Sub,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(5.0)));
    }

    #[test]
    fn arithmetic_applies_basic_numeric_coercion() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::LoadBool(true),
            Opcode::Add,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(2.0)));
    }

    #[test]
    fn multiplying_object_yields_nan_instead_of_type_error() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::CreateObject,
            Opcode::Mul,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        let value = vm.execute(&chunk).expect("execution should succeed");
        match value {
            JsValue::Number(number) => assert!(number.is_nan()),
            other => panic!("expected numeric result, got {other:?}"),
        }
    }

    #[test]
    fn errors_on_stack_underflow() {
        let chunk = empty_chunk(vec![Opcode::Mul, Opcode::Halt]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Err(VmError::StackUnderflow));
    }

    #[test]
    fn resolves_identifier_from_realm() {
        let chunk = empty_chunk(vec![
            Opcode::LoadIdentifier("x".to_string()),
            Opcode::LoadNumber(2.0),
            Opcode::Mul,
            Opcode::Halt,
        ]);
        let mut realm = Realm::default();
        realm.define_global("x", JsValue::Number(21.0));
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute_in_realm(&chunk, &realm),
            Ok(JsValue::Number(42.0))
        );
    }

    #[test]
    fn unresolved_this_loads_as_global_object() {
        let chunk = empty_chunk(vec![
            Opcode::LoadIdentifier("this".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        let value = vm.execute(&chunk).expect("execution should succeed");
        assert!(matches!(value, JsValue::Object(_)));
    }

    #[test]
    fn global_this_alias_matches_this_value() {
        let chunk = empty_chunk(vec![
            Opcode::LoadIdentifier("globalThis".to_string()),
            Opcode::LoadIdentifier("this".to_string()),
            Opcode::Eq,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn loads_undefined_nan_and_infinity_builtins() {
        let chunk = empty_chunk(vec![
            Opcode::LoadIdentifier("undefined".to_string()),
            Opcode::LoadIdentifier("NaN".to_string()),
            Opcode::Pop,
            Opcode::LoadIdentifier("Infinity".to_string()),
            Opcode::Pop,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Undefined));
    }

    #[test]
    fn executes_let_declaration_and_assignment() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::DefineVariable {
                name: "x".to_string(),
                mutable: true,
            },
            Opcode::LoadNumber(2.0),
            Opcode::StoreVariable("x".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(2.0)));
    }

    #[test]
    fn assignment_to_undeclared_name_creates_global_binding() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::StoreVariable("x".to_string()),
            Opcode::Pop,
            Opcode::LoadIdentifier("x".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(1.0)));
    }

    #[test]
    fn strict_assignment_to_undeclared_name_throws_reference_error() {
        let chunk = empty_chunk(vec![
            Opcode::MarkStrict,
            Opcode::LoadNumber(1.0),
            Opcode::StoreVariable("x".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute(&chunk),
            Err(VmError::UnknownIdentifier("x".to_string()))
        );
    }

    #[test]
    fn global_property_write_updates_var_binding() {
        let script = parse_script(
            "this['__declared__var'] = 'baloon'; var __declared__var; __declared__var;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute(&chunk),
            Ok(JsValue::String("baloon".to_string()))
        );
    }

    #[test]
    fn allows_var_style_redeclaration_and_updates_value() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::DefineVariable {
                name: "x".to_string(),
                mutable: true,
            },
            Opcode::LoadNumber(2.0),
            Opcode::DefineVariable {
                name: "x".to_string(),
                mutable: true,
            },
            Opcode::LoadIdentifier("x".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(2.0)));
    }

    #[test]
    fn var_style_redeclaration_without_initializer_keeps_existing_value() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(7.0),
            Opcode::DefineVariable {
                name: "x".to_string(),
                mutable: true,
            },
            Opcode::LoadUndefined,
            Opcode::DefineVariable {
                name: "x".to_string(),
                mutable: true,
            },
            Opcode::LoadIdentifier("x".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(7.0)));
    }

    #[test]
    fn define_function_overwrites_existing_mutable_binding() {
        let chunk = Chunk {
            code: vec![
                Opcode::LoadNumber(1.0),
                Opcode::DefineVariable {
                    name: "f".to_string(),
                    mutable: true,
                },
                Opcode::DefineFunction {
                    name: "f".to_string(),
                    function_id: 0,
                },
                Opcode::LoadIdentifier("f".to_string()),
                Opcode::Halt,
            ],
            functions: vec![CompiledFunction {
                name: "f".to_string(),
                length: 0,
                params: vec![],
                code: vec![Opcode::LoadUndefined, Opcode::Return],
            }],
        };
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Function(0)));
    }

    #[test]
    fn loads_function_value_and_calls_it() {
        let chunk = Chunk {
            code: vec![Opcode::LoadFunction(0), Opcode::Call(0), Opcode::Halt],
            functions: vec![CompiledFunction {
                name: "<anonymous>".to_string(),
                length: 0,
                params: vec![],
                code: vec![Opcode::LoadNumber(3.0), Opcode::Return],
            }],
        };
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(3.0)));
    }

    #[test]
    fn constructs_function_with_new_opcode() {
        let chunk = Chunk {
            code: vec![
                Opcode::LoadFunction(0),
                Opcode::Construct(0),
                Opcode::GetProperty("answer".to_string()),
                Opcode::Halt,
            ],
            functions: vec![CompiledFunction {
                name: "Ctor".to_string(),
                length: 0,
                params: vec![],
                code: vec![
                    Opcode::LoadIdentifier("this".to_string()),
                    Opcode::LoadNumber(42.0),
                    Opcode::SetProperty("answer".to_string()),
                    Opcode::Pop,
                    Opcode::LoadUndefined,
                    Opcode::Return,
                ],
            }],
        };
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(42.0)));
    }

    #[test]
    fn var_redeclaration_without_initializer_keeps_function_binding() {
        let chunk = Chunk {
            code: vec![
                Opcode::DefineFunction {
                    name: "f".to_string(),
                    function_id: 0,
                },
                Opcode::LoadUndefined,
                Opcode::DefineVariable {
                    name: "f".to_string(),
                    mutable: true,
                },
                Opcode::LoadIdentifier("f".to_string()),
                Opcode::Halt,
            ],
            functions: vec![CompiledFunction {
                name: "f".to_string(),
                length: 0,
                params: vec![],
                code: vec![Opcode::LoadUndefined, Opcode::Return],
            }],
        };
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Function(0)));
    }

    #[test]
    fn errors_when_assigning_to_const() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::DefineVariable {
                name: "x".to_string(),
                mutable: false,
            },
            Opcode::LoadNumber(2.0),
            Opcode::StoreVariable("x".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute(&chunk),
            Err(VmError::ImmutableBinding("x".to_string()))
        );
    }

    #[test]
    fn supports_scope_shadowing() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::DefineVariable {
                name: "x".to_string(),
                mutable: true,
            },
            Opcode::EnterScope,
            Opcode::LoadNumber(2.0),
            Opcode::DefineVariable {
                name: "x".to_string(),
                mutable: true,
            },
            Opcode::LoadIdentifier("x".to_string()),
            Opcode::Pop,
            Opcode::ExitScope,
            Opcode::LoadIdentifier("x".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(1.0)));
    }

    #[test]
    fn assignment_updates_nearest_scope_binding() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::DefineVariable {
                name: "x".to_string(),
                mutable: true,
            },
            Opcode::EnterScope,
            Opcode::LoadNumber(2.0),
            Opcode::StoreVariable("x".to_string()),
            Opcode::Pop,
            Opcode::ExitScope,
            Opcode::LoadIdentifier("x".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(2.0)));
    }

    #[test]
    fn executes_conditional_jumps() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(0.0),
            Opcode::JumpIfFalse(4),
            Opcode::LoadNumber(1.0),
            Opcode::Jump(5),
            Opcode::LoadNumber(2.0),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(2.0)));
    }

    #[test]
    fn executes_loop_with_jumps() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(0.0),
            Opcode::DefineVariable {
                name: "x".to_string(),
                mutable: true,
            },
            Opcode::LoadIdentifier("x".to_string()),
            Opcode::LoadNumber(3.0),
            Opcode::Lt,
            Opcode::JumpIfFalse(12),
            Opcode::LoadIdentifier("x".to_string()),
            Opcode::LoadNumber(1.0),
            Opcode::Add,
            Opcode::StoreVariable("x".to_string()),
            Opcode::Pop,
            Opcode::Jump(2),
            Opcode::LoadIdentifier("x".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(3.0)));
    }

    #[test]
    fn errors_on_invalid_jump_target() {
        let chunk = empty_chunk(vec![Opcode::Jump(99), Opcode::Halt]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Err(VmError::InvalidJump(99)));
    }

    #[test]
    fn throws_uncaught_exception_without_handler() {
        let chunk = empty_chunk(vec![Opcode::LoadNumber(7.0), Opcode::Throw, Opcode::Halt]);
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute(&chunk),
            Err(VmError::UncaughtException(JsValue::Number(7.0)))
        );
    }

    #[test]
    fn catches_thrown_exception() {
        let chunk = empty_chunk(vec![
            Opcode::PushExceptionHandler {
                catch_target: Some(5),
                finally_target: None,
            },
            Opcode::LoadNumber(1.0),
            Opcode::Throw,
            Opcode::PopExceptionHandler,
            Opcode::Jump(8),
            Opcode::LoadException,
            Opcode::DefineVariable {
                name: "e".to_string(),
                mutable: true,
            },
            Opcode::LoadIdentifier("e".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(1.0)));
    }

    #[test]
    fn runs_finally_block_on_exception() {
        let chunk = empty_chunk(vec![
            Opcode::PushExceptionHandler {
                catch_target: None,
                finally_target: Some(5),
            },
            Opcode::LoadNumber(1.0),
            Opcode::Throw,
            Opcode::PopExceptionHandler,
            Opcode::Jump(5),
            Opcode::LoadNumber(2.0),
            Opcode::Pop,
            Opcode::RethrowIfException,
            Opcode::LoadUndefined,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute(&chunk),
            Err(VmError::UncaughtException(JsValue::Number(1.0)))
        );
    }

    #[test]
    fn executes_function_call() {
        let chunk = Chunk {
            code: vec![
                Opcode::DefineFunction {
                    name: "add".to_string(),
                    function_id: 0,
                },
                Opcode::LoadIdentifier("add".to_string()),
                Opcode::LoadNumber(20.0),
                Opcode::LoadNumber(22.0),
                Opcode::Call(2),
                Opcode::Halt,
            ],
            functions: vec![CompiledFunction {
                name: "add".to_string(),
                length: 2,
                params: vec!["a".to_string(), "b".to_string()],
                code: vec![
                    Opcode::LoadIdentifier("a".to_string()),
                    Opcode::LoadIdentifier("b".to_string()),
                    Opcode::Add,
                    Opcode::Return,
                    Opcode::LoadUndefined,
                    Opcode::Return,
                ],
            }],
        };
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(42.0)));
    }

    #[test]
    fn member_call_binds_this_to_receiver() {
        let script = parse_script("let obj = { x: 5, m() { return this.x; } }; obj.m();")
            .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(5.0)));
    }

    #[test]
    fn function_object_exposes_is_prototype_of() {
        let script =
            parse_script("function F() {} F.prototype = F; var o = new F(); F.isPrototypeOf(o);")
                .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn function_value_exposes_is_prototype_of_method() {
        let script = parse_script("typeof (function() {}).isPrototypeOf === 'function';")
            .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn with_identifier_call_uses_with_base_object_as_this() {
        let script = parse_script(
            "var viaCall; var obj = { method: function() { viaCall = this; } }; with (obj) { method(); } viaCall === obj;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn with_statement_abrupt_empty_completion_updates_to_undefined() {
        let first_script = parse_script("5; do { 6; with({}) { break; } 7; } while (false);")
            .expect("script should parse");
        let second_script =
            parse_script("12; do { 13; with({}) { continue; } 14; } while (false);")
                .expect("script should parse");
        let first_chunk = compile_script(&first_script);
        let second_chunk = compile_script(&second_script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&first_chunk), Ok(JsValue::Undefined));
        assert_eq!(vm.execute(&second_chunk), Ok(JsValue::Undefined));
    }

    #[test]
    fn function_call_exposes_arguments_object() {
        let chunk = Chunk {
            code: vec![
                Opcode::DefineFunction {
                    name: "sum".to_string(),
                    function_id: 0,
                },
                Opcode::LoadIdentifier("sum".to_string()),
                Opcode::LoadNumber(20.0),
                Opcode::LoadNumber(22.0),
                Opcode::Call(2),
                Opcode::Halt,
            ],
            functions: vec![CompiledFunction {
                name: "sum".to_string(),
                length: 2,
                params: vec!["a".to_string(), "b".to_string()],
                code: vec![
                    Opcode::LoadIdentifier("arguments".to_string()),
                    Opcode::LoadNumber(0.0),
                    Opcode::GetPropertyByValue,
                    Opcode::LoadIdentifier("arguments".to_string()),
                    Opcode::LoadNumber(1.0),
                    Opcode::GetPropertyByValue,
                    Opcode::Add,
                    Opcode::Return,
                    Opcode::LoadUndefined,
                    Opcode::Return,
                ],
            }],
        };
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(42.0)));
    }

    #[test]
    fn parameter_named_arguments_shadows_arguments_object() {
        let script = parse_script("function f(arguments) { return arguments; } f(42);")
            .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(42.0)));
    }

    #[test]
    fn parameter_named_arguments_defaults_to_undefined() {
        let script = parse_script(
            "var arguments = 'Answer to Life, the Universe, and Everything'; function f(arguments) { return typeof arguments; } f();",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute(&chunk),
            Ok(JsValue::String("undefined".to_string()))
        );
    }

    #[test]
    fn array_sort_honors_compare_function() {
        let script = parse_script(
            "var arr = [4,3,2,1,4,3,2,1,4,3,2,1]; arr.sort(function(x, y) { if (x > y) return -1; if (x < y) return 1; return 0; }); arr.toString();",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute(&chunk),
            Ok(JsValue::String("4,4,4,3,3,3,2,2,2,1,1,1".to_string()))
        );
    }

    #[test]
    fn function_rest_parameter_collects_remaining_args() {
        let script = parse_script(
            "(function(a, ...args) { return args.length === 3 && args[0] === 2 && args[2] === 4; })(1, 2, 3, 4);",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn function_length_ignores_rest_parameter() {
        let script =
            parse_script("(function(a, ...args) {}).length;").expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(1.0)));
    }

    #[test]
    fn arrow_rest_parameter_collects_remaining_args() {
        let script =
            parse_script("((...args) => args.length)(1, 2, 3);").expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(3.0)));
    }

    #[test]
    fn bound_function_rejects_caller_property_access() {
        let script = parse_script(
            "function target() {} \
             var bound = target.bind({}); \
             var readThrows = false; \
             var writeThrows = false; \
             try { bound.caller; } catch (e) { readThrows = true; } \
             try { bound.caller = {}; } catch (e) { writeThrows = true; } \
             readThrows && writeThrows;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn function_call_observes_outer_assignment() {
        let chunk = Chunk {
            code: vec![
                Opcode::LoadNumber(10.0),
                Opcode::DefineVariable {
                    name: "x".to_string(),
                    mutable: true,
                },
                Opcode::DefineFunction {
                    name: "add".to_string(),
                    function_id: 0,
                },
                Opcode::LoadNumber(20.0),
                Opcode::StoreVariable("x".to_string()),
                Opcode::Pop,
                Opcode::LoadIdentifier("add".to_string()),
                Opcode::LoadNumber(1.0),
                Opcode::Call(1),
                Opcode::Halt,
            ],
            functions: vec![CompiledFunction {
                name: "add".to_string(),
                length: 1,
                params: vec!["v".to_string()],
                code: vec![
                    Opcode::LoadIdentifier("x".to_string()),
                    Opcode::LoadIdentifier("v".to_string()),
                    Opcode::Add,
                    Opcode::Return,
                    Opcode::LoadUndefined,
                    Opcode::Return,
                ],
            }],
        };
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(21.0)));
    }

    #[test]
    fn named_function_expression_assignment_is_ignored_in_sloppy_mode() {
        let script =
            parse_script("var f = function g() { g = 1; return typeof g === 'function'; }; f();")
                .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn named_function_expression_assignment_throws_in_strict_mode() {
        let script = parse_script(
            "var f = function g() { 'use strict'; var threw = false; try { g = 1; } catch (e) { threw = true; } return threw && typeof g === 'function'; }; f();",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn named_function_expression_binding_does_not_mutate_outer_name() {
        let script = parse_script(
            "var g = 42; var f = function g() { g = 1; return g === f; }; var innerOk = f(); innerOk && g === 42;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn function_declaration_hoists_over_var_binding() {
        let script = parse_script(
            "function f() { var x; return typeof x; function x() { return 7; } } f();",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute(&chunk),
            Ok(JsValue::String("function".to_string()))
        );
    }

    #[test]
    fn function_declaration_hoists_over_parameter_binding() {
        let script =
            parse_script("function f(x) { return typeof x; function x() { return 7; } } f();")
                .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute(&chunk),
            Ok(JsValue::String("function".to_string()))
        );
    }

    #[test]
    fn for_let_closure_uses_fresh_binding_per_iteration() {
        let script = parse_script(
            "let a = []; for (let i = 0; i < 3; ++i) { a.push(function () { return i; }); } '' + a[0]() + ',' + a[1]() + ',' + a[2]();",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::String("0,1,2".to_string())));
    }

    #[test]
    fn reports_not_callable() {
        let chunk = empty_chunk(vec![Opcode::LoadNumber(1.0), Opcode::Call(0), Opcode::Halt]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Err(VmError::NotCallable));
    }

    #[test]
    fn executes_unary_and_comparison_ops() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(2.0),
            Opcode::Neg,
            Opcode::LoadNumber(-2.0),
            Opcode::Eq,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn unary_plus_rejects_non_canonical_infinity_spelling() {
        let script = parse_script("let x = +\"INFINITY\"; x !== x;").expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn executes_strict_equality_opcodes() {
        let strict_eq = empty_chunk(vec![
            Opcode::LoadString("1".to_string()),
            Opcode::LoadNumber(1.0),
            Opcode::StrictEq,
            Opcode::Halt,
        ]);
        let strict_ne = empty_chunk(vec![
            Opcode::LoadString("1".to_string()),
            Opcode::LoadNumber(1.0),
            Opcode::StrictNe,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&strict_eq), Ok(JsValue::Bool(false)));
        assert_eq!(vm.execute(&strict_ne), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn update_expression_coerces_with_to_number() {
        let script =
            parse_script("let x = \"1\"; let old = x++; old + x;").expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(3.0)));
    }

    #[test]
    fn null_post_increment_returns_numeric_old_value() {
        let script = parse_script("let x = null; let old = x++; old === 0 && x === 1;")
            .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn executes_typeof_opcode_variants() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::Typeof,
            Opcode::TypeofIdentifier("missing".to_string()),
            Opcode::Pop,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute(&chunk),
            Ok(JsValue::String("number".to_string()))
        );
    }

    #[test]
    fn executes_boolean_and_null_literals() {
        let bool_chunk = empty_chunk(vec![Opcode::LoadBool(true), Opcode::Halt]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&bool_chunk), Ok(JsValue::Bool(true)));

        let null_chunk = empty_chunk(vec![Opcode::LoadNull, Opcode::Halt]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&null_chunk), Ok(JsValue::Null));

        let string_chunk = empty_chunk(vec![Opcode::LoadString("ok".to_string()), Opcode::Halt]);
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute(&string_chunk),
            Ok(JsValue::String("ok".to_string()))
        );
    }

    #[test]
    fn executes_logical_not_with_truthiness() {
        let chunk = empty_chunk(vec![Opcode::LoadNumber(0.0), Opcode::Not, Opcode::Halt]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn treats_null_as_falsy() {
        let chunk = empty_chunk(vec![Opcode::LoadNull, Opcode::Not, Opcode::Halt]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn treats_empty_string_as_falsy() {
        let chunk = empty_chunk(vec![
            Opcode::LoadString(String::new()),
            Opcode::Not,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn executes_relational_comparison() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(3.0),
            Opcode::LoadNumber(2.0),
            Opcode::Gt,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn relational_string_compare_uses_utf16_code_unit_order() {
        let script =
            parse_script("(\"\\uD7FF\" < \"\\u{10000}\") && (\"\\u{10000}\" < \"\\uFFFF\");")
                .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn number_min_value_is_smallest_subnormal() {
        let chunk = empty_chunk(vec![
            Opcode::LoadIdentifier("Number".to_string()),
            Opcode::GetProperty("MIN_VALUE".to_string()),
            Opcode::Halt,
        ]);
        let mut realm = Realm::default();
        realm.define_global(
            "Number",
            JsValue::NativeFunction(NativeFunction::NumberConstructor),
        );
        let mut vm = Vm::default();
        let value = vm
            .execute_in_realm(&chunk, &realm)
            .expect("execution should succeed");
        match value {
            JsValue::Number(number) => assert_eq!(number.to_bits(), f64::from_bits(1).to_bits()),
            other => panic!("expected numeric result, got {other:?}"),
        }
    }

    #[test]
    fn is_finite_baseline_coerces_and_checks_finiteness() {
        let mut realm = Realm::default();
        realm.define_global(
            "isFinite",
            JsValue::NativeFunction(NativeFunction::IsFinite),
        );
        let mut vm = Vm::default();

        let numeric_string = empty_chunk(vec![
            Opcode::LoadIdentifier("isFinite".to_string()),
            Opcode::LoadString("42".to_string()),
            Opcode::Call(1),
            Opcode::Halt,
        ]);
        assert_eq!(
            vm.execute_in_realm(&numeric_string, &realm),
            Ok(JsValue::Bool(true))
        );

        let infinity = empty_chunk(vec![
            Opcode::LoadIdentifier("isFinite".to_string()),
            Opcode::LoadNumber(f64::INFINITY),
            Opcode::Call(1),
            Opcode::Halt,
        ]);
        assert_eq!(
            vm.execute_in_realm(&infinity, &realm),
            Ok(JsValue::Bool(false))
        );

        let undefined = empty_chunk(vec![
            Opcode::LoadIdentifier("isFinite".to_string()),
            Opcode::LoadUndefined,
            Opcode::Call(1),
            Opcode::Halt,
        ]);
        assert_eq!(
            vm.execute_in_realm(&undefined, &realm),
            Ok(JsValue::Bool(false))
        );
    }

    #[test]
    fn parse_int_baseline_handles_sign_radix_and_nan() {
        let mut realm = Realm::default();
        realm.define_global(
            "parseInt",
            JsValue::NativeFunction(NativeFunction::ParseInt),
        );
        let mut vm = Vm::default();

        let signed_hex = empty_chunk(vec![
            Opcode::LoadIdentifier("parseInt".to_string()),
            Opcode::LoadString("  -0x10".to_string()),
            Opcode::Call(1),
            Opcode::Halt,
        ]);
        assert_eq!(
            vm.execute_in_realm(&signed_hex, &realm),
            Ok(JsValue::Number(-16.0))
        );

        let explicit_radix = empty_chunk(vec![
            Opcode::LoadIdentifier("parseInt".to_string()),
            Opcode::LoadString("11".to_string()),
            Opcode::LoadNumber(2.0),
            Opcode::Call(2),
            Opcode::Halt,
        ]);
        assert_eq!(
            vm.execute_in_realm(&explicit_radix, &realm),
            Ok(JsValue::Number(3.0))
        );

        let invalid = empty_chunk(vec![
            Opcode::LoadIdentifier("parseInt".to_string()),
            Opcode::LoadString("xyz".to_string()),
            Opcode::LoadNumber(10.0),
            Opcode::Call(2),
            Opcode::Halt,
        ]);
        let value = vm
            .execute_in_realm(&invalid, &realm)
            .expect("execution should succeed");
        match value {
            JsValue::Number(number) => assert!(number.is_nan()),
            other => panic!("expected NaN result, got {other:?}"),
        }
    }

    #[test]
    fn parse_float_baseline_handles_prefix_and_infinity() {
        let mut realm = Realm::default();
        realm.define_global(
            "parseFloat",
            JsValue::NativeFunction(NativeFunction::ParseFloat),
        );
        let mut vm = Vm::default();

        let prefixed = empty_chunk(vec![
            Opcode::LoadIdentifier("parseFloat".to_string()),
            Opcode::LoadString("  -1.25e2xyz".to_string()),
            Opcode::Call(1),
            Opcode::Halt,
        ]);
        assert_eq!(
            vm.execute_in_realm(&prefixed, &realm),
            Ok(JsValue::Number(-125.0))
        );

        let infinity = empty_chunk(vec![
            Opcode::LoadIdentifier("parseFloat".to_string()),
            Opcode::LoadString("Infinity".to_string()),
            Opcode::Call(1),
            Opcode::Halt,
        ]);
        assert_eq!(
            vm.execute_in_realm(&infinity, &realm),
            Ok(JsValue::Number(f64::INFINITY))
        );
    }

    #[test]
    fn primitive_property_accessor_baseline_methods_work() {
        let script = parse_script(
            "true.toString() === 'true' && 1..toString() === '1' && 1.1.toFixed(5) === '1.10000' && 'abc123'.charAt(5) === '3';",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn function_prototype_is_callable_function_value() {
        let script = parse_script(
            "typeof Function.prototype === 'function' && typeof Function.prototype.toString === 'function' && Function.prototype() === undefined;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "Function",
            JsValue::NativeFunction(NativeFunction::FunctionConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn realm_globals_are_visible_on_global_object_properties() {
        let script = parse_script(
            "typeof this.parseInt !== 'undefined' && typeof this.parseFloat !== 'undefined' && typeof this.isNaN !== 'undefined' && typeof this.isFinite !== 'undefined';",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "parseInt",
            JsValue::NativeFunction(NativeFunction::ParseInt),
        );
        realm.define_global(
            "parseFloat",
            JsValue::NativeFunction(NativeFunction::ParseFloat),
        );
        realm.define_global("isNaN", JsValue::NativeFunction(NativeFunction::IsNaN));
        realm.define_global(
            "isFinite",
            JsValue::NativeFunction(NativeFunction::IsFinite),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn primitive_constructor_prototypes_expose_baseline_methods() {
        let script = parse_script(
            "typeof String.prototype.valueOf === 'function' && typeof String.prototype.charAt === 'function' && typeof Number.prototype.valueOf === 'function' && typeof Number.prototype.toFixed === 'function' && typeof Boolean.prototype.valueOf === 'function';",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "String",
            JsValue::NativeFunction(NativeFunction::StringConstructor),
        );
        realm.define_global(
            "Number",
            JsValue::NativeFunction(NativeFunction::NumberConstructor),
        );
        realm.define_global(
            "Boolean",
            JsValue::NativeFunction(NativeFunction::BooleanConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn primitive_constructor_prototypes_expose_trim_and_to_exponential() {
        let script = parse_script(
            "typeof String.prototype.trim === 'function' && typeof Number.prototype.toExponential === 'function';",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "String",
            JsValue::NativeFunction(NativeFunction::StringConstructor),
        );
        realm.define_global(
            "Number",
            JsValue::NativeFunction(NativeFunction::NumberConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn string_length_and_char_code_at_follow_utf16_code_units() {
        let script = parse_script(
            "var chars = '𐒠'; chars.length === 2 && chars.charCodeAt(0) === 0xD801 && chars.charCodeAt(1) === 0xDCA0;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn string_primitive_exposes_string_constructor_property() {
        let script =
            parse_script("'rock\\'n\\'roll'.constructor === String;").expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "String",
            JsValue::NativeFunction(NativeFunction::StringConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn array_length_shrink_deletes_elements_for_subclass_instances() {
        let script = parse_script(
            "class Ar extends Array {} let arr = new Ar('foo', 'bar'); arr.length = 1; arr[0] === 'foo' && arr[1] === undefined && arr.length === 1;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "Object",
            JsValue::NativeFunction(NativeFunction::ObjectConstructor),
        );
        realm.define_global(
            "Array",
            JsValue::NativeFunction(NativeFunction::ArrayConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn number_subclass_inherits_to_exponential() {
        let script = parse_script(
            "class N extends Number {} let n = new N(42); n.toFixed(2) === '42.00' && n.toExponential(2) === '4.20e+1';",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "Object",
            JsValue::NativeFunction(NativeFunction::ObjectConstructor),
        );
        realm.define_global(
            "Number",
            JsValue::NativeFunction(NativeFunction::NumberConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn string_subclass_inherits_trim() {
        let script = parse_script(
            "class S extends String {} let s = new S(' test262 '); s.trim() === 'test262';",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "Object",
            JsValue::NativeFunction(NativeFunction::ObjectConstructor),
        );
        realm.define_global(
            "String",
            JsValue::NativeFunction(NativeFunction::StringConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn object_define_properties_baseline_applies_descriptors() {
        let script = parse_script(
            "var count = 0; Object.defineProperties(this, { x: { value: 1 }, y: { get() { count++; return 1; } } }); (typeof x === 'number') && (typeof y === 'number') && (count === 1);",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "Object",
            JsValue::NativeFunction(NativeFunction::ObjectConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn typeof_reflect_is_object_baseline() {
        let script = parse_script("typeof Reflect === 'object';").expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn object_method_super_property_read_uses_this_prototype() {
        let script = parse_script("let proto = { x: 7 }; let obj = { __proto__: proto, m() { return super.x; } }; obj.m();")
            .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(7.0)));
    }

    #[test]
    fn object_literal_proto_setter_and_shorthand_are_distinct() {
        let script = parse_script(
            "let a = { __proto__: null }; let __proto__ = 2; let b = { __proto__, __proto__ }; a.toString === undefined && b.hasOwnProperty('__proto__') && b.__proto__ === 2;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn object_prevent_extensions_blocks_proto_assignment_mutation() {
        let script = parse_script(
            "var x = Object.preventExtensions({}); var y = {}; \
             try { x.__proto__ = y; } catch (e) {} \
             Object.getPrototypeOf(x) === Object.prototype;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "Object",
            JsValue::NativeFunction(NativeFunction::ObjectConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn delete_number_nan_returns_false_and_keeps_property() {
        let script =
            parse_script("delete Number.NaN === false && typeof Number.NaN !== 'undefined';")
                .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "Number",
            JsValue::NativeFunction(NativeFunction::NumberConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn object_method_length_ignores_default_trailing_comma() {
        let script = parse_script("let obj = { method(a, b = 39,) {} }; obj.method.length;")
            .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(1.0)));
    }

    #[test]
    fn object_method_param_eval_scope_rest_close() {
        let script = parse_script(
            "var callCount = 0; ({ m(...[_ = (callCount = callCount + 1)]) {} }.m()); callCount === 1;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn object_method_param_eval_scope_rest_open() {
        let script = parse_script(
            "var x = 'outside'; var probe1, probe2; ({ m(_ = probe1 = function() { return x; }, ...[__ = (x = 'inside', probe2 = function() { return x; })]) {} }.m()); probe1() === 'inside' && probe2() === 'inside';",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn object_method_super_property_write_targets_this_value() {
        let script = parse_script(
            "let obj = { m() { super.x = 8; return this.hasOwnProperty('x'); } }; obj.m();",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn object_method_super_property_write_respects_freeze_non_strict() {
        let script = parse_script(
            "var xOwn; var yOwn; var obj = { method() { super.x = 8; Object.freeze(obj); super.y = 9; xOwn = obj.hasOwnProperty('x'); yOwn = obj.hasOwnProperty('y'); } }; obj.method(); xOwn && !yOwn;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        let mut realm = Realm::default();
        realm.define_global(
            "Object",
            JsValue::NativeFunction(NativeFunction::ObjectConstructor),
        );
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn class_constructor_super_property_read_baseline() {
        let script = parse_script(
            "var calls = 0; class B {} B.prototype.x = 42; class C extends B { constructor() { super(); calls++; this.v = super.x; } } var c = new C; c.v === 42 && calls === 1;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "Object",
            JsValue::NativeFunction(NativeFunction::ObjectConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn creates_object_and_reads_defined_property() {
        let chunk = empty_chunk(vec![
            Opcode::CreateObject,
            Opcode::LoadNumber(42.0),
            Opcode::DefineProperty("answer".to_string()),
            Opcode::GetProperty("answer".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(42.0)));
    }

    #[test]
    fn sets_object_property_and_returns_assigned_value() {
        let chunk = empty_chunk(vec![
            Opcode::CreateObject,
            Opcode::DefineVariable {
                name: "obj".to_string(),
                mutable: true,
            },
            Opcode::LoadIdentifier("obj".to_string()),
            Opcode::LoadNumber(7.0),
            Opcode::SetProperty("value".to_string()),
            Opcode::Pop,
            Opcode::LoadIdentifier("obj".to_string()),
            Opcode::GetProperty("value".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(7.0)));
    }

    #[test]
    fn supports_computed_property_access_and_assignment() {
        let chunk = empty_chunk(vec![
            Opcode::CreateObject,
            Opcode::DefineVariable {
                name: "obj".to_string(),
                mutable: true,
            },
            Opcode::LoadIdentifier("obj".to_string()),
            Opcode::LoadNumber(3.0),
            Opcode::LoadNumber(9.0),
            Opcode::SetPropertyByValue,
            Opcode::Pop,
            Opcode::LoadIdentifier("obj".to_string()),
            Opcode::LoadNumber(3.0),
            Opcode::GetPropertyByValue,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Number(9.0)));
    }

    #[test]
    fn executes_in_operator_for_object_properties() {
        let chunk = empty_chunk(vec![
            Opcode::CreateObject,
            Opcode::LoadNumber(1.0),
            Opcode::DefineProperty("x".to_string()),
            Opcode::DefineVariable {
                name: "obj".to_string(),
                mutable: true,
            },
            Opcode::LoadString("x".to_string()),
            Opcode::LoadIdentifier("obj".to_string()),
            Opcode::In,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn in_operator_walks_object_prototype_chain() {
        let script = parse_script("\"valueOf\" in ({})").expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn in_operator_throws_for_non_object_rhs() {
        let chunk = empty_chunk(vec![
            Opcode::LoadString("x".to_string()),
            Opcode::LoadNumber(1.0),
            Opcode::In,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute(&chunk),
            Err(VmError::TypeError("right-hand side of 'in' expects object"))
        );
    }

    #[test]
    fn shift_operators_coerce_left_operand_before_right_operand() {
        let script = parse_script(
            "var log1 = '';\
             var leftThrow = { valueOf: function() { log1 += 'L'; throw 'left'; } };\
             var rightSkip = { valueOf: function() { log1 += 'R'; return 1; } };\
             try { leftThrow << rightSkip; } catch (e) {}\
             var first = (log1 === 'L');\
             var log2 = '';\
             var left = { valueOf: function() { log2 += 'L'; return 8; } };\
             var right = { valueOf: function() { log2 += 'R'; throw 'right'; } };\
             try { left << right; } catch (e) {}\
             try { left >> right; } catch (e) {}\
             try { left >>> right; } catch (e) {}\
             first && (log2 === 'LRLRLR');",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn class_extends_observes_host_function_prototype_getter() {
        let script = parse_script(
            "var calls = 0;\
             var Base = function() {}.bind();\
             Object.defineProperty(Base, 'prototype', {\
               get: function() { calls++; return null; },\
               configurable: true\
             });\
             class C extends Base {}\
             Object.defineProperty(Base, 'prototype', {\
               get: function() { calls++; return 42; },\
               configurable: true\
             });\
             var threw = false;\
             try { class D extends Base {} } catch (e) { threw = (e instanceof TypeError); }\
             threw && calls === 2;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "Object",
            JsValue::NativeFunction(NativeFunction::ObjectConstructor),
        );
        realm.define_global(
            "TypeError",
            JsValue::NativeFunction(NativeFunction::TypeErrorConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn derived_constructor_accessing_this_before_super_throws_reference_error() {
        let script = parse_script(
            "class Base {}\
             var threw = false;\
             class Sub extends Base {\
               constructor() {\
                 try { this.x = 1; } catch (e) { threw = e instanceof ReferenceError; }\
                 super();\
               }\
             }\
             new Sub();\
             threw;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "Object",
            JsValue::NativeFunction(NativeFunction::ObjectConstructor),
        );
        realm.define_global(
            "ReferenceError",
            JsValue::NativeFunction(NativeFunction::ReferenceErrorConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn derived_constructor_second_super_throws_after_side_effects() {
        let script = parse_script(
            "var called = 0;\
             class Base { constructor() { called++; } }\
             class Sub extends Base {\
               constructor() {\
                 super();\
                 var threw = false;\
                 try { super(); } catch (e) { threw = e instanceof ReferenceError; }\
                 this.ok = threw;\
               }\
             }\
             var r = new Sub();\
             r.ok && called === 2;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "Object",
            JsValue::NativeFunction(NativeFunction::ObjectConstructor),
        );
        realm.define_global(
            "ReferenceError",
            JsValue::NativeFunction(NativeFunction::ReferenceErrorConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn derived_constructor_without_super_throws_reference_error() {
        let script = parse_script(
            "class Base {}\
             class Bad extends Base { constructor() {} }\
             var threw = false;\
             try { new Bad(); } catch (e) { threw = e instanceof ReferenceError; }\
             threw;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "Object",
            JsValue::NativeFunction(NativeFunction::ObjectConstructor),
        );
        realm.define_global(
            "ReferenceError",
            JsValue::NativeFunction(NativeFunction::ReferenceErrorConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn class_name_binding_is_lexically_scoped_and_immutable() {
        let script = parse_script(
            "var probeBefore = function() { return C; };\
             class C {\
               probe() { return C; }\
               modify() { C = null; }\
             }\
             var cls = probeBefore();\
             C = null;\
             var threw = false;\
             try { cls.prototype.modify(); } catch (e) { threw = e instanceof TypeError; }\
             threw && cls.prototype.probe() === cls;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "Object",
            JsValue::NativeFunction(NativeFunction::ObjectConstructor),
        );
        realm.define_global(
            "TypeError",
            JsValue::NativeFunction(NativeFunction::TypeErrorConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn derived_constructor_super_array_uses_derived_prototype_chain() {
        let script = parse_script(
            "var ArrayCtor = [].constructor;\
             class Sub extends ArrayCtor {}\
             var sub = new Sub();\
             sub instanceof Sub && sub instanceof ArrayCtor;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "Object",
            JsValue::NativeFunction(NativeFunction::ObjectConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn derived_constructor_super_function_returns_function_instance() {
        let script = parse_script(
            "var FunctionCtor = (function() {}).constructor;\
             class Sub extends FunctionCtor {}\
             var sub = new Sub('return 1;');\
             sub instanceof Sub && sub instanceof FunctionCtor && sub() === 1;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "Object",
            JsValue::NativeFunction(NativeFunction::ObjectConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn executes_instanceof_for_matching_constructor() {
        let chunk = empty_chunk(vec![
            Opcode::CreateObject,
            Opcode::DefineVariable {
                name: "obj".to_string(),
                mutable: true,
            },
            Opcode::LoadIdentifier("obj".to_string()),
            Opcode::LoadIdentifier("Ctor".to_string()),
            Opcode::DefineProperty("constructor".to_string()),
            Opcode::Pop,
            Opcode::LoadIdentifier("obj".to_string()),
            Opcode::LoadIdentifier("Ctor".to_string()),
            Opcode::InstanceOf,
            Opcode::Halt,
        ]);
        let mut realm = Realm::default();
        realm.define_global(
            "Ctor",
            JsValue::NativeFunction(NativeFunction::ObjectConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn type_error_instances_are_instanceof_error_and_type_error() {
        let script = parse_script(
            "let e = new TypeError('x'); (e instanceof Error) && (e instanceof TypeError);",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "Error",
            JsValue::NativeFunction(NativeFunction::ErrorConstructor),
        );
        realm.define_global(
            "TypeError",
            JsValue::NativeFunction(NativeFunction::TypeErrorConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn instanceof_uses_function_prototype_getter_for_object_lhs_only() {
        let script = parse_script(
            "var called = 0; Object.defineProperty(Function.prototype, 'prototype', { get: function() { called++; return Array.prototype; } }); var a = [] instanceof Function.prototype; var b = 0 instanceof Function.prototype; a && !b && called === 1;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "Object",
            JsValue::NativeFunction(NativeFunction::ObjectConstructor),
        );
        realm.define_global(
            "Function",
            JsValue::NativeFunction(NativeFunction::FunctionConstructor),
        );
        realm.define_global(
            "Array",
            JsValue::NativeFunction(NativeFunction::ArrayConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn instanceof_accepts_error_like_strings_for_error_constructors() {
        let chunk = empty_chunk(vec![
            Opcode::LoadString("ReferenceError: x is not defined".to_string()),
            Opcode::LoadIdentifier("ErrorCtor".to_string()),
            Opcode::InstanceOf,
            Opcode::Halt,
        ]);
        let mut realm = Realm::default();
        realm.define_global(
            "ErrorCtor",
            JsValue::NativeFunction(NativeFunction::ReferenceErrorConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn instanceof_throws_for_non_callable_rhs() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::LoadNumber(2.0),
            Opcode::InstanceOf,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute(&chunk),
            Err(VmError::TypeError(
                "right-hand side of 'instanceof' is not callable"
            ))
        );
    }

    #[test]
    fn property_access_boxes_numeric_receiver() {
        let chunk = empty_chunk(vec![
            Opcode::LoadNumber(1.0),
            Opcode::GetProperty("x".to_string()),
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Undefined));
    }

    #[test]
    fn generator_next_argument_flows_into_yield_identifier_baseline() {
        let script = parse_script(
            "function * isNameIn() {\
               return '' in (yield);\
             }\
             let iter1 = isNameIn();\
             let first = iter1.next();\
             let second = iter1.next({'': 0});\
             let iter2 = isNameIn();\
             iter2.next();\
             let third = iter2.next({});\
             first.done === false && first.value === undefined &&\
             second.done === true && second.value === true &&\
             third.done === true && third.value === false;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn tagged_template_reuses_template_object_for_same_site() {
        let script = parse_script(
            "var first = null;\
             var second = null;\
             function tag(t) {\
               if (first === null) { first = t; } else { second = t; }\
             }\
             function run() { tag`head${1}tail`; }\
             run();\
             run();\
             first === second;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn tagged_template_object_and_raw_are_frozen_in_non_strict_code() {
        let script = parse_script(
            "var templateObject = null;\
             (function(parameter) { templateObject = parameter; })``;\
             templateObject.test262Prop = true;\
             templateObject.raw.test262Prop = true;\
             templateObject.test262Prop === undefined &&\
             templateObject.raw.test262Prop === undefined;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn eval_statement_list_preserves_prior_value_across_empty_block_completion() {
        let script = parse_script("var result = eval('{length: 3000}{}'); result;")
            .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global("eval", JsValue::NativeFunction(NativeFunction::Eval));
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute_in_realm(&chunk, &realm),
            Ok(JsValue::Number(3000.0))
        );
    }

    #[test]
    fn eval_statement_list_regexp_literal_uses_regexp_prototype() {
        let script = parse_script(
            "var result = eval('{}/1/g;'); \
             Object.getPrototypeOf(result) === RegExp.prototype && \
             result.flags === 'g' && \
             result.toString() === '/1/g';",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global("eval", JsValue::NativeFunction(NativeFunction::Eval));
        realm.define_global(
            "RegExp",
            JsValue::NativeFunction(NativeFunction::RegExpConstructor),
        );
        realm.define_global(
            "Object",
            JsValue::NativeFunction(NativeFunction::ObjectConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn eval_throws_type_error_when_global_var_cannot_be_declared_direct() {
        let script = parse_script(
            "Object.preventExtensions(this); \
             var error; \
             try { eval('var unlikelyVariableName'); } catch (e) { error = e; } \
             error instanceof TypeError;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global("eval", JsValue::NativeFunction(NativeFunction::Eval));
        realm.define_global(
            "Object",
            JsValue::NativeFunction(NativeFunction::ObjectConstructor),
        );
        realm.define_global(
            "TypeError",
            JsValue::NativeFunction(NativeFunction::TypeErrorConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn eval_throws_type_error_when_global_var_cannot_be_declared_indirect() {
        let script = parse_script(
            "Object.preventExtensions(this); \
             assert.throws(TypeError, function() { (0, eval)('var unlikelyVariableName;'); });",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global("eval", JsValue::NativeFunction(NativeFunction::Eval));
        realm.define_global("assert", JsValue::NativeFunction(NativeFunction::Assert));
        realm.define_global(
            "Object",
            JsValue::NativeFunction(NativeFunction::ObjectConstructor),
        );
        realm.define_global(
            "TypeError",
            JsValue::NativeFunction(NativeFunction::TypeErrorConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Undefined));
    }

    #[test]
    fn async_function_call_returns_promise_instance() {
        let script =
            parse_script("async function f() { return 1; } var p = f(); p instanceof Promise;")
                .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "Promise",
            JsValue::NativeFunction(NativeFunction::PromiseConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn async_function_await_error_is_wrapped_into_returned_promise() {
        let script = parse_script(
            "let called = false; async function foo() { called = true; await new Promise(); } var p = foo(); called && (p instanceof Promise);",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global(
            "Promise",
            JsValue::NativeFunction(NativeFunction::PromiseConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(vm.execute_in_realm(&chunk, &realm), Ok(JsValue::Bool(true)));
    }

    #[test]
    fn assert_throws_observes_array_assignment_pattern_throw_path() {
        let script = parse_script(
            "function MyError() {}\
             function thrower() { throw new MyError(); }\
             var returnGetterCalled = 0;\
             var iterator = {\
               [Symbol.iterator]() { return this; },\
               next() { return {done: false}; },\
               get return() { returnGetterCalled += 1; throw 'bad'; }\
             };\
             var passed = false;\
             try {\
               assert.throws(MyError, function() { var a; ([a = thrower()] = iterator); });\
               passed = true;\
             } catch (e) {}\
             passed ? returnGetterCalled : -1;",
        )
        .expect("script should parse");
        let chunk = compile_script(&script);
        let mut realm = Realm::default();
        realm.define_global("assert", JsValue::NativeFunction(NativeFunction::Assert));
        realm.define_global(
            "Symbol",
            JsValue::NativeFunction(NativeFunction::SymbolConstructor),
        );
        realm.define_global(
            "Object",
            JsValue::NativeFunction(NativeFunction::ObjectConstructor),
        );
        let mut vm = Vm::default();
        assert_eq!(
            vm.execute_in_realm(&chunk, &realm),
            Ok(JsValue::Number(1.0))
        );
    }

    #[test]
    fn gc_reclaims_unreachable_objects() {
        let mut vm = Vm::default();
        let realm = Realm::default();
        let object = vm.create_object_value();
        let JsValue::Object(object_id) = object else {
            panic!("expected object value");
        };

        assert!(vm.objects.contains_key(&object_id));
        let stats = vm.collect_garbage(&realm);
        assert_eq!(stats.reclaimed_objects, 1);
        assert!(!vm.objects.contains_key(&object_id));
    }

    #[test]
    fn gc_stats_track_cumulative_reclaimed_objects() {
        let mut vm = Vm::default();
        let realm = Realm::default();
        let _ = vm.create_object_value();
        let first = vm.collect_garbage(&realm);
        assert_eq!(first.reclaimed_objects, 1);

        let _ = vm.create_object_value();
        let second = vm.collect_garbage(&realm);
        assert_eq!(second.reclaimed_objects, 2);
    }

    #[test]
    fn gc_reuses_slot_with_bumped_generation_and_rejects_stale_handle() {
        let mut vm = Vm::default();
        let realm = Realm::default();
        let stale = vm.create_object_value();
        let JsValue::Object(stale_id) = stale else {
            panic!("expected object value");
        };

        let first_stats = vm.collect_garbage(&realm);
        assert_eq!(first_stats.reclaimed_objects, 1);

        let fresh = vm.create_object_value();
        let JsValue::Object(fresh_id) = fresh else {
            panic!("expected object value");
        };

        assert_eq!(Vm::object_id_slot(stale_id), Vm::object_id_slot(fresh_id));
        assert_ne!(stale_id, fresh_id);
        assert!(Vm::object_id_generation(fresh_id) > Vm::object_id_generation(stale_id));

        assert!(matches!(
            vm.get_object_property(stale_id, "x", &realm),
            Err(VmError::UnknownObject(id)) if id == stale_id
        ));

        assert_eq!(
            vm.set_object_property(fresh_id, "x".to_string(), JsValue::Number(7.0), &realm),
            Ok(JsValue::Number(7.0))
        );
        assert_eq!(
            vm.get_object_property(fresh_id, "x", &realm),
            Ok(JsValue::Number(7.0))
        );
    }

    #[test]
    fn gc_keeps_objects_reachable_from_realm_globals() {
        let mut vm = Vm::default();
        let mut realm = Realm::default();
        let object = vm.create_object_value();
        let JsValue::Object(object_id) = object.clone() else {
            panic!("expected object value");
        };
        realm.define_global("root", object);

        let stats = vm.collect_garbage(&realm);
        assert_eq!(stats.reclaimed_objects, 0);
        assert!(vm.objects.contains_key(&object_id));
        assert!(stats.marked_objects >= 1);
        assert_eq!(vm.gc_stats(), stats);
    }

    #[test]
    fn auto_gc_is_disabled_by_default() {
        let chunk = empty_chunk(vec![
            Opcode::CreateObject,
            Opcode::Pop,
            Opcode::LoadUndefined,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Undefined));
        assert_eq!(vm.gc_stats(), GcStats::default());
    }

    #[test]
    fn auto_gc_runs_at_execution_boundary_when_enabled() {
        let chunk = empty_chunk(vec![
            Opcode::CreateObject,
            Opcode::Pop,
            Opcode::LoadUndefined,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        vm.enable_auto_gc(true);
        vm.set_auto_gc_object_threshold(1);
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Undefined));

        let stats = vm.gc_stats();
        assert!(stats.roots > 0);
        assert!(stats.marked_objects > 0);
        assert!(stats.reclaimed_objects >= 1);
        assert!(stats.collections_total >= 1);
        assert_eq!(stats.boundary_collections, stats.collections_total);
        assert_eq!(stats.runtime_collections, 0);
    }

    #[test]
    fn auto_gc_respects_threshold() {
        let chunk = empty_chunk(vec![
            Opcode::CreateObject,
            Opcode::Pop,
            Opcode::LoadUndefined,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        vm.enable_auto_gc(true);
        vm.set_auto_gc_object_threshold(usize::MAX);
        assert_eq!(vm.execute(&chunk), Ok(JsValue::Undefined));
        assert_eq!(vm.gc_stats(), GcStats::default());
    }

    #[test]
    fn runtime_gc_triggers_during_execution_when_enabled() {
        let chunk = empty_chunk(vec![
            Opcode::CreateObject,
            Opcode::Pop,
            Opcode::CreateObject,
            Opcode::Pop,
            Opcode::CreateObject,
            Opcode::Pop,
            Opcode::LoadUndefined,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        vm.enable_auto_gc(true);
        vm.set_auto_gc_object_threshold(1);
        vm.enable_runtime_gc(true);
        vm.set_runtime_gc_check_interval(1);

        assert_eq!(vm.execute(&chunk), Ok(JsValue::Undefined));
        let stats = vm.gc_stats();
        assert!(stats.runtime_collections > 0);
        assert_eq!(
            stats.collections_total,
            stats.runtime_collections + stats.boundary_collections
        );
    }

    #[test]
    fn runtime_gc_keeps_caller_stack_roots_across_nested_calls() {
        let source = r#"
function makeChunk(seed) {
  let arr = [];
  for (let i = 0; i < 6; i = i + 1) {
    arr.push({ value: seed + i });
  }
  return arr;
}
let slots = [makeChunk(1), makeChunk(10), makeChunk(20)];
for (let round = 0; round < 18; round = round + 1) {
  slots[round % 3] = makeChunk(round * 3);
}
slots[1][2].value + slots[2][0].value;
"#;
        let script = parse_script(source).expect("script should parse");
        let chunk = compile_script(&script);
        let mut vm = Vm::default();
        vm.enable_auto_gc(true);
        vm.set_auto_gc_object_threshold(1);
        vm.enable_runtime_gc(true);
        vm.set_runtime_gc_check_interval(1);

        assert!(vm.execute(&chunk).is_ok());
        let stats = vm.gc_stats();
        assert!(stats.runtime_collections > 0);
        assert_eq!(
            stats.collections_total,
            stats.runtime_collections + stats.boundary_collections
        );
    }

    #[test]
    fn runtime_gc_disabled_keeps_runtime_collection_count_zero() {
        let chunk = empty_chunk(vec![
            Opcode::CreateObject,
            Opcode::Pop,
            Opcode::LoadUndefined,
            Opcode::Halt,
        ]);
        let mut vm = Vm::default();
        vm.enable_auto_gc(true);
        vm.set_auto_gc_object_threshold(1);
        vm.enable_runtime_gc(false);

        assert_eq!(vm.execute(&chunk), Ok(JsValue::Undefined));
        let stats = vm.gc_stats();
        assert_eq!(stats.runtime_collections, 0);
        assert!(stats.boundary_collections >= 1);
        assert_eq!(stats.collections_total, stats.boundary_collections);
    }

    #[test]
    fn host_pin_keeps_object_alive_until_unpinned() {
        let mut vm = Vm::default();
        let realm = Realm::default();
        let object = vm.create_object_value();
        let JsValue::Object(object_id) = object.clone() else {
            panic!("expected object value");
        };

        let handle = vm.pin_host_value(object);
        let stats_while_pinned = vm.collect_garbage(&realm);
        assert_eq!(stats_while_pinned.reclaimed_objects, 0);
        assert!(vm.objects.contains_key(&object_id));

        assert!(vm.unpin_host_value(handle).is_some());
        let stats_after_unpin = vm.collect_garbage(&realm);
        assert!(stats_after_unpin.reclaimed_objects >= 1);
        assert!(!vm.objects.contains_key(&object_id));
    }

    #[test]
    fn unpin_unknown_handle_returns_none() {
        let mut vm = Vm::default();
        assert!(vm.unpin_host_value(999).is_none());
    }
}
