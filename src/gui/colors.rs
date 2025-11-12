use std::str::FromStr;

use palette::{LinSrgb, Mix, Srgb};

/// Blend a color toward white in linear light by `amount` in [0, 1].
/// Returns a lowercase `#rrggbb` hex string.
pub fn wash_out_by_blending(hex: &str, amount: f32) -> Result<String, String> {
    let srgb_u8: Srgb<u8> = Srgb::from_str(hex).map_err(|e| e.to_string())?;
    let srgb_f32: Srgb<f32> = srgb_u8.into_format();
    let lin: LinSrgb<f32> = srgb_f32.into_linear();

    // Mix toward linear white
    let amount = amount.clamp(0.0, 1.0);
    let white = LinSrgb::new(1.0, 1.0, 1.0);
    let mixed: LinSrgb<f32> = lin.mix(white, amount);

    // Convert back
    let out_srgb_f32: Srgb<f32> = mixed.into_encoding();
    let out_srgb_u8: Srgb<u8> = out_srgb_f32.into_format();
    let color_hex = format!("{:x}", out_srgb_u8);

    let color_hex = match color_hex.starts_with("#") {
        true => color_hex,
        false => format!("#{}", color_hex),
    };

    Ok(color_hex)
}
