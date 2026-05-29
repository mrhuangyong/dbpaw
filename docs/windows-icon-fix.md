# Windows 图标分辨率问题修复

## 问题

用户反馈 Windows 下应用图标分辨率太小/模糊。

## 原因

`src-tauri/icons/icon.ico` 只包含 **1 个 256x256 尺寸**：

```
MS Windows icon resource - 1 icon, 256x256
```

Windows 需要多尺寸 ICO 来适配不同场景：

| 场景 | 所需尺寸 |
|------|---------|
| 任务栏/标题栏 | 16x16 |
| 桌面图标/Alt+Tab | 32x32 |
| 资源管理器大图标 | 48x48 |
| 缩略图 | 256x256 |

当前单一 256x256 在小尺寸场景会被严重缩小导致模糊。

## 修复步骤

1. 准备 **1024x1024** 的高质量 PNG 源图标（带透明通道）
2. 运行：
   ```bash
   npx tauri icon your-1024x1024-icon.png
   ```
3. 重新构建：
   ```bash
   bun tauri build
   ```

## 预期结果

生成的 `icon.ico` 将包含 7 个尺寸：
- 16x16, 24x24, 32x32, 48x48, 64x64, 128x128, 256x256

## 验证

```bash
file src-tauri/icons/icon.ico
# 应显示: MS Windows icon resource - 7 icons
```
