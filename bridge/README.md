# springcat-bridge

把工具 hook 变成一条 inbox 文件。桌面应用用 `notify` 监听目录，不轮询、不开 HTTP。

```text
echo {json} | springcat-bridge emit --source gemini-cli --event task.completed
```

写入：

```text
{app_data}/springcat-ai/inbox/{timestamp}-{id}.json
```

Windows 通常是 `%APPDATA%\springcat-ai\inbox\`。

构建：

```text
cargo build --manifest-path bridge/Cargo.toml --release
```

二进制在 `bridge/target/release/springcat-bridge.exe`。
