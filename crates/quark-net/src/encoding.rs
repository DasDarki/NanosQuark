use std::io::{Read, Write};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Encoding {
    Brotli,
    Zstd,
    Gzip,
}

pub const DEFAULT_ENCODINGS: &[Encoding] = &[Encoding::Brotli, Encoding::Zstd, Encoding::Gzip];
pub const ACCEPT_ENCODING: &str = "gzip, br, zstd, deflate";

impl Encoding {
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "br" | "brotli" => Some(Encoding::Brotli),
            "zstd" => Some(Encoding::Zstd),
            "gzip" | "x-gzip" => Some(Encoding::Gzip),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Encoding::Brotli => "br",
            Encoding::Zstd => "zstd",
            Encoding::Gzip => "gzip",
        }
    }
}

pub fn choose(accept: &str, allowed: &[Encoding]) -> Option<Encoding> {
    let mut best: Option<(f32, usize, Encoding)> = None;
    let mut wildcard: Option<f32> = None;
    let mut seen = Vec::new();
    for part in accept.split(',') {
        let mut pieces = part.split(';');
        let name = pieces
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let quality = pieces
            .find_map(|piece| {
                piece
                    .trim()
                    .strip_prefix("q=")
                    .map(|value| value.trim().parse::<f32>().unwrap_or(0.0))
            })
            .unwrap_or(1.0);
        if name == "*" {
            wildcard = Some(quality);
            continue;
        }
        let Some(encoding) = Encoding::parse(&name) else {
            continue;
        };
        seen.push(encoding);
        let Some(rank) = allowed.iter().position(|candidate| *candidate == encoding) else {
            continue;
        };
        if quality <= 0.0 {
            continue;
        }
        if best.is_none_or(|(best_quality, best_rank, _)| {
            quality > best_quality || (quality == best_quality && rank < best_rank)
        }) {
            best = Some((quality, rank, encoding));
        }
    }
    if best.is_none()
        && let Some(quality) = wildcard
        && quality > 0.0
    {
        return allowed
            .iter()
            .copied()
            .find(|encoding| !seen.contains(encoding));
    }
    best.map(|(_, _, encoding)| encoding)
}

pub fn compressible(content_type: &str) -> bool {
    let essence = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    essence.starts_with("text/")
        || essence.ends_with("+json")
        || essence.ends_with("+xml")
        || matches!(
            essence.as_str(),
            "application/json"
                | "application/javascript"
                | "application/xml"
                | "application/wasm"
                | "application/x-www-form-urlencoded"
                | "image/svg+xml"
        )
}

pub fn compress(encoding: Encoding, data: &[u8], level: Option<u32>) -> std::io::Result<Vec<u8>> {
    match encoding {
        Encoding::Brotli => {
            let mut output = Vec::new();
            {
                let mut writer = brotli::CompressorWriter::new(
                    &mut output,
                    4096,
                    level.unwrap_or(5).min(11),
                    22,
                );
                writer.write_all(data)?;
            }
            Ok(output)
        }
        Encoding::Zstd => zstd::bulk::compress(data, level.unwrap_or(3).clamp(1, 22) as i32),
        Encoding::Gzip => {
            let mut writer = flate2::write::GzEncoder::new(
                Vec::new(),
                flate2::Compression::new(level.unwrap_or(6).min(9)),
            );
            writer.write_all(data)?;
            writer.finish()
        }
    }
}

pub fn decompress(
    content_encoding: &str,
    data: Vec<u8>,
    max_size: usize,
) -> Result<Vec<u8>, String> {
    let mut current = data;
    for name in content_encoding.split(',').rev() {
        let name = name.trim().to_ascii_lowercase();
        let reader: Box<dyn Read + '_> = match name.as_str() {
            "" | "identity" => continue,
            "gzip" | "x-gzip" => Box::new(flate2::read::MultiGzDecoder::new(current.as_slice())),
            "deflate" if current.first().is_some_and(|byte| byte & 0x0f == 8) => {
                Box::new(flate2::read::ZlibDecoder::new(current.as_slice()))
            }
            "deflate" => Box::new(flate2::read::DeflateDecoder::new(current.as_slice())),
            "br" => Box::new(brotli::Decompressor::new(current.as_slice(), 4096)),
            "zstd" => Box::new(
                zstd::stream::read::Decoder::new(current.as_slice())
                    .map_err(|error| format!("invalid zstd response: {error}"))?,
            ),
            other => return Err(format!("unsupported content encoding '{other}'")),
        };
        let mut output = Vec::new();
        reader
            .take(max_size as u64 + 1)
            .read_to_end(&mut output)
            .map_err(|error| format!("cannot decode the {name} response: {error}"))?;
        if output.len() > max_size {
            return Err(format!(
                "the decoded response is larger than {max_size} bytes"
            ));
        }
        current = output;
    }
    Ok(current)
}
