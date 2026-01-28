use anstyle::RgbColor;
use core::fmt::Write;
use heapless::{String, Vec};

use crate::{print, println};

// Logo 最大行长度（包括 ANSI 颜色代码）
const MAX_LINE_LEN: usize = 2048;

pub fn print_logo() {
    println!();
    println!();
    // 渐变色: #667eea -> #764ba2
    let start_color = (0x66, 0x7e, 0xea); // #667eea
    let end_color = (0x76, 0x4b, 0xa2); // #764ba2

    let logo = include_str!("logo.txt");

    // 逐行应用横向渐变
    for line in logo.lines() {
        let gradient_line = apply_gradient_line(line, start_color, end_color);
        print!("{gradient_line}");
    }

    println!();
    println!();
}

/// 线性插值
fn lerp(start: u8, end: u8, t: f32) -> u8 {
    (start as f32 + t * (end as f32 - start as f32)) as u8
}

/// 对单行应用横向渐变色（使用 heapless，无需堆分配）
fn apply_gradient_line(
    line: &str,
    start_rgb: (u8, u8, u8),
    end_rgb: (u8, u8, u8),
) -> String<MAX_LINE_LEN> {
    let mut chars = Vec::<char, 128>::new();
    for c in line.chars() {
        if chars.push(c).is_err() {
            break; // 行太长，截断
        }
    }

    let len = chars.len();
    if len == 0 {
        return String::new();
    }

    let mut result = String::<MAX_LINE_LEN>::new();

    for (i, &c) in chars.iter().enumerate() {
        let t = if len <= 1 {
            0.0
        } else {
            i as f32 / (len - 1) as f32
        };
        let r = lerp(start_rgb.0, end_rgb.0, t);
        let g = lerp(start_rgb.1, end_rgb.1, t);
        let b = lerp(start_rgb.2, end_rgb.2, t);

        // 使用 anstyle 设置前景色
        let style = anstyle::Style::new().fg_color(Some(RgbColor(r, g, b).into()));

        // 拼接样式和字符，使用 write! 避免堆分配
        if write!(
            &mut result,
            "{}{}{}",
            style.render(),
            c,
            anstyle::Reset.render()
        )
        .is_err()
        {
            break; // 结果已满，截断
        }
    }

    // 添加换行符
    let _ = result.push('\n');
    result
}
