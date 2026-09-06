# 2026-09 审查修复记录

本次按审查清单完成 F01–F20、O01–O07、R01–R05 和 I01。未使用 subagents。
原始基线为 `4c004155a5e41eb927b2c7a86ba6a94ff4199d28`；当前修改在工作区，尚未发布。

## 正确性与兼容性

| 清单 | 完成内容 | 主要验收 |
| --- | --- | --- |
| F01 | visitor 返回后检查容器结束；定长剩余项、未消费 map value、不定长 break 和 bytes-as-sequence 都检查 | reader / slice / Value / async typed 边界；提前返回的 visitor |
| F02 | 十六进制浮点按有效位、guard/sticky bits 统一舍入，指数饱和运算避免 panic | 正规/次正规数边界、极大指数、舍入中点、10,000 个固定种子精确值 |
| F03 | flatten 编码保留原始字段 bytes；解码只重映射 key，将原反序列化器交给字段 | RawValue 非 preferred 编码、Simple::UNDEFINED、借用字段指针 |
| F04 | canonical 的 RawValue 转换保留 undefined | f7 不变为 f6；null/undefined 两个键仍可区分 |
| F05 | 编译期拒绝位置数组的条件/单向字段跳过 | 真实 consumer crate 的 array / tuple compile-fail fixture |
| F06 | canonical 键检测递归采用 signed-zero 等价表示，与输出字节顺序分开 | 正负零直接键、array/tag/map 复合键，两种 KeyOrder |
| F07 | float 的文本参数均先 hex 解码，byte string 参数保持原始 bytes | 单引号、raw string、sequence 文本和 bytes 参数 |
| F08 | same 统一入口，保留 simple/NaN 信息；map 按键值集合比较并区分键等价与浮点值等价 | 简写、null/undefined、NaN payload、顺序、正负零 |
| F09 | dt 将完整十进制 epoch 一次转换成 f64，正确处理负秒的小数部分 | `.12` 及原有日期/时区测试 |
| F10–F11 | 位级 NaN 扩展/收窄保持符号、信号位、payload；非默认 NaN 输出 float literal | 所有 binary16 patterns；typed f32、Value 和 CDN 往返 |
| F12 | Value 与 wire 反序列化对齐 byte string、tuple 长度与裸文本枚举行为 | 相同输入跨入口验收 |
| F13–F15 | derive 支持原类型的 container Default、Self 路径；通过 cbor2 re-export 解析 serde | 没有直接 serde 依赖的真实 consumer；泛型手写 Default、default factory、递归 Self |
| F16 | 拆分 cdn-hash / cdn-cri，并明确 std 依赖 | 默认、derive、各扩展独立开关、裸机 cdn-hash + derive |
| F17 | 省略号之间的连续字节 span 拼接完成后再检查 UTF-8 | 分开的 c3 / a4 在省略号后正确构成 ä |
| F18 | CR 在词法分析前忽略，错误位置映回原源码；raw 控制字符和指示符分隔受检 | CR、escape、raw HT/DEL、数组和 map 指示符 |
| F19 | IP 复用 core::net；数字点分 host 不完整匹配 IPv4 时按 reg-name 处理 | 127.000.0.1 被拒绝；123.456 注册名 host 被接受 |
| F20 | Homebrew 公式断言适配 pretty 输出 | 已与实际构建的 CLI 输出核对 |
| I01 | canonicalize 在修改前验证并保存 map 排列计划；出错保持原值 | 重复键和嵌套错误后的编码逐字节不变 |

另对齐了异步校验在深度上限处的空容器行为；超大已知长度的异步 body 在读取任何 body 字节前被拒绝。

## 性能与代码结构

- [x] **O01**：RawValue 直接写入目标 sink，尺寸计算和调用方缓冲区编码不再复制到临时 Vec。
- [x] **O02**：内存输入使用 validate_slice；slice capture 对原始范围一次复制。保留公共 RawValue serde visitor 的再次验证，以便其他反序列化器也必须满足构造不变式；这次验证不会复制大 byte string 本体。
- [x] **O03**：Value 的 struct 收集器改为 map/array 二选一，仅分配实际形状。
- [x] **O04**：二/八/十六进制整数按位打包；普通整数先走 u128；十进制采用 32-bit limbs 和九位分块。增加 source-byte 限制 API，供不可信输入设置预算。
- [x] **O05**：CLI validate 迭代 IgnoredAny，不保存 item payload；show 与 diagnostic decode 共用路径。
- [x] **O06**：reader 复用文本 chunk buffer，slice 按 chunk 借用并追加；hex 输出使用查表。保留非法 UTF-8 body 的原错误位置。
- [x] **O07**：canonical map 使用共享 key arena、无稳定性要求的排序，并直接写出缓存 key bytes。原地 canonicalize 只保存排列计划，不复制整个 payload 来实现回滚。
- [x] **R01**：async 与 core 共用 header 语义解释，CDN 共用 preferred uint 编码，NaN 转换也统一。数组访问器保持小状态，避免 map 专用状态拖慢整数数组。
- [x] **R02**：应用扩展字符串简写进入统一 dispatcher；参数按需要解码，未解析扩展保留原 bytes。
- [x] **R03**：加入运行时回归、真实 consumer compile-pass/compile-fail、分配契约和结构/数值交叉检查；CI 增加 doc-tests、release 回归及裸机 hash 组合。
- [x] **R04**：新增 focused benchmark；结果脚本包含 validate_slice 和 focused 输出；分配次数单独验收。
- [x] **R05**：更新排序示例、simple value、最小依赖、特性边界、位置数组、MSRV、分配承诺等文档。

## 实测

相同 fixture，对比原提交与修复版；本机 aarch64 macOS、Rust 1.97.1、Criterion release + thin LTO。
表中是 Criterion 的时间估计，不能外推成整个库的固定加速倍数。

| 操作 | 修复前 | 修复后 |
| --- | ---: | ---: |
| CDN 十六进制整数，16,000 位 | 80.294 ms | 61.727 µs |
| CDN 十进制整数，16,000 位 | 66.542 ms | 1.913 ms |
| RawValue slice 解码，1 MiB | 45.027 µs | 19.903 µs |
| canonical map，1,024 个整数键 | 66.895 µs | 43.763 µs |
| canonical map，128 个复合键 | 25.856 µs | 17.010 µs |
| 分段文本 slice 解码，1,024 chunks | 47.702 µs | 20.553 µs |
| flatten 解码，4 KiB RawValue 字段 | 731.58 ns | 311.90 ns |
| flatten 编码，同一记录 | 662.49 ns | 861.62 ns |
| 整数数组解码，1,024 项 | 3.890 µs | 3.858 µs |

flatten 编码约多 0.20 µs：它现在缓冲真实 wire bytes 并读取字段范围，以保留原始字段编码，不能再使用会改写数据的 Value 中转。其解码更快。1 MiB RawValue 固定缓冲区写入计时约为 18.44 → 19.49 µs，没有声称此操作加速；其额外分配从一份 payload 降为零。尺寸计算的亚纳秒计时容易被优化器消除中间工作，因此以独立分配契约测试验证，而不报告该项目的加速比例。

分配测试同时验证 debug/release：RawValue sizing 与 to_slice 为零分配；slice capture 只分配一份结果；四字段 array Value 只分配一个容器；CLI 使用的结构校验路径不分配 payload。

## 验证入口

```sh
cargo test --workspace --all-features --all-targets
cargo +1.89.0 test --workspace --all-features --all-targets
cargo test --workspace --all-features --doc
cargo test -p cbor2 --release --all-features --test it
cargo test -p cbor2 --release --all-features --test allocations
cargo test -p cbor2 --no-default-features --lib
cargo test -p cbor2 --no-default-features --features alloc --lib
cargo check -p cbor2 --no-default-features --features cdn-hash,derive --target thumbv7em-none-eabihf
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
git diff --check
cargo bench --manifest-path cbor2-bench/Cargo.toml --bench focused
```

## 兼容性说明

1. `cdn` 启用全部扩展，明确需要 std。无标准库场景选择 `cdn-hash`；基本 CDN 仍随 alloc 提供。
2. 位置数组的条件或单向省略字段现在报编译错误；改用 Option 占位或双向 serde(skip)。
3. flatten 的二进制适配器依赖 cbor2 的内部 raw-item 协议；其他二进制 serializer 不受支持。可读格式使用普通 serde 路径。
4. Value 的普通解码仍将 undefined 映射成 Null；canonical/CDN 等内部保真路径使用保留 simple value 的解码。
5. 没有改变 from_slice 允许后续顶层 item、普通 Vec<u8> 的数组编码，以及 async read 取消后的连接处理规则。
