# mos_消散 / mos_dissolve

EffectCraft 文字与图层粒子消散外部效果助手（`mos.dissolve`，分组 `MOS 视效` / `MOS VFX`，支持自动画布扩展 `padding` 与自定义遮罩图层 `layer_input`）。

## 核心特性

- **0% – 100% 完整消散驱动**：0% 时像素级完整保留原文字，100% 时所有粒子彻底飞散消隐。
- **三种遮罩驱动**：
  - **线性遮罩**：自动贴合文字有效内容包围盒，支持任意角度与边缘噪点扰动。
  - **杂色遮罩**：3 层 fBM 有机云雾噪声场，可控缩放与随机种子。
  - **自定义遮罩**：直接读取合成中任意其他图层（支持亮度 / Alpha 通道提取与反转遮罩）。
- **双模式切换**：
  - **粒子生成**：在文字区域按密度生成粒子，自动采样生成位置及重叠邻域的真实像素颜色；支持柔和圆点、锐利圆点、方形碎屑（自转）与风向拖尾 4 种形状。
  - **像素分离**：直接将文字本身的像素（1px 逐像素或 2–16px 像素块）切开并物理驱动飞散，未脱离区域 100% 锐利保留。
- **风场物理与生命周期渐变**：
  - 风向、风力、重力/浮力、随机爆散、2D 旋度湍流（Curl Noise）与速度随机。
  - 存活时间、存活随机、4 种消散淡出曲线、尾端尺寸缩放、可选尾端变色与发光叠加。
- **中英双语本地化**：自动读取 `%APPDATA%\EffectCraft\mos-suite.json` 语言配置。

## 构建与安装

```powershell
powershell -ExecutionPolicy Bypass -File .\build.ps1
```

构建产物将自动安装到 `%APPDATA%\EffectCraft\helpers\effectcraft-helper-mos-dissolve.exe`，并在 `EffectCraft\Scripts\Startup\` 确保自加载脚本就位。在已打开的 EffectCraft 窗口中，执行 **效果 ▸ 扫描效果辅助程序...** 即可立即加载。

## 自测与样张

```powershell
python .\tools\test_helper.py
```

输出样张位于 `tests\out\`。

## 许可

AGPL-3.0-or-later，见 `LICENSE`。
