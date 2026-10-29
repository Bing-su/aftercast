// ACT ColorRGB/Extensions.ToAdjustedColor: Color's HSL getters use f32,
// FromHSLA computes in f64 and Convert.ToByte rounds to even.
fn hsl(rgb: [u8; 3]) -> (f32, f32, f32) {
    let [red, green, blue] = rgb;
    let [r, g, b] = rgb.map(|v| f32::from(v) / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let diff = max - min;
    let light = (max + min) / 2.0;
    if diff == 0.0 {
        return (0.0, 0.0, light);
    }
    let saturation = if light <= 0.5 {
        diff / (max + min)
    } else {
        diff / (2.0 - max - min)
    };
    let mut hue = if red >= green && red >= blue {
        (g - b) / diff
    } else if green >= blue {
        2.0 + (b - r) / diff
    } else {
        4.0 + (r - g) / diff
    } * 60.0;
    if hue < 0.0 {
        hue += 360.0;
    }
    (hue / 360.0, saturation, light)
}
#[expect(
    clippy::cast_sign_loss,
    reason = "HSL clamps components to the nonnegative byte range"
)]
fn from_hsl(h: f32, s: f32, l: f64) -> [u8; 3] {
    let s = f64::from(s).clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    let m = if l > 0.5 {
        l + s - l * s
    } else {
        l * (1.0 + s)
    };
    let mut rgb = [l; 3];
    if m > 0.0 {
        let min = 2.0 * l - m;
        let h = f64::from(h) * 6.0;
        let sector = h as i32;
        let d = m * ((m - min) / m) * (h - f64::from(sector));
        let a = min + d;
        let b = m - d;
        rgb = match sector {
            0 => [m, a, min],
            1 => [b, m, min],
            2 => [min, m, a],
            3 => [min, b, m],
            4 => [a, min, m],
            5 => [m, min, b],
            _ => rgb,
        };
    }
    rgb.map(|v| (v * 255.0).round_ties_even() as u8)
}
#[cfg(test)]
pub(super) fn adjust(argb: i32, invert: bool, luminosity: i8, saturation: i8) -> i32 {
    adjust_runtime(
        argb,
        invert,
        f32::from(luminosity) / 100.0,
        f32::from(saturation) / 100.0,
    )
}
pub(super) fn adjust_runtime(argb: i32, invert: bool, luminosity: f32, saturation: f32) -> i32 {
    let value = argb.cast_unsigned();
    let rgb = [(value >> 16) as u8, (value >> 8) as u8, value as u8];
    let (h, s, l) = hsl(rgb);
    let mut rgb = from_hsl(
        h,
        (s + saturation).clamp(-1.0, 1.0),
        f64::from((l + luminosity).clamp(-1.0, 1.0)),
    );
    if invert {
        let (h, s, l) = hsl(rgb);
        rgb = if f64::from(l) > 0.45 && f64::from(l) < 0.55 {
            from_hsl(h, s, 0.65)
        } else {
            from_hsl(h, s, f64::from(1.0 - l))
        };
    }
    ((value & 0xff000000)
        | (u32::from(rgb[0]) << 16)
        | (u32::from(rgb[1]) << 8)
        | u32::from(rgb[2]))
    .cast_signed()
}
#[cfg(test)]
mod tests {
    use super::adjust;
    #[test]
    fn original_act_varied_offsets_and_alpha() {
        for line in include_str!("../../../tests/fixtures/act-color-adjust.txt").lines() {
            let values: Vec<i32> = line
                .split_whitespace()
                .map(|s| s.parse().expect("oracle integer"))
                .collect();
            let [color, invert, lum, sat, expected] = values.as_slice() else {
                panic!("oracle row");
            };
            assert_eq!(
                adjust(
                    *color,
                    *invert == 1,
                    i8::try_from(*lum).expect("lum"),
                    i8::try_from(*sat).expect("sat")
                ),
                *expected,
                "{line}"
            );
        }
    }
    #[test]
    fn original_act_samples() {
        for (color, invert, offset, expected) in [
            (301924352, false, 0, 301924352),
            (301924352, false, 20, 301950566),
            (301924352, false, -20, 294260495),
            (301924352, true, 0, 301944141),
            (301924352, true, 20, 295239680),
            (301924352, true, -20, 300971381),
            (-16777216, false, 0, -16777216),
            (-16777216, false, 20, -12768983),
            (-16777216, false, -20, -16777216),
            (-16777216, true, 0, -1),
            (-16777216, true, 20, -2702654),
            (-16777216, true, -20, -1),
            (-8421505, false, 0, -8421505),
            (-8421505, false, 20, -4086877),
            (-8421505, false, -20, -11776948),
            (-8421505, true, 0, -5855578),
            (-8421505, true, 20, -10731970),
            (-8421505, true, -20, -5000269),
            (-14795686, false, 0, -14795686),
            (-14795686, false, 20, -14585923),
            (-14795686, false, -20, -16381684),
            (-14795686, true, 0, -5913631),
            (-14795686, true, 20, -12414754),
            (-14795686, true, -20, -788743),
        ] {
            assert_eq!(
                adjust(color, invert, offset, offset),
                expected,
                "{color} {invert} {offset}"
            );
        }
    }
}
