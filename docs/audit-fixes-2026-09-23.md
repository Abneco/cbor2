# 2026-09-23 src 审查修复

基线：`278100b`。本次未使用 subagents；以局部修复和减少不必要的分配为主。

## 清单执行结果

| 项目 | 结果 | 验收 |
| --- | --- | --- |
| 1. 内建 Simple 转换 | bool、unit、Option 和 unit enum variant 与 wire 解码一致，保留通用 Simple 表示 | 四个内建值及 simple(59)，含 tag 和不含 tag，逐类型对照 wire 路径 |
| 2. CDN 错误偏移 | 扩展求值错误统一定位到源参数的开头分隔符；词法错误仍保留原位置 | dt/IP/hex/base64/hash/CRI、转义、计算参数、嵌套扩展、中文和 CR |
| 3. Value 数组分配 | SeqAccess 提供准确的剩余长度提示 | 16/128/1024 个 u64 均只分配一次 |
| 4. 忽略 Value 字段 | 直接跳过内存中的子树 | 已知字段夹着未知复合字段的转换测试和独立基准 |
| 5. Pretty map | entry 直接写入最终输出，只延迟逗号和注释 | 嵌套注释的精确输出；128 对整数键值分配从 165 次降至 9 次 |
| 6. Canonical 排序 | 默认 bytewise 且无特殊负零键时省去重复排序 | 两种 KeyOrder、复合键、正负零和错误回滚的现有测试 |
| 7. 数组容量策略 | 完成对比后保留原来的 `4 * length + 9` | 较小预留量增加部分场景的耗时，详见下文 |
| 8. Hex 解码 | 两个 nibble 直接合成一个输出字节，去掉完整 nibble 缓冲 | 跨注释配对、省略号两侧、奇数位和原有 CDN 测试 |
| 9. 代码清理 | 去掉不可能触发的空输出检查和数字词法条件，简化无错误返回的 helper | 保留并解释旧 derive 所需的 `__cbor2_flatten_*` 兼容入口 |

## 基准与取舍

本机 aarch64 macOS、Rust 1.98.1、Criterion release + thin LTO。
数据是相同 fixture 的中位数估计，不代表整个库的固定加速比例。
基线使用 20 个样本、0.2 秒预热、0.5 秒测量；最终版本使用
30 个样本、0.5 秒预热、1 秒测量。几个百分点的差异可能包含测量波动。

| 操作 | 修复前 | 修复后 |
| --- | ---: | ---: |
| Value 数组转换，1024 个 u64 | 2.864 µs | 2.424 µs |
| 提取一个字段并忽略 1024 项 Value 子树 | 1.955 µs | 7.5 ns |
| Pretty indefinite map，128 对整数键值 | 8.600 µs | 5.184 µs |
| Canonical map，1024 个整数键 | 42.244 µs | 37.442 µs |
| Hex 字符串，4096 个输出字节 | 84.533 µs | 81.887 µs |

忽略字段的收益来自消除整棵子树的遍历；该 fixture 只提取一个标量字段。
Hex 优化主要减少临时存储，不声称显著的速度提升。

容量实验覆盖 16、1024、10000 项的 bool、小 u8、宽 u64 和结构体数组：

- `length + 9`：10000 个 bool 的初始预留提示由 40009 降至 10009 字节，
  但 16 项宽 u64 从约 107 ns 升至 189 ns，结构体从约 186 ns 升至 260 ns。
- `min(4 * length, 4096) + 9`：保留小数组预留量，但部分 10000 项
  fixture 约慢 2%–6%，缺少统一的时间收益。
- 最终保留原策略，并留下可重复运行的基准，避免为降低某一种数据形状的
  容量占用而引入其他形状的扩容退化。

## 验证命令

以下检查全部通过：workspace 全特性全目标 349 项测试，默认 + derive
311 项测试及 49 项文档测试，no_std 无堆 11 项、alloc 49 项测试；
Clippy、格式检查、Rust 1.89 全目标兼容性检查及基准 crate 的 Clippy 均通过。

```sh
cargo fmt --all --check
cargo test --workspace --all-targets --all-features
cargo test -p cbor2 --features derive
cargo test --workspace --all-features --doc
cargo test -p cbor2 --no-default-features --lib
cargo test -p cbor2 --no-default-features --features alloc --lib
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +1.89.0 check --workspace --all-targets --all-features
cargo clippy --manifest-path cbor2-bench/Cargo.toml --bench focused -- -D warnings
cargo bench --manifest-path cbor2-bench/Cargo.toml --bench focused -- review
git diff --check
```
