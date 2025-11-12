use std::str::FromStr;

use palette::{FromColor, Lab, LinSrgb, Mix, Srgb};
use qmetaobject::{QEnum, QMetaType, QString, QVariantMap};

#[repr(C)]
#[derive(QEnum, Default, Clone, Copy)]
pub enum ColorType {
    #[default]
    Light,
    Dark,
}

impl ToString for ColorType {
    fn to_string(&self) -> String {
        match self {
            ColorType::Dark => "Dark",
            ColorType::Light => "Light",
        }
        .into()
    }
}

fn type_to_string(color_type: &ColorType) -> QString {
    match color_type {
        ColorType::Dark => "Dark",
        ColorType::Light => "Light",
    }
    .into()
}

fn string_to_type(qstr: &QString) -> ColorType {
    match qstr.to_string().as_str() {
        "Dark" => ColorType::Dark,
        "Light" => ColorType::Light,
        _ => panic!("Invalid color type: {}", qstr.to_string()),
    }
}

impl QMetaType for ColorType {
    const CONVERSION_FROM_STRING: Option<fn(&QString) -> Self> = Some(string_to_type);
    const CONVERSION_TO_STRING: Option<fn(&Self) -> QString> = Some(type_to_string);
}

pub struct ColorPacket {
    background_color: String,
    text_color_type: ColorType,
}

/// Blend a color toward white in linear light by `amount` in [0, 1].
/// Returns a lowercase `#rrggbb` hex string.
pub fn wash_out_by_blending(hex: &str, amount: f32) -> Result<ColorPacket, String> {
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

    Ok(ColorPacket {
        text_color_type: text_color_for_background(&color_hex)?,
        background_color: color_hex,
    })
}

pub fn text_color_for_background(hex: &str) -> Result<ColorType, String> {
    // Parse background color as Srgb<u8> → Srgb<f32>
    let srgb_u8: Srgb<u8> = Srgb::from_str(hex).map_err(|e| e.to_string())?;
    let srgb_f32: Srgb<f32> = srgb_u8.into_format();

    // Convert to Lab to access perceptual lightness
    let lab: Lab = Lab::from_color(srgb_f32);

    // L is 0–100, normalize to 0–1 for clarity
    let normalized_l = lab.l / 100.0;

    // Threshold: ~0.5 gives a good perceptual boundary
    if normalized_l > 0.5 {
        Ok(ColorType::Dark) // background is light → use dark text
    } else {
        Ok(ColorType::Light) // background is dark → use text text
    }
}

impl From<ColorPacket> for QVariantMap {
    fn from(value: ColorPacket) -> Self {
        let mut map = QVariantMap::default();
        map.insert(
            "backgroundColor".into(),
            QString::from(value.background_color).into(),
        );

        map.insert(
            "textColorType".into(),
            QString::from(value.text_color_type.to_string()).into(),
        );

        map
    }
}
