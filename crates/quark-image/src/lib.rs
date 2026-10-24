use std::io::Cursor;

use image::imageops::{self, FilterType};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use mlua::prelude::*;
use quark_core::{
    Binary, Bytes, Object, Options, create_userdata, index_base, options_and_callback,
};

const MAX_SIDE: u32 = 16_384;
const MAX_PIXELS: u64 = 64 * 1024 * 1024;

pub struct Image {
    pixels: RgbaImage,
    base: i64,
}

fn check_size(width: u32, height: u32) -> Result<(), String> {
    if width == 0
        || height == 0
        || width > MAX_SIDE
        || height > MAX_SIDE
        || u64::from(width) * u64::from(height) > MAX_PIXELS
    {
        return Err(format!(
            "images need between 1 and {MAX_SIDE} pixels per side and at most {MAX_PIXELS} pixels"
        ));
    }
    Ok(())
}

fn hex_digit(character: char) -> LuaResult<u8> {
    character
        .to_digit(16)
        .map(|digit| digit as u8)
        .ok_or_else(|| LuaError::runtime(format!("'{character}' is not a hex digit")))
}

pub fn read_color(value: &LuaValue) -> LuaResult<Rgba<u8>> {
    let channel = |number: f64| number.round().clamp(0.0, 255.0) as u8;
    match value {
        LuaValue::String(text) => {
            let text = text.to_str()?;
            let digits: Vec<char> = text.trim_start_matches('#').chars().collect();
            let pair = |index: usize| -> LuaResult<u8> {
                Ok(hex_digit(digits[index])? * 16 + hex_digit(digits[index + 1])?)
            };
            match digits.len() {
                3 | 4 => {
                    let single =
                        |index: usize| -> LuaResult<u8> { Ok(hex_digit(digits[index])? * 17) };
                    let alpha = if digits.len() == 4 { single(3)? } else { 255 };
                    Ok(Rgba([single(0)?, single(1)?, single(2)?, alpha]))
                }
                6 | 8 => {
                    let alpha = if digits.len() == 8 { pair(6)? } else { 255 };
                    Ok(Rgba([pair(0)?, pair(2)?, pair(4)?, alpha]))
                }
                _ => Err(LuaError::runtime(format!(
                    "'{text}' is not a color like #rrggbb"
                ))),
            }
        }
        LuaValue::Table(table) => {
            if let Some(red) = table.get::<Option<f64>>("R")? {
                let green: f64 = table.get::<Option<f64>>("G")?.unwrap_or(0.0);
                let blue: f64 = table.get::<Option<f64>>("B")?.unwrap_or(0.0);
                let alpha: f64 = table.get::<Option<f64>>("A")?.unwrap_or(1.0);
                return Ok(Rgba([
                    channel(red * 255.0),
                    channel(green * 255.0),
                    channel(blue * 255.0),
                    channel(alpha * 255.0),
                ]));
            }
            let component = |index: usize, default: f64| -> LuaResult<u8> {
                Ok(channel(
                    table.raw_get::<Option<f64>>(index)?.unwrap_or(default),
                ))
            };
            Ok(Rgba([
                component(1, 0.0)?,
                component(2, 0.0)?,
                component(3, 0.0)?,
                component(4, 255.0)?,
            ]))
        }
        LuaValue::UserData(userdata) => {
            let get = |name: &str, default: f64| -> LuaResult<f64> {
                Ok(userdata.get::<Option<f64>>(name)?.unwrap_or(default))
            };
            Ok(Rgba([
                channel(get("R", 0.0)? * 255.0),
                channel(get("G", 0.0)? * 255.0),
                channel(get("B", 0.0)? * 255.0),
                channel(get("A", 1.0)? * 255.0),
            ]))
        }
        other => Err(LuaError::runtime(format!(
            "expected a color, got {}",
            other.type_name()
        ))),
    }
}

fn blend(destination: &mut Rgba<u8>, source: Rgba<u8>, opacity: f32) {
    let alpha = f32::from(source[3]) / 255.0 * opacity;
    if alpha <= 0.0 {
        return;
    }
    let below = f32::from(destination[3]) / 255.0;
    let out = alpha + below * (1.0 - alpha);
    for channel in 0..3 {
        let mixed = (f32::from(source[channel]) * alpha
            + f32::from(destination[channel]) * below * (1.0 - alpha))
            / out.max(f32::EPSILON);
        destination[channel] = mixed.round().clamp(0.0, 255.0) as u8;
    }
    destination[3] = (out * 255.0).round() as u8;
}

fn format_of(name: &str) -> LuaResult<ImageFormat> {
    match name.to_ascii_lowercase().as_str() {
        "png" => Ok(ImageFormat::Png),
        "jpeg" | "jpg" => Ok(ImageFormat::Jpeg),
        "webp" => Ok(ImageFormat::WebP),
        "bmp" => Ok(ImageFormat::Bmp),
        other => Err(LuaError::runtime(format!(
            "unknown image format '{other}', expected png, jpeg, webp or bmp"
        ))),
    }
}

fn encode(pixels: &RgbaImage, format: ImageFormat, quality: u8) -> Result<Vec<u8>, String> {
    let mut output = Cursor::new(Vec::new());
    let failed = |error: image::ImageError| format!("cannot encode the image: {error}");
    match format {
        ImageFormat::Jpeg => {
            let rgb = DynamicImage::ImageRgba8(pixels.clone()).to_rgb8();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut output, quality)
                .encode_image(&rgb)
                .map_err(failed)?;
        }
        format => {
            pixels.write_to(&mut output, format).map_err(failed)?;
        }
    }
    Ok(output.into_inner())
}

fn decode(data: &[u8]) -> Result<RgbaImage, String> {
    let reader = image::ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|error| format!("cannot read the image: {error}"))?;
    let dimensions = reader
        .into_dimensions()
        .map_err(|error| format!("cannot read the image: {error}"))?;
    check_size(dimensions.0, dimensions.1)?;
    let image = image::load_from_memory(data)
        .map_err(|error| format!("cannot decode the image: {error}"))?;
    Ok(image.to_rgba8())
}

struct Decoded(RgbaImage);

impl Image {
    fn position(&self, x: i64, y: i64) -> Option<(u32, u32)> {
        let (column, row) = (x - self.base, y - self.base);
        (column >= 0
            && row >= 0
            && (column as u32) < self.pixels.width()
            && (row as u32) < self.pixels.height())
        .then_some((column as u32, row as u32))
    }

    fn put(&mut self, x: i64, y: i64, color: Rgba<u8>, mix: bool) {
        if let Some((column, row)) = self.position(x, y) {
            let pixel = self.pixels.get_pixel_mut(column, row);
            if mix {
                blend(pixel, color, 1.0);
            } else {
                *pixel = color;
            }
        }
    }

    fn derived(&self, pixels: RgbaImage) -> Object<Image> {
        Object(Image {
            pixels,
            base: self.base,
        })
    }
}

impl LuaUserData for Image {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetSize", |_, image, ()| {
            Ok((image.pixels.width(), image.pixels.height()))
        });
        methods.add_method("GetPixel", |lua, image, (x, y): (i64, i64)| {
            let (column, row) = image.position(x, y).ok_or_else(|| {
                LuaError::runtime(format!("the pixel {x}, {y} is outside of the image"))
            })?;
            let Rgba([red, green, blue, alpha]) = *image.pixels.get_pixel(column, row);
            (red, green, blue, alpha).into_lua_multi(lua)
        });
        methods.add_method_mut(
            "SetPixel",
            |_, image, (x, y, color): (i64, i64, LuaValue)| {
                let color = read_color(&color)?;
                image.put(x, y, color, false);
                Ok(())
            },
        );
        methods.add_method_mut("Fill", |_, image, color: LuaValue| {
            let color = read_color(&color)?;
            image.pixels.pixels_mut().for_each(|pixel| *pixel = color);
            Ok(())
        });
        methods.add_method_mut(
            "FillRect",
            |_, image, (x, y, width, height, color): (i64, i64, i64, i64, LuaValue)| {
                let color = read_color(&color)?;
                for row in y..y + height {
                    for column in x..x + width {
                        image.put(column, row, color, true);
                    }
                }
                Ok(())
            },
        );
        methods.add_method_mut("DrawLine", |_, image, (x1, y1, x2, y2, color, thickness): (i64, i64, i64, i64, LuaValue, Option<i64>)| {
            let color = read_color(&color)?;
            let radius = (thickness.unwrap_or(1).max(1) - 1) / 2;
            let (mut x, mut y) = (x1, y1);
            let (dx, dy) = ((x2 - x1).abs(), -(y2 - y1).abs());
            let (sx, sy) = (if x1 < x2 { 1 } else { -1 }, if y1 < y2 { 1 } else { -1 });
            let mut error = dx + dy;
            loop {
                for oy in -radius..=radius {
                    for ox in -radius..=radius {
                        image.put(x + ox, y + oy, color, false);
                    }
                }
                if x == x2 && y == y2 {
                    break;
                }
                let doubled = 2 * error;
                if doubled >= dy {
                    error += dy;
                    x += sx;
                }
                if doubled <= dx {
                    error += dx;
                    y += sy;
                }
            }
            Ok(())
        });
        methods.add_method_mut(
            "DrawCircle",
            |_, image, (x, y, radius, color, filled): (i64, i64, i64, LuaValue, Option<bool>)| {
                let color = read_color(&color)?;
                let filled = filled.unwrap_or(true);
                let squared = radius * radius;
                for oy in -radius..=radius {
                    for ox in -radius..=radius {
                        let distance = ox * ox + oy * oy;
                        if distance <= squared && (filled || distance > (radius - 1).pow(2)) {
                            image.put(x + ox, y + oy, color, true);
                        }
                    }
                }
                Ok(())
            },
        );
        methods.add_method_mut(
            "DrawImage",
            |_, image, (other, x, y, options): (LuaAnyUserData, i64, i64, Options)| {
                let opacity: f32 = options.get_or("Opacity", 1.0)?;
                let source = other.borrow::<Image>()?.pixels.clone();
                for (column, row, pixel) in source.enumerate_pixels() {
                    if let Some((target_x, target_y)) =
                        image.position(x + i64::from(column), y + i64::from(row))
                    {
                        blend(
                            image.pixels.get_pixel_mut(target_x, target_y),
                            *pixel,
                            opacity,
                        );
                    }
                }
                Ok(())
            },
        );
        methods.add_method(
            "Resize",
            |_, image, (width, height, filter): (u32, u32, Option<String>)| {
                check_size(width, height).map_err(LuaError::runtime)?;
                let filter = match filter.as_deref().unwrap_or("Linear") {
                    "Nearest" => FilterType::Nearest,
                    "Linear" => FilterType::Triangle,
                    "Cubic" => FilterType::CatmullRom,
                    "Lanczos" => FilterType::Lanczos3,
                    other => {
                        return Err(LuaError::runtime(format!(
                            "unknown filter '{other}', expected Nearest, Linear, Cubic or Lanczos"
                        )));
                    }
                };
                Ok(image.derived(imageops::resize(&image.pixels, width, height, filter)))
            },
        );
        methods.add_method(
            "Crop",
            |_, image, (x, y, width, height): (i64, i64, u32, u32)| {
                let (column, row) = image.position(x, y).ok_or_else(|| {
                    LuaError::runtime(format!("the pixel {x}, {y} is outside of the image"))
                })?;
                let width = width.min(image.pixels.width() - column);
                let height = height.min(image.pixels.height() - row);
                check_size(width, height).map_err(LuaError::runtime)?;
                Ok(image.derived(
                    imageops::crop_imm(&image.pixels, column, row, width, height).to_image(),
                ))
            },
        );
        methods.add_method("Rotate", |_, image, degrees: i64| {
            Ok(image.derived(match degrees.rem_euclid(360) {
                0 => image.pixels.clone(),
                90 => imageops::rotate90(&image.pixels),
                180 => imageops::rotate180(&image.pixels),
                270 => imageops::rotate270(&image.pixels),
                _ => {
                    return Err(LuaError::runtime(
                        "images can be rotated by 90, 180 or 270 degrees",
                    ));
                }
            }))
        });
        methods.add_method("Flip", |_, image, direction: Option<String>| {
            Ok(
                image.derived(match direction.as_deref().unwrap_or("Horizontal") {
                    "Horizontal" => imageops::flip_horizontal(&image.pixels),
                    "Vertical" => imageops::flip_vertical(&image.pixels),
                    other => {
                        return Err(LuaError::runtime(format!(
                            "unknown direction '{other}', expected Horizontal or Vertical"
                        )));
                    }
                }),
            )
        });
        methods.add_method("Blur", |_, image, sigma: f32| {
            Ok(image.derived(imageops::blur(&image.pixels, sigma.max(0.1))))
        });
        methods.add_method("Grayscale", |_, image, ()| {
            Ok(image
                .derived(DynamicImage::ImageLuma8(imageops::grayscale(&image.pixels)).to_rgba8()))
        });
        methods.add_method("Clone", |_, image, ()| {
            Ok(image.derived(image.pixels.clone()))
        });
        methods.add_method("GetBytes", |_, image, ()| {
            Ok(Bytes(image.pixels.as_raw().clone()))
        });
        methods.add_method(
            "Encode",
            |lua, image, (format, options, callback): (Option<String>, LuaValue, Option<LuaFunction>)| {
                let (options, callback) = options_and_callback(options, callback)?;
                let format = format_of(format.as_deref().unwrap_or("png"))?;
                let quality: u8 = options.get_or("Quality", 85)?;
                if !(1..=100).contains(&quality) {
                    return Err(LuaError::runtime("Quality has to be between 1 and 100"));
                }
                let pixels = image.pixels.clone();
                quark_core::run(lua, callback, move || encode(&pixels, format, quality).map(Bytes))
            },
        );
    }
}

impl IntoLuaMulti for Decoded {
    fn into_lua_multi(self, lua: &Lua) -> LuaResult<LuaMultiValue> {
        create_userdata(
            lua,
            Image {
                pixels: self.0,
                base: index_base(lua)?,
            },
        )?
        .into_lua_multi(lua)
    }
}

fn gradient(stops: &[(f64, Rgba<u8>)], value: f64) -> Rgba<u8> {
    if value <= stops[0].0 {
        return stops[0].1;
    }
    for pair in stops.windows(2) {
        let ((low, from), (high, to)) = (pair[0], pair[1]);
        if value <= high {
            let t = if high > low {
                (value - low) / (high - low)
            } else {
                0.0
            };
            let mix =
                |a: u8, b: u8| (f64::from(a) + (f64::from(b) - f64::from(a)) * t).round() as u8;
            return Rgba([
                mix(from[0], to[0]),
                mix(from[1], to[1]),
                mix(from[2], to[2]),
                mix(from[3], to[3]),
            ]);
        }
    }
    stops[stops.len() - 1].1
}

fn read_values(values: &LuaValue) -> LuaResult<Vec<f64>> {
    match values {
        LuaValue::Table(table) => table.sequence_values::<f64>().collect(),
        LuaValue::UserData(userdata) => quark_core::view_numbers(userdata)?
            .ok_or_else(|| LuaError::runtime("expected a list of numbers or a buffer view")),
        other => Err(LuaError::runtime(format!(
            "expected a list of numbers or a buffer view, got {}",
            other.type_name()
        ))),
    }
}

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    module.set(
        "New",
        lua.create_function(
            |lua, (width, height, color): (u32, u32, Option<LuaValue>)| {
                check_size(width, height).map_err(LuaError::runtime)?;
                let color = match color {
                    Some(color) => read_color(&color)?,
                    None => Rgba([0, 0, 0, 0]),
                };
                create_userdata(
                    lua,
                    Image {
                        pixels: RgbaImage::from_pixel(width, height, color),
                        base: index_base(lua)?,
                    },
                )
            },
        )?,
    )?;
    module.set(
        "Decode",
        lua.create_function(|lua, (data, callback): (Binary, Option<LuaFunction>)| {
            quark_core::run(lua, callback, move || decode(&data.0).map(Decoded))
        })?,
    )?;
    module.set(
        "FromHeights",
        lua.create_function(
            |lua, (width, height, values, options): (u32, u32, LuaValue, Options)| {
                check_size(width, height).map_err(LuaError::runtime)?;
                let values = read_values(&values)?;
                if values.len() != (width * height) as usize {
                    return Err(LuaError::runtime(format!(
                        "expected {} values, got {}",
                        width * height,
                        values.len()
                    )));
                }
                let minimum = options
                    .get::<f64>("Min")?
                    .unwrap_or_else(|| values.iter().copied().fold(f64::INFINITY, f64::min));
                let maximum = options
                    .get::<f64>("Max")?
                    .unwrap_or_else(|| values.iter().copied().fold(f64::NEG_INFINITY, f64::max));
                let mut stops = Vec::new();
                match options.get::<LuaTable>("Colors")? {
                    Some(table) => {
                        for entry in table.sequence_values::<LuaTable>() {
                            let entry = entry?;
                            stops.push((
                                entry.raw_get::<f64>(1)?,
                                read_color(&entry.raw_get::<LuaValue>(2)?)?,
                            ));
                        }
                    }
                    None => {
                        stops.push((0.0, Rgba([0, 0, 0, 255])));
                        stops.push((1.0, Rgba([255, 255, 255, 255])));
                    }
                }
                if stops.is_empty() {
                    return Err(LuaError::runtime(
                        "Colors needs at least one { position, color } entry",
                    ));
                }
                stops.sort_by(|left, right| left.0.total_cmp(&right.0));
                let range = (maximum - minimum).max(f64::EPSILON);
                let pixels = RgbaImage::from_fn(width, height, |x, y| {
                    let value = values[(y * width + x) as usize];
                    gradient(&stops, ((value - minimum) / range).clamp(0.0, 1.0))
                });
                create_userdata(
                    lua,
                    Image {
                        pixels,
                        base: index_base(lua)?,
                    },
                )
            },
        )?,
    )?;
    module.set(
        "FromIndexed",
        lua.create_function(
            |lua, (width, height, data, palette): (u32, u32, LuaValue, LuaTable)| {
                check_size(width, height).map_err(LuaError::runtime)?;
                let indices: Vec<i64> = match &data {
                    LuaValue::String(text) => text
                        .as_bytes()
                        .iter()
                        .map(|byte| i64::from(*byte))
                        .collect(),
                    LuaValue::UserData(userdata) if userdata.is::<quark_core::Buffer>() => userdata
                        .borrow::<quark_core::Buffer>()?
                        .data
                        .iter()
                        .map(|byte| i64::from(*byte))
                        .collect(),
                    other => read_values(other)?
                        .into_iter()
                        .map(|value| value as i64)
                        .collect(),
                };
                if indices.len() != (width * height) as usize {
                    return Err(LuaError::runtime(format!(
                        "expected {} values, got {}",
                        width * height,
                        indices.len()
                    )));
                }
                let mut colors = std::collections::HashMap::new();
                for pair in palette.pairs::<i64, LuaValue>() {
                    let (index, color) = pair?;
                    colors.insert(index, read_color(&color)?);
                }
                let pixels = RgbaImage::from_fn(width, height, |x, y| {
                    colors
                        .get(&indices[(y * width + x) as usize])
                        .copied()
                        .unwrap_or(Rgba([0, 0, 0, 0]))
                });
                create_userdata(
                    lua,
                    Image {
                        pixels,
                        base: index_base(lua)?,
                    },
                )
            },
        )?,
    )?;
    module.set(
        "IsImage",
        lua.create_function(|_, value: LuaValue| {
            Ok(matches!(value, LuaValue::UserData(userdata) if userdata.is::<Image>()))
        })?,
    )?;
    Ok(module)
}
