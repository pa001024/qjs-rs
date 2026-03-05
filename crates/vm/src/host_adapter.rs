use runtime::{JsValue, Realm};
use serde_json::to_string as json_string;
use std::cell::RefCell;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::{
    ScriptAsyncHostCallbackRegistration, ScriptHostCallbackRegistration, ScriptRunOutput,
    ScriptRuntime, ScriptRuntimeError, VmError,
};

static NEXT_HOST_CLASS_ID: AtomicU64 = AtomicU64::new(0);
static NEXT_CONSOLE_BINDING_ID: AtomicU64 = AtomicU64::new(0);

/// Host async 回调 future 类型。
pub type HostAsyncFuture = Pin<Box<dyn Future<Output = Result<JsValue, VmError>> + Send + 'static>>;

/// Host class 构造器回调：返回需要绑定到 JS 实例对象上的 Rust 数据。
pub type HostClassConstructor<T> =
    dyn FnMut(Vec<JsValue>, &Realm, bool) -> Result<T, VmError> + 'static;

/// Host class 实例同步方法回调。
pub type HostClassInstanceMethod<T> = dyn FnMut(&mut crate::Vm, &mut T, Vec<JsValue>, &Realm, bool) -> Result<JsValue, VmError>
    + 'static;

/// Host class 实例异步方法回调。
pub type HostClassAsyncInstanceMethod<T> =
    dyn FnMut(&mut T, Vec<JsValue>) -> Result<HostAsyncFuture, VmError> + 'static;

/// Host class 静态同步方法回调。
pub type HostClassStaticMethod = dyn FnMut(Vec<JsValue>) -> Result<JsValue, VmError> + 'static;

/// Host class 静态异步方法回调。
pub type HostClassAsyncStaticMethod =
    dyn FnMut(Vec<JsValue>) -> Result<HostAsyncFuture, VmError> + 'static;

/// Host class 实例同步方法注册项。
pub struct HostClassMethodRegistration<T> {
    pub name: String,
    pub length: f64,
    pub callback: Box<HostClassInstanceMethod<T>>,
}

impl<T> HostClassMethodRegistration<T> {
    /// 创建实例同步方法注册项。
    pub fn new<F>(name: impl Into<String>, length: f64, callback: F) -> Self
    where
        F: FnMut(&mut crate::Vm, &mut T, Vec<JsValue>, &Realm, bool) -> Result<JsValue, VmError>
            + 'static,
    {
        Self {
            name: name.into(),
            length,
            callback: Box::new(callback),
        }
    }
}

/// Host class 实例异步方法注册项。
pub struct HostClassAsyncMethodRegistration<T> {
    pub name: String,
    pub length: f64,
    pub callback: Box<HostClassAsyncInstanceMethod<T>>,
}

impl<T> HostClassAsyncMethodRegistration<T> {
    /// 创建实例异步方法注册项。
    pub fn new<F, Fut>(name: impl Into<String>, length: f64, mut callback: F) -> Self
    where
        F: FnMut(&mut T, Vec<JsValue>) -> Result<Fut, VmError> + 'static,
        Fut: Future<Output = Result<JsValue, VmError>> + Send + 'static,
    {
        Self {
            name: name.into(),
            length,
            callback: Box::new(move |instance, args| {
                callback(instance, args).map(|future| Box::pin(future) as HostAsyncFuture)
            }),
        }
    }
}

/// Host class 静态同步方法注册项。
pub struct HostClassStaticMethodRegistration {
    pub name: String,
    pub length: f64,
    pub callback: Box<HostClassStaticMethod>,
}

impl HostClassStaticMethodRegistration {
    /// 创建静态同步方法注册项。
    pub fn new<F>(name: impl Into<String>, length: f64, callback: F) -> Self
    where
        F: FnMut(Vec<JsValue>) -> Result<JsValue, VmError> + 'static,
    {
        Self {
            name: name.into(),
            length,
            callback: Box::new(callback),
        }
    }
}

/// Host class 静态异步方法注册项。
pub struct HostClassAsyncStaticMethodRegistration {
    pub name: String,
    pub length: f64,
    pub callback: Box<HostClassAsyncStaticMethod>,
}

impl HostClassAsyncStaticMethodRegistration {
    /// 创建静态异步方法注册项。
    pub fn new<F, Fut>(name: impl Into<String>, length: f64, mut callback: F) -> Self
    where
        F: FnMut(Vec<JsValue>) -> Result<Fut, VmError> + 'static,
        Fut: Future<Output = Result<JsValue, VmError>> + Send + 'static,
    {
        Self {
            name: name.into(),
            length,
            callback: Box::new(move |args| {
                callback(args).map(|future| Box::pin(future) as HostAsyncFuture)
            }),
        }
    }
}

/// Host class 注册定义。
pub struct HostClassRegistration<T> {
    pub class_name: String,
    pub constructor_length: f64,
    pub constructor: Box<HostClassConstructor<T>>,
    pub instance_methods: Vec<HostClassMethodRegistration<T>>,
    pub async_instance_methods: Vec<HostClassAsyncMethodRegistration<T>>,
    pub static_methods: Vec<HostClassStaticMethodRegistration>,
    pub async_static_methods: Vec<HostClassAsyncStaticMethodRegistration>,
}

impl<T> HostClassRegistration<T> {
    /// 创建 host class 注册定义。
    pub fn new<F>(class_name: impl Into<String>, constructor_length: f64, constructor: F) -> Self
    where
        F: FnMut(Vec<JsValue>, &Realm, bool) -> Result<T, VmError> + 'static,
    {
        Self {
            class_name: class_name.into(),
            constructor_length,
            constructor: Box::new(constructor),
            instance_methods: Vec::new(),
            async_instance_methods: Vec::new(),
            static_methods: Vec::new(),
            async_static_methods: Vec::new(),
        }
    }

    /// 添加实例同步方法。
    pub fn with_method(mut self, method: HostClassMethodRegistration<T>) -> Self {
        self.instance_methods.push(method);
        self
    }

    /// 添加实例异步方法。
    pub fn with_async_method(mut self, method: HostClassAsyncMethodRegistration<T>) -> Self {
        self.async_instance_methods.push(method);
        self
    }

    /// 添加静态同步方法。
    pub fn with_static_method(mut self, method: HostClassStaticMethodRegistration) -> Self {
        self.static_methods.push(method);
        self
    }

    /// 添加静态异步方法。
    pub fn with_async_static_method(
        mut self,
        method: HostClassAsyncStaticMethodRegistration,
    ) -> Self {
        self.async_static_methods.push(method);
        self
    }
}

/// Console 消息级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleLevel {
    Log,
    Info,
    Warn,
    Error,
    Debug,
}

/// Console logger 接口。
pub trait ConsoleLogger {
    fn on_console(&mut self, level: ConsoleLevel, args: &[JsValue]);
}

fn sanitize_js_identifier(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            output.push(ch);
        } else {
            output.push('_');
        }
    }
    if output.is_empty() {
        output.push_str("host");
    }
    output
}

fn js_string_literal(value: &str) -> Result<String, ScriptRuntimeError> {
    json_string(value).map_err(|err| ScriptRuntimeError::Io(format!("序列化 JS 字符串失败：{err}")))
}

impl ScriptRuntime {
    /// 注册 Host Class（构造器 + 实例方法 + 静态方法）。
    ///
    /// 说明：
    /// - 构造器强制 `new` 调用（无 `this` 时抛 TypeError）。
    /// - Rust 实例通过 `opaque_data` 绑定到 JS 实例对象。
    /// - 实例方法调用时会安全取回 Rust 实例。
    pub fn register_host_class<T: 'static>(
        &mut self,
        registration: HostClassRegistration<T>,
    ) -> Result<JsValue, ScriptRuntimeError> {
        let class_id = NEXT_HOST_CLASS_ID.fetch_add(1, Ordering::Relaxed);
        let class_name = registration.class_name;
        let constructor_length = registration.constructor_length;
        let mut constructor = registration.constructor;
        let mut instance_methods = registration.instance_methods;
        let mut async_instance_methods = registration.async_instance_methods;
        let mut static_methods = registration.static_methods;
        let mut async_static_methods = registration.async_static_methods;

        let class_ctor =
            self.register_host_callback(ScriptHostCallbackRegistration::constructor(
                class_name.clone(),
                constructor_length,
                move |vm, this_arg, args, realm, strict| {
                    let this_value = this_arg.ok_or(VmError::TypeError(
                        "HostClass:ConstructorMustBeCalledWithNew",
                    ))?;
                    if !matches!(this_value, JsValue::Object(_)) {
                        return Err(VmError::TypeError("HostClass:InvalidConstructorReceiver"));
                    }
                    let instance = constructor(args, realm, strict)?;
                    vm.bind_opaque_data(&this_value, instance)?;
                    Ok(this_value)
                },
            ))?;

        let class_name_sanitized = sanitize_js_identifier(&class_name);
        let mut instance_link_instructions = String::new();
        let mut static_link_instructions = String::new();
        let mut hidden_globals = Vec::new();

        for (index, method) in instance_methods.drain(..).enumerate() {
            let method_name = method.name;
            let method_length = method.length;
            let mut method_callback = method.callback;
            let hidden_name = format!(
                "__qjs_host_class_{}_{}_inst_{}",
                class_name_sanitized,
                sanitize_js_identifier(&method_name),
                class_id.saturating_add(index as u64)
            );
            self.register_host_callback(ScriptHostCallbackRegistration::function(
                hidden_name.clone(),
                method_length,
                move |vm, this_arg, args, realm, strict| {
                    let this_value = this_arg.ok_or(VmError::TypeError("HostClass:MissingThis"))?;
                    let mut instance = vm
                        .take_opaque_data_fast::<T>(&this_value)
                        .ok_or(VmError::TypeError("HostClass:MissingInstance"))?;
                    let result = method_callback(vm, &mut instance, args, realm, strict);
                    let _ = vm.restore_opaque_data_fast(&this_value, instance);
                    result
                },
            ))?;
            let method_name_js = js_string_literal(&method_name)?;
            let hidden_name_js = js_string_literal(&hidden_name)?;
            instance_link_instructions.push_str(&format!(
                "Object.defineProperty(__ctor.prototype, {method_name_js}, {{ value: globalThis[{hidden_name_js}], writable: true, enumerable: false, configurable: true }});"
            ));
            hidden_globals.push(hidden_name);
        }

        for (index, method) in async_instance_methods.drain(..).enumerate() {
            let method_name = method.name;
            let method_length = method.length;
            let mut method_callback = method.callback;
            let hidden_name = format!(
                "__qjs_host_class_{}_{}_ainst_{}",
                class_name_sanitized,
                sanitize_js_identifier(&method_name),
                class_id.saturating_add(index as u64)
            );
            self.register_async_host_callback(ScriptAsyncHostCallbackRegistration::function(
                hidden_name.clone(),
                method_length,
                move |vm, this_arg, args, _realm, _strict| {
                    let this_value = this_arg.ok_or(VmError::TypeError("HostClass:MissingThis"))?;
                    let future = {
                        let instance = vm
                            .opaque_data_mut::<T>(&this_value)
                            .ok_or(VmError::TypeError("HostClass:MissingInstance"))?;
                        method_callback(instance, args)?
                    };
                    Ok(future)
                },
            ))?;
            let method_name_js = js_string_literal(&method_name)?;
            let hidden_name_js = js_string_literal(&hidden_name)?;
            instance_link_instructions.push_str(&format!(
                "Object.defineProperty(__ctor.prototype, {method_name_js}, {{ value: globalThis[{hidden_name_js}], writable: true, enumerable: false, configurable: true }});"
            ));
            hidden_globals.push(hidden_name);
        }

        for (index, method) in static_methods.drain(..).enumerate() {
            let method_name = method.name;
            let method_length = method.length;
            let mut method_callback = method.callback;
            let hidden_name = format!(
                "__qjs_host_class_{}_{}_static_{}",
                class_name_sanitized,
                sanitize_js_identifier(&method_name),
                class_id.saturating_add(index as u64)
            );
            self.register_host_callback(ScriptHostCallbackRegistration::function(
                hidden_name.clone(),
                method_length,
                move |_vm, _this_arg, args, _realm, _strict| method_callback(args),
            ))?;
            let method_name_js = js_string_literal(&method_name)?;
            let hidden_name_js = js_string_literal(&hidden_name)?;
            static_link_instructions.push_str(&format!(
                "Object.defineProperty(__ctor, {method_name_js}, {{ value: globalThis[{hidden_name_js}], writable: true, enumerable: false, configurable: true }});"
            ));
            hidden_globals.push(hidden_name);
        }

        for (index, method) in async_static_methods.drain(..).enumerate() {
            let method_name = method.name;
            let method_length = method.length;
            let mut method_callback = method.callback;
            let hidden_name = format!(
                "__qjs_host_class_{}_{}_astatic_{}",
                class_name_sanitized,
                sanitize_js_identifier(&method_name),
                class_id.saturating_add(index as u64)
            );
            self.register_async_host_callback(ScriptAsyncHostCallbackRegistration::function(
                hidden_name.clone(),
                method_length,
                move |_vm, _this_arg, args, _realm, _strict| method_callback(args),
            ))?;
            let method_name_js = js_string_literal(&method_name)?;
            let hidden_name_js = js_string_literal(&hidden_name)?;
            static_link_instructions.push_str(&format!(
                "Object.defineProperty(__ctor, {method_name_js}, {{ value: globalThis[{hidden_name_js}], writable: true, enumerable: false, configurable: true }});"
            ));
            hidden_globals.push(hidden_name);
        }

        let class_name_js = js_string_literal(&class_name)?;
        let mut cleanup_instructions = String::new();
        for hidden in hidden_globals {
            let hidden_js = js_string_literal(&hidden)?;
            cleanup_instructions.push_str(&format!("delete globalThis[{hidden_js}];"));
        }

        let link_script = format!(
            "(function(){{\
                const __ctor = globalThis[{class_name_js}];\
                {instance_link_instructions}\
                {static_link_instructions}\
                {cleanup_instructions}\
            }})();"
        );
        self.execute_source(&link_script)?;
        Ok(class_ctor)
    }

    /// 注入可替换的 console logger（log/info/warn/error/debug）。
    pub fn inject_console_logger_shared(
        &mut self,
        logger: Rc<RefCell<dyn ConsoleLogger>>,
    ) -> Result<(), ScriptRuntimeError> {
        let binding_id = NEXT_CONSOLE_BINDING_ID.fetch_add(1, Ordering::Relaxed);
        let entries = [
            ("log", ConsoleLevel::Log),
            ("info", ConsoleLevel::Info),
            ("warn", ConsoleLevel::Warn),
            ("error", ConsoleLevel::Error),
            ("debug", ConsoleLevel::Debug),
        ];

        let mut assign_instructions = String::new();
        let mut cleanup_instructions = String::new();
        for (offset, (name, level)) in entries.into_iter().enumerate() {
            let hidden_name = format!("__qjs_console_bridge_{}_{}_{}", name, binding_id, offset);
            let logger_ref = Rc::clone(&logger);
            self.register_host_callback(ScriptHostCallbackRegistration::function(
                hidden_name.clone(),
                0.0,
                move |_vm, _this_arg, args, _realm, _strict| {
                    logger_ref.borrow_mut().on_console(level, &args);
                    Ok(JsValue::Undefined)
                },
            ))?;
            let name_js = js_string_literal(name)?;
            let hidden_name_js = js_string_literal(&hidden_name)?;
            assign_instructions.push_str(&format!(
                "Object.defineProperty(globalThis.console, {name_js}, {{ value: globalThis[{hidden_name_js}], writable: true, enumerable: false, configurable: true }});"
            ));
            cleanup_instructions.push_str(&format!("delete globalThis[{hidden_name_js}];"));
        }

        let script = format!(
            "(function(){{\
                if (typeof globalThis.console !== 'object' || globalThis.console === null) {{ globalThis.console = {{}}; }}\
                {assign_instructions}\
                {cleanup_instructions}\
            }})();"
        );
        self.execute_source(&script)?;
        Ok(())
    }

    /// 注入 console logger（便捷版本）。
    pub fn inject_console_logger<L>(&mut self, logger: L) -> Result<(), ScriptRuntimeError>
    where
        L: ConsoleLogger + 'static,
    {
        self.inject_console_logger_shared(Rc::new(RefCell::new(logger)))
    }
}

/// 宿主适配层，统一封装 ScriptRuntime 的宿主注册与执行入口。
pub struct HostAdapter {
    runtime: ScriptRuntime,
}

impl Default for HostAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl HostAdapter {
    /// 创建适配器实例。
    pub fn new() -> Self {
        Self {
            runtime: ScriptRuntime::new(),
        }
    }

    /// 访问内部 `ScriptRuntime` 只读引用。
    pub fn runtime(&self) -> &ScriptRuntime {
        &self.runtime
    }

    /// 访问内部 `ScriptRuntime` 可变引用。
    pub fn runtime_mut(&mut self) -> &mut ScriptRuntime {
        &mut self.runtime
    }

    /// 注册全局同步函数。
    pub fn register_global_function<F>(
        &mut self,
        name: impl Into<String>,
        length: f64,
        callback: F,
    ) -> Result<JsValue, ScriptRuntimeError>
    where
        F: FnMut(
                &mut crate::Vm,
                Option<JsValue>,
                Vec<JsValue>,
                &Realm,
                bool,
            ) -> Result<JsValue, VmError>
            + 'static,
    {
        self.runtime
            .register_host_callback(ScriptHostCallbackRegistration::function(
                name, length, callback,
            ))
    }

    /// 注册全局异步函数（返回 Promise）。
    pub fn register_global_async_function<F, Fut>(
        &mut self,
        name: impl Into<String>,
        length: f64,
        callback: F,
    ) -> Result<JsValue, ScriptRuntimeError>
    where
        F: FnMut(
                &mut crate::Vm,
                Option<JsValue>,
                Vec<JsValue>,
                &Realm,
                bool,
            ) -> Result<Fut, VmError>
            + 'static,
        Fut: Future<Output = Result<JsValue, VmError>> + Send + 'static,
    {
        self.runtime
            .register_async_host_callback(ScriptAsyncHostCallbackRegistration::function(
                name, length, callback,
            ))
    }

    /// 注册 host class。
    pub fn register_host_class<T: 'static>(
        &mut self,
        registration: HostClassRegistration<T>,
    ) -> Result<JsValue, ScriptRuntimeError> {
        self.runtime.register_host_class(registration)
    }

    /// 运行脚本源码。
    pub fn run_script_source(
        &mut self,
        source: &str,
    ) -> Result<ScriptRunOutput, ScriptRuntimeError> {
        self.runtime.execute_source(source)
    }

    /// 运行脚本源码但不主动 drain Promise jobs。
    pub fn run_script_source_without_drain(
        &mut self,
        source: &str,
    ) -> Result<ScriptRunOutput, ScriptRuntimeError> {
        self.runtime.execute_source_without_drain(source)
    }

    /// 运行脚本文件。
    pub fn run_script_file<P>(
        &mut self,
        script_path: P,
    ) -> Result<ScriptRunOutput, ScriptRuntimeError>
    where
        P: AsRef<Path>,
    {
        self.runtime.execute_file(script_path)
    }

    /// 运行脚本文件但不主动 drain Promise jobs。
    pub fn run_script_file_without_drain<P>(
        &mut self,
        script_path: P,
    ) -> Result<ScriptRunOutput, ScriptRuntimeError>
    where
        P: AsRef<Path>,
    {
        self.runtime.execute_file_without_drain(script_path)
    }

    /// 手动 drain Promise jobs。
    pub fn drain_jobs(&mut self, budget: usize) -> Result<usize, ScriptRuntimeError> {
        self.runtime.drain_jobs(budget)
    }

    /// 设置 stop token（中断控制）。
    pub fn set_stop_token(&mut self, token: Arc<AtomicBool>) {
        self.runtime.set_stop_token(token);
    }

    /// 触发中断请求。
    pub fn interrupt(&self) {
        self.runtime.request_stop();
    }

    /// 清除中断控制。
    pub fn clear_interrupt(&mut self) {
        self.runtime.clear_stop_token();
    }

    /// 注入 console logger。
    pub fn inject_console_logger<L>(&mut self, logger: L) -> Result<(), ScriptRuntimeError>
    where
        L: ConsoleLogger + 'static,
    {
        self.runtime.inject_console_logger(logger)
    }
}
