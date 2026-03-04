完成任务前建议：
1) 最小化影响面，优先修改 vm 内核的局部路径并验证编译。
2) 运行 `cargo test -p vm` 覆盖核心语义。
3) 若改动 GC/对象生命周期，补充或更新 vm tests（尤其 module lifecycle / promise / gc 相关）。
4) 再跑 workspace 级检查（fmt/clippy/test）至少到相关 crate。