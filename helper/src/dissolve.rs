// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 MOSSLS

//! mos_dissolve v1.0.0:
//! Text & layer particle dissolve effect for EffectCraft.
//!
//! Features:
//! - 0–100% dissolve progress with feathering & per-particle lifetime
//! - 3 mask types: Linear (with angle & organic edge jitter), Fractal Noise, and Custom Layer (Luma/Alpha + invert)
//! - Samples true source pixel / overlapping neighborhood color
//! - Physics: directional wind, gravity/buoyancy, random scatter, 2D curl-noise turbulence
//! - Lifetime gradients: opacity fade-out curves, size-over-life scaling, optional end-color transition & additive glow
//! - Dual modes:
//!   - Mode 0: Particle Emit (soft circle, hard circle, rotating square flake, velocity streak)
//!   - Mode 1: Direct Pixel Separation (1px–16px blocks split & driven directly from the source pixels)

use std::f32::consts::{PI, TAU};
use std::thread;

// Flat parameter indices (note: `end_color` is a 4-float RGBA color occupying indices 28..=31).
pub const I_MODE: usize = 0;
pub const I_PROGRESS: usize = 1;
pub const I_FEATHER: usize = 2;
pub const I_MASK_TYPE: usize = 3;

pub const I_MASK_INVERT: usize = 4;
pub const I_MASK_ANGLE: usize = 5;
pub const I_MASK_JITTER: usize = 6;
pub const I_NOISE_SCALE: usize = 7;
pub const I_MASK_LAYER: usize = 8;
pub const I_MASK_CHANNEL: usize = 9;

pub const I_PARTICLE_DENSITY: usize = 10;
pub const I_PARTICLE_SIZE: usize = 11;
pub const I_SIZE_RANDOM: usize = 12;
pub const I_PARTICLE_SHAPE: usize = 13;
pub const I_PIXEL_BLOCK: usize = 14;

pub const I_WIND_ANGLE: usize = 15;
pub const I_WIND_STRENGTH: usize = 16;
pub const I_GRAVITY: usize = 17;
pub const I_SCATTER: usize = 18;
pub const I_SPEED_RANDOM: usize = 19;
pub const I_TURBULENCE_AMP: usize = 20;
pub const I_TURBULENCE_SCALE: usize = 21;
pub const I_TIME_EVOLVE: usize = 22;

pub const I_LIFETIME: usize = 23;
pub const I_LIFE_RANDOM: usize = 24;
pub const I_FADE_OUT_CURVE: usize = 25;
pub const I_SIZE_OVER_LIFE: usize = 26;
pub const I_COLOR_FADE_ENABLE: usize = 27;
pub const I_END_COLOR_R: usize = 28;
pub const I_END_COLOR_G: usize = 29;
pub const I_END_COLOR_B: usize = 30;
#[allow(dead_code)]
pub const I_END_COLOR_A: usize = 31;
pub const I_COLOR_FADE_MIX: usize = 32;
pub const I_ADDITIVE_GLOW: usize = 33;

pub const I_SEED: usize = 34;
pub const I_MIX: usize = 35;

pub struct LayerImage {
    pub width: usize,
    pub height: usize,
    #[allow(dead_code)]
    pub origin: [f64; 2],
    #[allow(dead_code)]
    pub scale: f64,
    pub px: Vec<f32>,
}

pub trait Layers {
    fn layer_image(&mut self, param: &str, layer_id: i64, time: f64) -> Option<LayerImage>;
}

pub fn manifest() -> serde_json::Value {
    serde_json::json!({
        "api": 2,
        "id": "mos.dissolve",
        "name": "mos_消散",
        "category": "MOS 视效",
        "version": "1.0.0",
        "author": "MOSSLS",
        "description": "文字与图层粒子消散效果：支持 0–100% 消散进度与三种遮罩（线性、杂色、自定义图层），自动读取文字本像素或重叠像素颜色，提供「粒子生成」与「像素分离」双模式，结合风力、重力、湍流卷曲与存活时间尺寸/透明度/颜色渐变自然消散。",
        "params": [
            {"id": "mode", "name": "消散模式", "type": "popup", "options": ["粒子生成", "像素分离"], "default": 0},
            {"id": "progress", "name": "消散进度 %", "type": "slider", "default": 35, "min": 0, "max": 100, "decimals": 1},
            {"id": "feather", "name": "过渡羽化 %", "type": "slider", "default": 18, "min": 0, "max": 80, "decimals": 1},
            {"id": "mask_type", "name": "遮罩类型", "type": "popup", "options": ["线性遮罩", "杂色遮罩", "自定义遮罩"], "default": 0},

            {"id": "mask_invert", "name": "反转遮罩", "type": "checkbox", "default": false, "group": "遮罩"},
            {"id": "mask_angle", "name": "角度", "type": "angle", "default": 0, "group": "遮罩/线性"},
            {"id": "mask_jitter", "name": "边缘扰动 %", "type": "slider", "default": 15, "min": 0, "max": 100, "decimals": 1, "group": "遮罩/线性"},
            {"id": "noise_scale", "name": "缩放", "type": "slider", "default": 80, "min": 10, "max": 400, "decimals": 1, "group": "遮罩/杂色"},
            {"id": "mask_layer", "name": "遮罩图层", "type": "layer", "group": "遮罩/自定义图层"},
            {"id": "mask_channel", "name": "提取通道", "type": "popup", "options": ["亮度", "Alpha"], "default": 0, "group": "遮罩/自定义图层"},

            {"id": "particle_density", "name": "密度 %", "type": "slider", "default": 55, "min": 5, "max": 100, "decimals": 1, "group": "粒子与像素/粒子生成"},
            {"id": "particle_size", "name": "大小 px", "type": "slider", "default": 3.2, "min": 0.5, "max": 24, "sliderMax": 12, "decimals": 1, "group": "粒子与像素/粒子生成"},
            {"id": "size_random", "name": "大小随机 %", "type": "slider", "default": 50, "min": 0, "max": 100, "decimals": 1, "group": "粒子与像素/粒子生成"},
            {"id": "particle_shape", "name": "形状", "type": "popup", "options": ["柔和圆点", "锐利圆点", "方形碎屑", "风向拖尾"], "default": 0, "group": "粒子与像素/粒子生成"},
            {"id": "pixel_block", "name": "像素块 px", "type": "slider", "default": 1, "min": 1, "max": 16, "decimals": 0, "integer": true, "group": "粒子与像素/像素分离"},

            {"id": "wind_angle", "name": "风向角度", "type": "angle", "default": -15, "group": "风场与物理"},
            {"id": "wind_strength", "name": "风力强度 px", "type": "slider", "default": 160, "min": 0, "max": 800, "sliderMax": 400, "decimals": 1, "group": "风场与物理"},
            {"id": "gravity", "name": "重力 / 浮力 px", "type": "slider", "default": -25, "min": -400, "max": 400, "sliderMin": -200, "sliderMax": 200, "decimals": 1, "group": "风场与物理"},
            {"id": "scatter", "name": "随机爆散 px", "type": "slider", "default": 45, "min": 0, "max": 400, "sliderMax": 200, "decimals": 1, "group": "风场与物理"},
            {"id": "speed_random", "name": "速度随机 %", "type": "slider", "default": 45, "min": 0, "max": 100, "decimals": 1, "group": "风场与物理"},
            {"id": "turbulence_amp", "name": "强度 px", "type": "slider", "default": 55, "min": 0, "max": 300, "sliderMax": 150, "decimals": 1, "group": "风场与物理/湍流卷曲"},
            {"id": "turbulence_scale", "name": "尺度", "type": "slider", "default": 75, "min": 15, "max": 300, "decimals": 1, "group": "风场与物理/湍流卷曲"},
            {"id": "time_evolve", "name": "时间演化", "type": "slider", "default": 20, "min": 0, "max": 200, "decimals": 1, "group": "风场与物理/湍流卷曲"},

            {"id": "lifetime", "name": "存活时间 %", "type": "slider", "default": 45, "min": 5, "max": 150, "decimals": 1, "group": "寿命与渐变"},
            {"id": "life_random", "name": "存活随机 %", "type": "slider", "default": 35, "min": 0, "max": 100, "decimals": 1, "group": "寿命与渐变"},
            {"id": "fade_out_curve", "name": "淡出曲线", "type": "popup", "options": ["平滑淡出", "线性淡出", "快速燃尽", "尾端保留"], "default": 0, "group": "寿命与渐变"},
            {"id": "size_over_life", "name": "尾端尺寸 %", "type": "slider", "default": 25, "min": 0, "max": 250, "decimals": 1, "group": "寿命与渐变"},
            {"id": "color_fade_enable", "name": "启用变色", "type": "checkbox", "default": false, "group": "寿命与渐变/尾端色彩"},
            {"id": "end_color", "name": "尾端颜色", "type": "color", "default": [1.0, 0.55, 0.2, 1.0], "group": "寿命与渐变/尾端色彩"},
            {"id": "color_fade_mix", "name": "变色强度 %", "type": "slider", "default": 75, "min": 0, "max": 100, "decimals": 1, "group": "寿命与渐变/尾端色彩"},
            {"id": "additive_glow", "name": "发光叠加 %", "type": "slider", "default": 0, "min": 0, "max": 100, "decimals": 1, "group": "寿命与渐变/尾端色彩"},

            {"id": "seed", "name": "随机种子", "type": "slider", "default": 1, "min": 0, "max": 1000, "sliderMax": 100, "decimals": 0, "integer": true, "group": "输出"},
            {"id": "mix", "name": "混合 %", "type": "slider", "default": 100, "min": 0, "max": 100, "decimals": 0, "integer": true, "group": "输出"}
        ]
    })
}

#[derive(Clone, Copy)]
struct Params {
    mode: i32,
    progress: f32,
    feather: f32,
    lifetime: f32,
    life_random: f32,

    mask_type: i32,
    mask_angle: f32,
    mask_jitter: f32,
    noise_scale: f32,
    mask_layer: i64,
    mask_channel: i32,
    mask_invert: bool,

    particle_density: f32,
    particle_size: f32,
    size_random: f32,
    particle_shape: i32,
    pixel_block: i32,

    wind_angle: f32,
    wind_strength: f32,
    gravity: f32,
    scatter: f32,
    turbulence_amp: f32,
    turbulence_scale: f32,
    speed_random: f32,

    fade_out_curve: i32,
    size_over_life: f32,
    color_fade_enable: bool,
    end_color: [f32; 3],
    color_fade_mix: f32,
    additive_glow: f32,

    seed: u32,
    time_evolve: f32,
    mix: f32,
}

impl Params {
    fn parse(flat: &[f64]) -> Self {
        let f = |i: usize, d: f64| -> f32 {
            let v = flat.get(i).copied().unwrap_or(d);
            if v.is_finite() { v as f32 } else { d as f32 }
        };
        let layer_v = flat.get(I_MASK_LAYER).copied().unwrap_or(-1.0);
        let mask_layer = if layer_v.is_finite() {
            layer_v as i64
        } else {
            -1
        };

        Self {
            mode: (f(I_MODE, 0.0).round() as i32).clamp(0, 1),
            progress: (f(I_PROGRESS, 35.0) / 100.0).clamp(0.0, 1.0),
            feather: (f(I_FEATHER, 18.0) / 100.0).clamp(0.0, 0.8),
            lifetime: (f(I_LIFETIME, 45.0) / 100.0).clamp(0.05, 1.5),
            life_random: (f(I_LIFE_RANDOM, 35.0) / 100.0).clamp(0.0, 1.0),

            mask_type: (f(I_MASK_TYPE, 0.0).round() as i32).clamp(0, 2),
            mask_angle: f(I_MASK_ANGLE, 0.0),
            mask_jitter: (f(I_MASK_JITTER, 15.0) / 100.0).clamp(0.0, 1.0),
            noise_scale: f(I_NOISE_SCALE, 80.0).clamp(10.0, 400.0),
            mask_layer,
            mask_channel: (f(I_MASK_CHANNEL, 0.0).round() as i32).clamp(0, 1),
            mask_invert: f(I_MASK_INVERT, 0.0) != 0.0,

            particle_density: (f(I_PARTICLE_DENSITY, 55.0) / 100.0).clamp(0.05, 1.0),
            particle_size: f(I_PARTICLE_SIZE, 3.2).clamp(0.5, 24.0),
            size_random: (f(I_SIZE_RANDOM, 50.0) / 100.0).clamp(0.0, 1.0),
            particle_shape: (f(I_PARTICLE_SHAPE, 0.0).round() as i32).clamp(0, 3),
            pixel_block: (f(I_PIXEL_BLOCK, 1.0).round() as i32).clamp(1, 16),

            wind_angle: f(I_WIND_ANGLE, -15.0),
            wind_strength: f(I_WIND_STRENGTH, 160.0).clamp(0.0, 800.0),
            gravity: f(I_GRAVITY, -25.0).clamp(-400.0, 400.0),
            scatter: f(I_SCATTER, 45.0).clamp(0.0, 400.0),
            turbulence_amp: f(I_TURBULENCE_AMP, 55.0).clamp(0.0, 300.0),
            turbulence_scale: f(I_TURBULENCE_SCALE, 75.0).clamp(15.0, 300.0),
            speed_random: (f(I_SPEED_RANDOM, 45.0) / 100.0).clamp(0.0, 1.0),

            fade_out_curve: (f(I_FADE_OUT_CURVE, 0.0).round() as i32).clamp(0, 3),
            size_over_life: (f(I_SIZE_OVER_LIFE, 25.0) / 100.0).clamp(0.0, 2.5),
            color_fade_enable: f(I_COLOR_FADE_ENABLE, 0.0) != 0.0,
            end_color: [
                f(I_END_COLOR_R, 1.0).clamp(0.0, 4.0),
                f(I_END_COLOR_G, 0.55).clamp(0.0, 4.0),
                f(I_END_COLOR_B, 0.2).clamp(0.0, 4.0),
            ],
            color_fade_mix: (f(I_COLOR_FADE_MIX, 75.0) / 100.0).clamp(0.0, 1.0),
            additive_glow: (f(I_ADDITIVE_GLOW, 0.0) / 100.0).clamp(0.0, 1.0),

            seed: f(I_SEED, 1.0).clamp(0.0, 10000.0).round() as u32,
            time_evolve: f(I_TIME_EVOLVE, 20.0).clamp(0.0, 200.0),
            mix: (f(I_MIX, 100.0) / 100.0).clamp(0.0, 1.0),
        }
    }
}

/// Compute stable canvas padding in buffer pixels so wind-blown particles aren't clipped at layer bounds.
pub fn padding(flat: &[f64], scale: f64) -> u32 {
    let p = Params::parse(flat);
    if p.mix <= 1e-4 {
        return 0;
    }
    let sc = if scale.is_finite() && scale > 0.0 {
        scale as f32
    } else {
        1.0
    };
    let speed_factor = 1.0 + 0.5 * p.speed_random;
    let max_reach = (p.wind_strength * speed_factor
        + p.gravity.abs() * 0.55 * speed_factor
        + p.scatter * speed_factor
        + p.turbulence_amp * 1.15
        + p.particle_size * (1.0 + p.size_over_life).max(1.0) * 1.5)
        * sc;
    // Cap at 512 * scale to prevent excessive memory allocation on extreme slider values.
    let cap = (512.0 * sc).max(64.0);
    max_reach.clamp(0.0, cap).ceil() as u32
}

// ---------------------------------------------------------------------------
// Deterministic hashes & 2D smooth value / fBM / curl noise
// ---------------------------------------------------------------------------

#[inline]
fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

#[inline]
fn hash3(x: i32, y: i32, seed: u32) -> f32 {
    let h = hash_u32(
        (x as u32)
            .wrapping_mul(0x9e37_79b1)
            .wrapping_add((y as u32).wrapping_mul(0x85eb_ca77))
            .wrapping_add(seed.wrapping_mul(0xc2b2_ae3d)),
    );
    (h & 0x00ff_ffff) as f32 * (1.0 / 16_777_216.0)
}

#[inline]
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let d = edge1 - edge0;
    if d.abs() < 1e-6 {
        return if x >= edge1 { 1.0 } else { 0.0 };
    }
    let t = ((x - edge0) / d).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Smooth 2D value noise returning `(value, d/dx, d/dy)` in `[0, 1]` for value.
#[inline]
fn value_noise_grad(x: f32, y: f32, seed: u32) -> (f32, f32, f32) {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - ix as f32;
    let fy = y - iy as f32;

    // Quintic C2 interpolation for smooth derivatives (no grid crease in curl noise).
    let ux = fx * fx * fx * (fx * (fx * 6.0 - 15.0) + 10.0);
    let uy = fy * fy * fy * (fy * (fy * 6.0 - 15.0) + 10.0);
    let dux = 30.0 * fx * fx * (fx - 1.0) * (fx - 1.0);
    let duy = 30.0 * fy * fy * (fy - 1.0) * (fy - 1.0);

    let a = hash3(ix, iy, seed);
    let b = hash3(ix + 1, iy, seed);
    let c = hash3(ix, iy + 1, seed);
    let d = hash3(ix + 1, iy + 1, seed);

    let ab = a + (b - a) * ux;
    let cd = c + (d - c) * ux;
    let val = ab + (cd - ab) * uy;
    let dvdx = ((b - a) + (a - b - c + d) * uy) * dux;
    let dvdy = ((c - a) + (a - b - c + d) * ux) * duy;
    (val, dvdx, dvdy)
}

#[inline]
fn fbm3(x: f32, y: f32, seed: u32) -> f32 {
    let (n0, _, _) = value_noise_grad(x, y, seed);
    let (n1, _, _) = value_noise_grad(x * 2.03 + 17.3, y * 2.03 - 11.7, seed.wrapping_add(101));
    let (n2, _, _) = value_noise_grad(x * 4.07 - 31.1, y * 4.07 + 23.9, seed.wrapping_add(211));
    n0 * 0.56 + n1 * 0.29 + n2 * 0.15
}

/// Divergence-free 2D curl velocity field built from 2-octave smooth noise.
#[inline]
fn curl_vel(x: f32, y: f32, inv_scale: f32, drift: f32, seed: u32) -> (f32, f32) {
    let sx = x * inv_scale + drift;
    let sy = y * inv_scale - drift * 0.63;
    let (_, gx0, gy0) = value_noise_grad(sx, sy, seed.wrapping_add(709));
    let (_, gx1, gy1) = value_noise_grad(
        sx * 2.1 + 13.7,
        sy * 2.1 + 29.3,
        seed.wrapping_add(1427),
    );
    let vx = gy0 * 0.72 + gy1 * 0.28;
    let vy = -(gx0 * 0.72 + gx1 * 0.28);
    (vx, vy)
}

// ---------------------------------------------------------------------------
// Mask field evaluation
// ---------------------------------------------------------------------------

struct MaskCtx<'a> {
    mode: i32, // 0 linear, 1 noise, 2 custom
    dx: f32,
    dy: f32,
    proj_min: f32,
    proj_inv: f32,
    jitter: f32,
    inv_noise_scale: f32,
    seed: u32,
    invert: bool,
    channel: i32,
    custom: Option<&'a LayerImage>,
    frame_w: usize,
    frame_h: usize,
    frame_origin: [f64; 2],
    frame_scale: f64,
    layer_size: [f64; 2],
}

impl<'a> MaskCtx<'a> {
    #[inline]
    fn sample_custom(&self, x: f32, y: f32) -> f32 {
        let Some(img) = self.custom else {
            return 0.0;
        };
        if img.width == 0 || img.height == 0 {
            return 0.0;
        }
        // Align coordinates from current frame buffer space into normalized layer UV,
        // then sample the custom mask layer bilinearly.
        let (u, v) = if self.layer_size[0] > 1.0 && self.layer_size[1] > 1.0 && self.frame_scale > 1e-4 {
            let lx = self.frame_origin[0] + (x as f64) / self.frame_scale;
            let ly = self.frame_origin[1] + (y as f64) / self.frame_scale;
            (
                (lx / self.layer_size[0]) as f32,
                (ly / self.layer_size[1]) as f32,
            )
        } else {
            (
                x / (self.frame_w.max(1) as f32),
                y / (self.frame_h.max(1) as f32),
            )
        };

        let mx = (u.clamp(0.0, 1.0) * (img.width.saturating_sub(1) as f32))
            .clamp(0.0, (img.width - 1) as f32);
        let my = (v.clamp(0.0, 1.0) * (img.height.saturating_sub(1) as f32))
            .clamp(0.0, (img.height - 1) as f32);

        let x0 = mx.floor() as usize;
        let y0 = my.floor() as usize;
        let x1 = (x0 + 1).min(img.width - 1);
        let y1 = (y0 + 1).min(img.height - 1);
        let fx = mx - x0 as f32;
        let fy = my - y0 as f32;

        let read_val = |ix: usize, iy: usize| -> f32 {
            let o = (iy * img.width + ix) * 4;
            let r = img.px[o];
            let g = img.px[o + 1];
            let b = img.px[o + 2];
            let a = img.px[o + 3].clamp(0.0, 1.0);
            if self.channel == 1 {
                a
            } else {
                // Premultiplied luminance naturally fades to 0 where transparent.
                (0.2126 * r + 0.7152 * g + 0.0722 * b).clamp(0.0, 1.0)
            }
        };

        let v00 = read_val(x0, y0);
        let v10 = read_val(x1, y0);
        let v01 = read_val(x0, y1);
        let v11 = read_val(x1, y1);
        let top = v00 + (v10 - v00) * fx;
        let bot = v01 + (v11 - v01) * fx;
        (top + (bot - top) * fy).clamp(0.0, 1.0)
    }

    #[inline]
    fn eval(&self, x: f32, y: f32) -> f32 {
        let mut m = match self.mode {
            1 => {
                // Fractal noise mask stretched to cleanly cover [0, 1].
                let n = fbm3(x * self.inv_noise_scale, y * self.inv_noise_scale, self.seed.wrapping_add(37));
                ((n - 0.18) / 0.64).clamp(0.0, 1.0)
            }
            2 if self.custom.is_some() => self.sample_custom(x, y),
            _ => {
                // Linear mask along angle across content bounds, with optional organic edge noise jitter.
                let proj = x * self.dx + y * self.dy;
                let base = ((proj - self.proj_min) * self.proj_inv).clamp(0.0, 1.0);
                if self.jitter > 1e-4 {
                    let n = fbm3(
                        x * self.inv_noise_scale * 1.4,
                        y * self.inv_noise_scale * 1.4,
                        self.seed.wrapping_add(91),
                    );
                    let delta = (n - 0.5) * self.jitter * 0.65;
                    ((base + delta + self.jitter * 0.25) / (1.0 + self.jitter * 0.5)).clamp(0.0, 1.0)
                } else {
                    base
                }
            }
        };
        if self.invert {
            m = 1.0 - m;
        }
        m
    }
}

// ---------------------------------------------------------------------------
// Source pixel sampling (straight RGB + alpha from premultiplied buffer)
// ---------------------------------------------------------------------------

#[inline]
fn sample_source_bilinear(src: &[f32], w: usize, h: usize, x: f32, y: f32) -> [f32; 4] {
    let cx = x.clamp(0.0, (w - 1) as f32);
    let cy = y.clamp(0.0, (h - 1) as f32);
    let x0 = cx.floor() as usize;
    let y0 = cy.floor() as usize;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let fx = cx - x0 as f32;
    let fy = cy - y0 as f32;

    let i00 = (y0 * w + x0) * 4;
    let i10 = (y0 * w + x1) * 4;
    let i01 = (y1 * w + x0) * 4;
    let i11 = (y1 * w + x1) * 4;

    let mut out = [0.0f32; 4];
    for c in 0..4 {
        let top = src[i00 + c] + (src[i10 + c] - src[i00 + c]) * fx;
        let bot = src[i01 + c] + (src[i11 + c] - src[i01 + c]) * fx;
        out[c] = top + (bot - top) * fy;
    }
    out
}

/// Sample overlapping neighborhood color around `(x, y)` so particles inherit clean un-premultiplied
/// text/pixel colors even near anti-aliased glyph edges.
#[inline]
fn sample_particle_color(src: &[f32], w: usize, h: usize, x: f32, y: f32, radius: f32) -> ([f32; 3], f32) {
    let c0 = sample_source_bilinear(src, w, h, x, y);
    let mut pr = c0[0] * 2.0;
    let mut pg = c0[1] * 2.0;
    let mut pb = c0[2] * 2.0;
    let mut pa = c0[3] * 2.0;
    let mut wsum = 2.0f32;

    if radius >= 1.2 {
        let r = (radius * 0.55).min(3.0);
        for &(ox, oy) in &[(-r, 0.0), (r, 0.0), (0.0, -r), (0.0, r)] {
            let s = sample_source_bilinear(src, w, h, x + ox, y + oy);
            pr += s[0];
            pg += s[1];
            pb += s[2];
            pa += s[3];
            wsum += 1.0;
        }
    }

    let avg_a = (pa / wsum).clamp(0.0, 1.0);
    if pa > 1e-4 {
        ([pr / pa, pg / pa, pb / pa], avg_a)
    } else {
        ([0.0, 0.0, 0.0], 0.0)
    }
}

// ---------------------------------------------------------------------------
// Active particle / separated pixel block
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Sprite {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    radius: f32,
    rot: f32,
    // Premultiplied RGBA ready for compositing
    r: f32,
    g: f32,
    b: f32,
    a: f32,
    // 0: soft circle, 1: hard circle, 2: square flake, 3: velocity streak, 4: pixel splat (1px bilinear), 5: pixel block
    kind: u8,
    min_y: i32,
    max_y: i32,
}

#[inline]
fn eval_fade_curve(u: f32, curve: i32) -> f32 {
    // Brief smooth birth fade-in (0 -> 0.07) prevents harsh popping on the dissolve front.
    let fade_in = smoothstep(0.0, 0.07, u);
    let fade_out = match curve {
        1 => (1.0 - u).clamp(0.0, 1.0), // Linear
        2 => (1.0 - u).clamp(0.0, 1.0).powf(2.2), // Fast burn
        3 => 1.0 - smoothstep(0.55, 1.0, u), // Late fade
        _ => 1.0 - smoothstep(0.18, 1.0, u), // Smooth fade
    };
    (fade_in * fade_out).clamp(0.0, 1.0)
}

#[inline]
fn compute_physics(
    x0: f32,
    y0: f32,
    u: f32,
    r_dir: f32,
    r_spd: f32,
    r_scat: f32,
    p: &Params,
    sc: f32,
    wind_x: f32,
    wind_y: f32,
    drift: f32,
) -> (f32, f32, f32, f32) {
    let speed_mul = (1.0 + (r_spd * 2.0 - 1.0) * p.speed_random * 0.65).max(0.15);
    let u2 = u * u;

    // Smoothly accelerating wind displacement (slower at detachment, faster as wind catches it).
    let wind_prog = (0.32 * u + 0.68 * u2) * speed_mul;
    let mut dx = wind_x * wind_prog;
    let mut dy = wind_y * wind_prog;

    // Gravity / buoyancy.
    dy += p.gravity * sc * 0.55 * u2 * speed_mul;

    // Random radial scatter burst.
    let ang = r_dir * TAU;
    let scat_mag = p.scatter * sc * (0.25 + 0.75 * r_scat) * (0.65 * u + 0.35 * u2) * speed_mul;
    dx += ang.cos() * scat_mag;
    dy += ang.sin() * scat_mag;

    // Base instantaneous velocity (for velocity streak orientation).
    let mut vx = wind_x * (0.32 + 1.36 * u) * speed_mul + ang.cos() * scat_mag;
    let mut vy = wind_y * (0.32 + 1.36 * u) * speed_mul
        + p.gravity * sc * 1.1 * u * speed_mul
        + ang.sin() * scat_mag;

    // 3-step curl turbulence integration along the particle trajectory.
    if p.turbulence_amp > 0.1 {
        let inv_ts = 1.0 / (p.turbulence_scale * sc).max(2.0);
        let step_amp = p.turbulence_amp * sc * (u.powf(1.15)) * speed_mul * (1.0 / 3.0);
        let mut cx = x0;
        let mut cy = y0;
        let mut last_tx = 0.0f32;
        let mut last_ty = 0.0f32;
        for step in 0..3 {
            let frac = (step as f32 + 0.5) / 3.0;
            let sample_x = cx + dx * frac;
            let sample_y = cy + dy * frac;
            let (tx, ty) = curl_vel(sample_x, sample_y, inv_ts, drift, p.seed);
            cx += tx * step_amp * 2.2;
            cy += ty * step_amp * 2.2;
            last_tx = tx * step_amp * 3.0;
            last_ty = ty * step_amp * 3.0;
        }
        dx += cx - x0;
        dy += cy - y0;
        vx += last_tx;
        vy += last_ty;
    }

    (x0 + dx, y0 + dy, vx, vy)
}

#[inline]
fn blend_pixel(dst: &mut [f32], idx: usize, sr: f32, sg: f32, sb: f32, sa: f32, glow: f32) {
    if sa <= 1e-5 {
        return;
    }
    let inv_rgb = 1.0 - sa * (1.0 - glow);
    let inv_a = 1.0 - sa;
    dst[idx] = sr + dst[idx] * inv_rgb;
    dst[idx + 1] = sg + dst[idx + 1] * inv_rgb;
    dst[idx + 2] = sb + dst[idx + 2] * inv_rgb;
    dst[idx + 3] = (sa + dst[idx + 3] * inv_a).clamp(0.0, 1.0);
}

fn rasterize_sprite_band(
    sp: &Sprite,
    dst: &mut [f32],
    w: usize,
    band_y0: usize,
    band_y1: usize,
    glow: f32,
) {
    let wi = w as i32;
    let by0 = band_y0 as i32;
    let by1 = band_y1 as i32;

    match sp.kind {
        4 => {
            // 1px bilinear sub-pixel splat for razor-sharp pixel separation.
            let x0 = sp.x.floor() as i32;
            let y0 = sp.y.floor() as i32;
            let fx = sp.x - x0 as f32;
            let fy = sp.y - y0 as f32;
            let weights = [
                (x0, y0, (1.0 - fx) * (1.0 - fy)),
                (x0 + 1, y0, fx * (1.0 - fy)),
                (x0, y0 + 1, (1.0 - fx) * fy),
                (x0 + 1, y0 + 1, fx * fy),
            ];
            for &(px, py, wt) in &weights {
                if px >= 0 && px < wi && py >= by0 && py < by1 && wt > 1e-4 {
                    let idx = ((py - by0) as usize * w + px as usize) * 4;
                    blend_pixel(
                        dst,
                        idx,
                        sp.r * wt,
                        sp.g * wt,
                        sp.b * wt,
                        sp.a * wt,
                        glow,
                    );
                }
            }
        }
        5 => {
            // Pixel block (axis-aligned anti-aliased square for pixel separation mode when block > 1).
            let half = sp.radius.max(0.5);
            let x0 = (sp.x - half - 0.5).floor() as i32;
            let x1 = (sp.x + half + 0.5).ceil() as i32;
            let y0 = ((sp.y - half - 0.5).floor() as i32).max(by0);
            let y1 = ((sp.y + half + 0.5).ceil() as i32).min(by1 - 1);
            let x0c = x0.max(0);
            let x1c = x1.min(wi - 1);
            if x0c > x1c || y0 > y1 {
                return;
            }
            for py in y0..=y1 {
                let dy = (py as f32 + 0.5 - sp.y).abs();
                let cov_y = (half + 0.5 - dy).clamp(0.0, 1.0);
                if cov_y <= 1e-4 {
                    continue;
                }
                let row = (py - by0) as usize * w;
                for px in x0c..=x1c {
                    let dx = (px as f32 + 0.5 - sp.x).abs();
                    let cov_x = (half + 0.5 - dx).clamp(0.0, 1.0);
                    let cov = cov_x * cov_y;
                    if cov > 1e-4 {
                        let idx = (row + px as usize) * 4;
                        blend_pixel(
                            dst,
                            idx,
                            sp.r * cov,
                            sp.g * cov,
                            sp.b * cov,
                            sp.a * cov,
                            glow,
                        );
                    }
                }
            }
        }
        2 => {
            // Rotating square flake.
            let half = sp.radius.max(0.5);
            let bound = half * 1.45 + 1.0;
            let x0c = ((sp.x - bound).floor() as i32).max(0);
            let x1c = ((sp.x + bound).ceil() as i32).min(wi - 1);
            let y0 = ((sp.y - bound).floor() as i32).max(by0);
            let y1 = ((sp.y + bound).ceil() as i32).min(by1 - 1);
            if x0c > x1c || y0 > y1 {
                return;
            }
            let cs = sp.rot.cos();
            let sn = sp.rot.sin();
            for py in y0..=y1 {
                let dy = py as f32 + 0.5 - sp.y;
                let row = (py - by0) as usize * w;
                for px in x0c..=x1c {
                    let dx = px as f32 + 0.5 - sp.x;
                    let lx = (dx * cs + dy * sn).abs();
                    let ly = (-dx * sn + dy * cs).abs();
                    let edge = lx.max(ly);
                    let cov = (half + 0.65 - edge).clamp(0.0, 1.0);
                    if cov > 1e-4 {
                        let idx = (row + px as usize) * 4;
                        blend_pixel(
                            dst,
                            idx,
                            sp.r * cov,
                            sp.g * cov,
                            sp.b * cov,
                            sp.a * cov,
                            glow,
                        );
                    }
                }
            }
        }
        3 => {
            // Velocity-aligned streak capsule.
            let r = sp.radius.max(0.55);
            let vlen = (sp.vx * sp.vx + sp.vy * sp.vy).sqrt();
            let (dir_x, dir_y) = if vlen > 1e-3 {
                (sp.vx / vlen, sp.vy / vlen)
            } else {
                (1.0, 0.0)
            };
            let half_len = (r * 1.8 + vlen * 0.045).min(r * 6.0);
            let bx = (dir_x.abs() * half_len + r + 1.0).ceil();
            let by = (dir_y.abs() * half_len + r + 1.0).ceil();
            let x0c = ((sp.x - bx).floor() as i32).max(0);
            let x1c = ((sp.x + bx).ceil() as i32).min(wi - 1);
            let y0 = ((sp.y - by).floor() as i32).max(by0);
            let y1 = ((sp.y + by).ceil() as i32).min(by1 - 1);
            if x0c > x1c || y0 > y1 {
                return;
            }
            for py in y0..=y1 {
                let dy = py as f32 + 0.5 - sp.y;
                let row = (py - by0) as usize * w;
                for px in x0c..=x1c {
                    let dx = px as f32 + 0.5 - sp.x;
                    let along = (dx * dir_x + dy * dir_y).clamp(-half_len, half_len);
                    let px_d = dx - dir_x * along;
                    let py_d = dy - dir_y * along;
                    let dist = (px_d * px_d + py_d * py_d).sqrt();
                    let cov = (1.0 - dist / (r + 0.5)).clamp(0.0, 1.0);
                    let cov = cov * cov;
                    if cov > 1e-4 {
                        let idx = (row + px as usize) * 4;
                        blend_pixel(
                            dst,
                            idx,
                            sp.r * cov,
                            sp.g * cov,
                            sp.b * cov,
                            sp.a * cov,
                            glow,
                        );
                    }
                }
            }
        }
        _ => {
            // Soft circle (kind 0) or Hard anti-aliased circle (kind 1).
            let r = sp.radius.max(0.5);
            let soft = sp.kind == 0;
            let bound = r + 1.0;
            let x0c = ((sp.x - bound).floor() as i32).max(0);
            let x1c = ((sp.x + bound).ceil() as i32).min(wi - 1);
            let y0 = ((sp.y - bound).floor() as i32).max(by0);
            let y1 = ((sp.y + bound).ceil() as i32).min(by1 - 1);
            if x0c > x1c || y0 > y1 {
                return;
            }
            let inv_r = 1.0 / (r + 0.5);
            for py in y0..=y1 {
                let dy = py as f32 + 0.5 - sp.y;
                let dy2 = dy * dy;
                let row = (py - by0) as usize * w;
                for px in x0c..=x1c {
                    let dx = px as f32 + 0.5 - sp.x;
                    let d2 = dx * dx + dy2;
                    let max_r = r + 0.65;
                    if d2 >= max_r * max_r {
                        continue;
                    }
                    let dist = d2.sqrt();
                    let cov = if soft {
                        let t = (1.0 - dist * inv_r).clamp(0.0, 1.0);
                        t * t * (3.0 - 2.0 * t)
                    } else {
                        (r + 0.5 - dist).clamp(0.0, 1.0)
                    };
                    if cov > 1e-4 {
                        let idx = (row + px as usize) * 4;
                        blend_pixel(
                            dst,
                            idx,
                            sp.r * cov,
                            sp.g * cov,
                            sp.b * cov,
                            sp.a * cov,
                            glow,
                        );
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Main render entry point
// ---------------------------------------------------------------------------

pub fn render<L: Layers>(
    layers: &mut L,
    px: &mut [f32],
    w: usize,
    h: usize,
    origin: [f64; 2],
    layer_size: [f64; 2],
    scale: f64,
    time: f64,
    flat: &[f64],
) -> Result<(), String> {
    if w == 0 || h == 0 || px.len() < w * h * 4 {
        return Ok(());
    }
    let p = Params::parse(flat);
    if p.mix <= 1e-5 || p.progress <= 1e-5 {
        return Ok(());
    }

    let sc = if scale.is_finite() && scale > 0.0 {
        scale as f32
    } else {
        1.0
    };

    // 1. Scan content bounding box (alpha > 0.01) so linear mask cleanly sweeps across the text itself.
    let mut min_x = w;
    let mut max_x = 0usize;
    let mut min_y = h;
    let mut max_y = 0usize;
    for y in 0..h {
        let row = y * w;
        for x in 0..w {
            if px[(row + x) * 4 + 3] > 0.01 {
                if x < min_x {
                    min_x = x;
                }
                if x > max_x {
                    max_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if y > max_y {
                    max_y = y;
                }
            }
        }
    }
    if min_x > max_x || min_y > max_y {
        return Ok(());
    }

    // 2. Fetch custom mask layer if mask_type == 2 and a valid layer is chosen.
    let custom_img = if p.mask_type == 2 && p.mask_layer >= 0 {
        layers.layer_image("mask_layer", p.mask_layer, time)
    } else {
        None
    };

    let mask_rad = p.mask_angle * (PI / 180.0);
    let mdx = mask_rad.cos();
    let mdy = mask_rad.sin();
    let corners = [
        (min_x as f32, min_y as f32),
        (max_x as f32, min_y as f32),
        (min_x as f32, max_y as f32),
        (max_x as f32, max_y as f32),
    ];
    let mut proj_min = f32::INFINITY;
    let mut proj_max = f32::NEG_INFINITY;
    for &(cx, cy) in &corners {
        let pr = cx * mdx + cy * mdy;
        if pr < proj_min {
            proj_min = pr;
        }
        if pr > proj_max {
            proj_max = pr;
        }
    }
    let proj_inv = if (proj_max - proj_min).abs() > 1e-4 {
        1.0 / (proj_max - proj_min)
    } else {
        0.0
    };

    let mask_ctx = MaskCtx {
        mode: p.mask_type,
        dx: mdx,
        dy: mdy,
        proj_min,
        proj_inv,
        jitter: p.mask_jitter,
        inv_noise_scale: 1.0 / (p.noise_scale * sc).max(2.0),
        seed: p.seed,
        invert: p.mask_invert,
        channel: p.mask_channel,
        custom: custom_img.as_ref(),
        frame_w: w,
        frame_h: h,
        frame_origin: origin,
        frame_scale: scale,
        layer_size,
    };

    // Front sweep mapping:
    //   progress = 0   -> front = -feather       (nothing dissolved yet)
    //   progress = 1   -> front = 1 + feather + lifetime * (1 + life_random * 0.5) (all particles completely gone)
    let feather = p.feather.max(0.01);
    let max_life = p.lifetime * (1.0 + p.life_random * 0.5);
    let front = -feather + p.progress * (1.0 + 2.0 * feather + max_life);

    let wind_rad = p.wind_angle * (PI / 180.0);
    let wind_x = wind_rad.cos() * p.wind_strength * sc;
    let wind_y = wind_rad.sin() * p.wind_strength * sc;
    let drift = (time as f32) * (p.time_evolve * 0.015);

    let src = px.to_vec();
    let mut out = vec![0.0f32; w * h * 4];
    let mut sprites: Vec<Sprite> = Vec::new();

    let n_threads = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(1, 8);

    if p.mode == 0 {
        // ===================================================================
        // MODE 0: Particle Emit
        // ===================================================================
        // 1) Fade remaining source text smoothly behind the dissolve front.
        let rows_per = (h + n_threads - 1) / n_threads;
        thread::scope(|s| {
            for (ti, chunk) in out.chunks_mut(rows_per * w * 4).enumerate() {
                let y_start = ti * rows_per;
                let y_end = (y_start + rows_per).min(h);
                let src_ref = &src;
                let mctx = &mask_ctx;
                s.spawn(move || {
                    for y in y_start..y_end {
                        if y < min_y || y > max_y {
                            continue;
                        }
                        let dst_row = (y - y_start) * w;
                        let src_row = y * w;
                        for x in min_x..=max_x {
                            let si = (src_row + x) * 4;
                            let sa = src_ref[si + 3];
                            if sa <= 1e-4 {
                                continue;
                            }
                            let m = mctx.eval(x as f32 + 0.5, y as f32 + 0.5);
                            let raw = front - m;
                            let stay = if raw <= 0.0 {
                                1.0
                            } else if raw >= feather {
                                0.0
                            } else {
                                1.0 - smoothstep(0.0, feather, raw)
                            };
                            if stay > 1e-4 {
                                let di = (dst_row + x) * 4;
                                chunk[di] = src_ref[si] * stay;
                                chunk[di + 1] = src_ref[si + 1] * stay;
                                chunk[di + 2] = src_ref[si + 2] * stay;
                                chunk[di + 3] = sa * stay;
                            }
                        }
                    }
                });
            }
        });

        // 2) Emit particles on a jittered grid across content bounds, sampling source pixel color.
        let step = ((8.2 - 6.95 * p.particle_density.powf(0.75)) * sc).max(1.2);
        let gx0 = (min_x as f32 / step).floor() as i32;
        let gx1 = (max_x as f32 / step).ceil() as i32;
        let gy0 = (min_y as f32 / step).floor() as i32;
        let gy1 = (max_y as f32 / step).ceil() as i32;
        let total_gy = (gy1 - gy0 + 1).max(0) as usize;
        let gy_chunk = (total_gy + n_threads - 1) / n_threads;

        let mut per_thread: Vec<Vec<Sprite>> = vec![Vec::new(); n_threads];
        thread::scope(|s| {
            for (ti, bucket) in per_thread.iter_mut().enumerate() {
                let y_a = gy0 + (ti * gy_chunk) as i32;
                let y_b = (y_a + gy_chunk as i32).min(gy1 + 1);
                if y_a >= y_b {
                    continue;
                }
                let src_ref = &src;
                let mctx = &mask_ctx;
                s.spawn(move || {
                    for gy in y_a..y_b {
                        for gx in gx0..=gx1 {
                            let jx = hash3(gx, gy, p.seed.wrapping_add(11)) - 0.5;
                            let jy = hash3(gx, gy, p.seed.wrapping_add(29)) - 0.5;
                            let sx = (gx as f32 + 0.5 + jx * 0.9) * step;
                            let sy = (gy as f32 + 0.5 + jy * 0.9) * step;
                            if sx < min_x as f32 || sx > max_x as f32 || sy < min_y as f32 || sy > max_y as f32 {
                                continue;
                            }

                            let (rgb, alpha) =
                                sample_particle_color(src_ref, w, h, sx, sy, p.particle_size * sc);
                            if alpha < 0.04 {
                                continue;
                            }

                            let m = mctx.eval(sx, sy);
                            let h_born = hash3(gx, gy, p.seed.wrapping_add(53));
                            let born = m + (h_born - 0.1) * feather * 0.85;
                            let elapsed = front - born;
                            if elapsed <= 0.0 {
                                continue;
                            }

                            let h_life = hash3(gx, gy, p.seed.wrapping_add(71));
                            let life_var =
                                (1.0 + (h_life - 0.5) * p.life_random).max(0.2);
                            let u = elapsed / (p.lifetime * life_var);
                            if u >= 1.0 {
                                continue;
                            }

                            let r_dir = hash3(gx, gy, p.seed.wrapping_add(107));
                            let r_spd = hash3(gx, gy, p.seed.wrapping_add(131));
                            let r_scat = hash3(gx, gy, p.seed.wrapping_add(163));
                            let r_sz = hash3(gx, gy, p.seed.wrapping_add(197));
                            let r_rot = hash3(gx, gy, p.seed.wrapping_add(233));

                            let (nx, ny, vx, vy) = compute_physics(
                                sx, sy, u, r_dir, r_spd, r_scat, &p, sc, wind_x, wind_y, drift,
                            );

                            let size_var = (1.0 + (r_sz * 2.0 - 1.0) * p.size_random * 0.65).max(0.2);
                            let life_scale = 1.0 + (p.size_over_life - 1.0) * u;
                            let rad = (p.particle_size * sc * size_var * life_scale).max(0.35);

                            let fade = eval_fade_curve(u, p.fade_out_curve);
                            let pa = (alpha * fade).clamp(0.0, 1.0);
                            if pa <= 1e-4 {
                                continue;
                            }

                            let mut cr = rgb[0];
                            let mut cg = rgb[1];
                            let mut cb = rgb[2];
                            if p.color_fade_enable && p.color_fade_mix > 1e-4 {
                                let cf = (u.powf(0.85) * p.color_fade_mix).clamp(0.0, 1.0);
                                cr += (p.end_color[0] - cr) * cf;
                                cg += (p.end_color[1] - cg) * cf;
                                cb += (p.end_color[2] - cb) * cf;
                            }

                            let kind = (p.particle_shape as u8).min(3);
                            let bound_y = if kind == 3 {
                                let vlen = (vx * vx + vy * vy).sqrt();
                                let hl = (rad * 1.8 + vlen * 0.045).min(rad * 6.0);
                                hl + rad + 2.0
                            } else {
                                rad * 1.5 + 2.0
                            };
                            let sp_min_y = (ny - bound_y).floor() as i32;
                            let sp_max_y = (ny + bound_y).ceil() as i32;
                            if sp_max_y < 0 || sp_min_y >= h as i32 {
                                continue;
                            }
                            if nx + bound_y < 0.0 || nx - bound_y >= w as f32 {
                                continue;
                            }

                            let spin = (r_rot - 0.5) * 8.0 * u + r_rot * TAU;
                            bucket.push(Sprite {
                                x: nx,
                                y: ny,
                                vx,
                                vy,
                                radius: rad,
                                rot: spin,
                                r: cr * pa,
                                g: cg * pa,
                                b: cb * pa,
                                a: pa,
                                kind,
                                min_y: sp_min_y,
                                max_y: sp_max_y,
                            });
                        }
                    }
                });
            }
        });
        for mut b in per_thread {
            sprites.append(&mut b);
        }
    } else {
        // ===================================================================
        // MODE 1: Direct Pixel Separation
        // ===================================================================
        let bs = ((p.pixel_block as f32) * sc).round().max(1.0) as usize;
        let bx_count = ((max_x - min_x) / bs) + 1;
        let by_count = ((max_y - min_y) / bs) + 1;

        // Partition blocks by rows across threads; untouched blocks write straight to `out`,
        // detached blocks become flying pixel sprites.
        let by_per = (by_count + n_threads - 1) / n_threads;
        let mut per_thread: Vec<Vec<Sprite>> = vec![Vec::new(); n_threads];

        // Copy untouched blocks directly in a single clean pass (fast & borrow-safe),
        // while collecting detached pixel blocks in parallel.
        thread::scope(|s| {
            for (ti, bucket) in per_thread.iter_mut().enumerate() {
                let by_a = ti * by_per;
                let by_b = (by_a + by_per).min(by_count);
                if by_a >= by_b {
                    continue;
                }
                let src_ref = &src;
                let mctx = &mask_ctx;
                s.spawn(move || {
                    for by in by_a..by_b {
                        let py0 = min_y + by * bs;
                        let py1 = (py0 + bs).min(max_y + 1);
                        for bx in 0..bx_count {
                            let px0 = min_x + bx * bs;
                            let px1 = (px0 + bs).min(max_x + 1);

                            // Compute block average premultiplied RGBA.
                            let mut sum_r = 0.0f32;
                            let mut sum_g = 0.0f32;
                            let mut sum_b = 0.0f32;
                            let mut sum_a = 0.0f32;
                            let mut cnt = 0.0f32;
                            for yy in py0..py1 {
                                let row = yy * w;
                                for xx in px0..px1 {
                                    let si = (row + xx) * 4;
                                    sum_r += src_ref[si];
                                    sum_g += src_ref[si + 1];
                                    sum_b += src_ref[si + 2];
                                    sum_a += src_ref[si + 3];
                                    cnt += 1.0;
                                }
                            }
                            if sum_a <= 1e-4 {
                                continue;
                            }
                            let avg_a = (sum_a / cnt).clamp(0.0, 1.0);
                            if avg_a < 0.01 {
                                continue;
                            }

                            let cx = (px0 as f32 + px1 as f32) * 0.5;
                            let cy = (py0 as f32 + py1 as f32) * 0.5;
                            let m = mctx.eval(cx, cy);
                            let h_born = hash3(bx as i32, by as i32, p.seed.wrapping_add(41));
                            let born = m + h_born * feather * 0.8;
                            let elapsed = front - born;
                            if elapsed <= 0.0 {
                                // Still attached: handled in the untouched copy pass below.
                                continue;
                            }

                            let h_life = hash3(bx as i32, by as i32, p.seed.wrapping_add(73));
                            let life_var =
                                (1.0 + (h_life - 0.5) * p.life_random).max(0.2);
                            let u = elapsed / (p.lifetime * life_var);
                            if u >= 1.0 {
                                continue;
                            }

                            let r_dir = hash3(bx as i32, by as i32, p.seed.wrapping_add(109));
                            let r_spd = hash3(bx as i32, by as i32, p.seed.wrapping_add(137));
                            let r_scat = hash3(bx as i32, by as i32, p.seed.wrapping_add(167));

                            let (nx, ny, vx, vy) = compute_physics(
                                cx, cy, u, r_dir, r_spd, r_scat, &p, sc, wind_x, wind_y, drift,
                            );

                            let fade = eval_fade_curve(u, p.fade_out_curve);
                            let pa = (avg_a * fade).clamp(0.0, 1.0);
                            if pa <= 1e-4 {
                                continue;
                            }

                            let mut cr = sum_r / sum_a;
                            let mut cg = sum_g / sum_a;
                            let mut cb = sum_b / sum_a;
                            if p.color_fade_enable && p.color_fade_mix > 1e-4 {
                                let cf = (u.powf(0.85) * p.color_fade_mix).clamp(0.0, 1.0);
                                cr += (p.end_color[0] - cr) * cf;
                                cg += (p.end_color[1] - cg) * cf;
                                cb += (p.end_color[2] - cb) * cf;
                            }

                            let life_scale = 1.0 + (p.size_over_life - 1.0) * u;
                            let cur_size = (bs as f32) * life_scale;
                            let (kind, rad) = if cur_size <= 1.25 {
                                (4u8, 0.5)
                            } else {
                                (5u8, cur_size * 0.5)
                            };

                            let bound = rad + 1.5;
                            let sp_min_y = (ny - bound).floor() as i32;
                            let sp_max_y = (ny + bound).ceil() as i32;
                            if sp_max_y < 0 || sp_min_y >= h as i32 {
                                continue;
                            }
                            if nx + bound < 0.0 || nx - bound >= w as f32 {
                                continue;
                            }

                            bucket.push(Sprite {
                                x: if kind == 4 { nx - 0.5 } else { nx },
                                y: if kind == 4 { ny - 0.5 } else { ny },
                                vx,
                                vy,
                                radius: rad,
                                rot: 0.0,
                                r: cr * pa,
                                g: cg * pa,
                                b: cb * pa,
                                a: pa,
                                kind,
                                min_y: sp_min_y,
                                max_y: sp_max_y,
                            });
                        }
                    }
                });
            }
        });

        // Write still-attached pixel blocks verbatim into `out` (100% crisp preservation).
        for by in 0..by_count {
            let py0 = min_y + by * bs;
            let py1 = (py0 + bs).min(max_y + 1);
            for bx in 0..bx_count {
                let px0 = min_x + bx * bs;
                let px1 = (px0 + bs).min(max_x + 1);
                let cx = (px0 as f32 + px1 as f32) * 0.5;
                let cy = (py0 as f32 + py1 as f32) * 0.5;
                let m = mask_ctx.eval(cx, cy);
                let h_born = hash3(bx as i32, by as i32, p.seed.wrapping_add(41));
                let born = m + h_born * feather * 0.8;
                if front - born <= 0.0 {
                    for yy in py0..py1 {
                        let row = yy * w;
                        for xx in px0..px1 {
                            let idx = (row + xx) * 4;
                            out[idx] = src[idx];
                            out[idx + 1] = src[idx + 1];
                            out[idx + 2] = src[idx + 2];
                            out[idx + 3] = src[idx + 3];
                        }
                    }
                }
            }
        }

        for mut b in per_thread {
            sprites.append(&mut b);
        }
    }

    // 3. Parallel band-partitioned sprite rasterization onto `out` (deterministic & lock-free).
    if !sprites.is_empty() {
        let rows_per = (h + n_threads - 1) / n_threads;
        let sprites_ref = &sprites;
        let glow = p.additive_glow;
        thread::scope(|s| {
            for (ti, band) in out.chunks_mut(rows_per * w * 4).enumerate() {
                let y0 = ti * rows_per;
                let y1 = (y0 + rows_per).min(h);
                let y0i = y0 as i32;
                let y1i = y1 as i32;
                s.spawn(move || {
                    for sp in sprites_ref.iter() {
                        if sp.max_y < y0i || sp.min_y >= y1i {
                            continue;
                        }
                        rasterize_sprite_band(sp, band, w, y0, y1, glow);
                    }
                });
            }
        });
    }

    // 4. Final mix with original frame + finite/alpha sanitization.
    let mix = p.mix;
    let inv_mix = 1.0 - mix;
    for i in 0..(w * h) {
        let o = i * 4;
        let mut r = out[o];
        let mut g = out[o + 1];
        let mut b = out[o + 2];
        let mut a = out[o + 3];
        if mix < 0.9999 {
            r = src[o] * inv_mix + r * mix;
            g = src[o + 1] * inv_mix + g * mix;
            b = src[o + 2] * inv_mix + b * mix;
            a = src[o + 3] * inv_mix + a * mix;
        }
        px[o] = if r.is_finite() { r.max(0.0) } else { 0.0 };
        px[o + 1] = if g.is_finite() { g.max(0.0) } else { 0.0 };
        px[o + 2] = if b.is_finite() { b.max(0.0) } else { 0.0 };
        px[o + 3] = if a.is_finite() { a.clamp(0.0, 1.0) } else { 0.0 };
    }

    Ok(())
}
