// Editing a picture by describing the change.
//
// Two jobs that honest arithmetic on pixels cannot do: repaint part of a picture
// from a description, and enlarge one past what the file contains. Everything
// else in this program is arithmetic and stays arithmetic.
//
// Two providers, both bring-your-own-key:
//
//   OpenRouter  POST /api/v1/images       JSON, the source as a data URL in
//                                         `input_references`
//   OpenAI      POST /v1/images/edits     multipart form data
//
// Anthropic is deliberately absent: its models read pictures but do not produce
// them, so there is no endpoint to call.
//
// The key belongs to the person and never leaves this machine except to the
// provider it belongs to; see `secrets`.

use image::RgbaImage;

pub const OPENROUTER: &str = "openrouter";
pub const OPENAI: &str = "openai";

pub const OPENROUTER_BASE: &str = "https://openrouter.ai/api/v1";
pub const OPENAI_BASE: &str = "https://api.openai.com/v1";

/// Sent by OpenRouter's convention, so usage is attributable to this program.
/// Neither line is a secret and neither identifies the person.
const REFERER: &str = "https://github.com/GiovanniCst/Cutaway";
const TITLE: &str = "Cutaway";

/// Image models cap their input anyway, and a smaller upload is faster and
/// cheaper. The answer comes back at whatever size the model chooses, which is
/// usually not the size that went in - the caller is told which.
const MAX_UPLOAD_EDGE: u32 = 2048;

/// Painting a picture is slow compared with a normal request, and a timeout
/// that fires early costs the person money they have already spent.
const PATIENCE: std::time::Duration = std::time::Duration::from_secs(240);

/// Asked for a bigger picture, an image model is as willing to paint a wider
/// scene as to resolve the one it was given. Every clause here forbids that.
pub const UPSCALE_PROMPT: &str = concat!(
    "Upscale this exact image to a higher resolution. Reconstruct the fine detail ",
    "and sharp edges that were lost, as a photo restoration would. ",
    "Keep the framing identical: do not extend or continue the scene, do not add ",
    "anything beyond the original borders, do not zoom out, do not add margins or ",
    "padding. Every element must remain in the same relative position and at the ",
    "same relative size, and nothing new may appear. Change nothing about the ",
    "content, the colours or the composition - only the resolution."
);

/// What to ask for, and what to do with what comes back.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Size {
    /// The picture's own proportions, then resized back to its exact pixels.
    Original,
    /// That tier, left as the model returns it. This is the upscale.
    K1,
    K2,
    K4,
}

impl Size {
    pub fn tier(self) -> Option<&'static str> {
        match self {
            Size::Original => None,
            Size::K1 => Some("1K"),
            Size::K2 => Some("2K"),
            Size::K4 => Some("4K"),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Size::Original => "Dimensione originale",
            Size::K1 => "1K",
            Size::K2 => "2K",
            Size::K4 => "4K",
        }
    }
}

/// The result, and everything worth telling the person about how it was got.
pub struct Outcome {
    pub picture: RgbaImage,
    /// What the call cost, when the provider says so. OpenRouter's usage block
    /// carries the whole bill, the prompt side included; OpenAI's has no
    /// dollars in it at all, and None is the honest answer there.
    pub usd: Option<f64>,
    pub seconds: f64,
    /// What was asked for and what arrived, so a surprise is explainable.
    pub aspect: Option<String>,
    pub resolution: Option<String>,
    /// True when the model refused the size tier and it was dropped.
    pub resolution_refused: bool,
    /// The size the model answered at, before any resize of ours.
    pub returned: (u32, u32),
}

// --- choosing what to ask for ------------------------------------------------

/// The listed ratio nearest the picture's own.
///
/// Left to itself a model answers at a shape of its own choosing, and the only
/// thing that stops a 4:3 photo coming back square is asking for the closest
/// ratio it does offer.
pub fn closest_aspect(width: u32, height: u32, available: &[String]) -> Option<String> {
    if available.is_empty() || width == 0 || height == 0 {
        return None;
    }
    let target = (width as f64 / height as f64).ln();
    let mut best: Option<(f64, &String)> = None;
    for candidate in available {
        let Some((left, right)) = candidate.split_once(':') else { continue };
        let (Ok(left), Ok(right)) = (left.trim().parse::<f64>(), right.trim().parse::<f64>())
        else {
            continue;
        };
        if left <= 0.0 || right <= 0.0 {
            continue;
        }
        // Compared in log space, so 2:1 and 1:2 sit the same distance from 1:1.
        let distance = ((left / right).ln() - target).abs();
        if best.as_ref().map_or(true, |(closest, _)| distance < *closest) {
            best = Some((distance, candidate));
        }
    }
    best.map(|(_, name)| name.clone())
}

/// The tier nearest the picture's own size, so a plain edit does not silently
/// upscale it.
pub fn closest_resolution(longest_edge: u32, available: &[String]) -> Option<String> {
    let pixels = |tier: &str| -> Option<u32> {
        match tier {
            "512" => Some(512),
            "1K" => Some(1024),
            "2K" => Some(2048),
            "4K" => Some(4096),
            _ => None,
        }
    };
    available
        .iter()
        .filter_map(|tier| pixels(tier).map(|size| (size, tier)))
        .min_by_key(|(size, _)| size.abs_diff(longest_edge))
        .map(|(_, tier)| tier.clone())
}

/// PNG bytes of the picture, scaled down to what the models accept.
fn prepare(picture: &RgbaImage) -> Result<Vec<u8>, String> {
    let longest = picture.width().max(picture.height());
    let sent = if longest > MAX_UPLOAD_EDGE {
        let scale = MAX_UPLOAD_EDGE as f32 / longest as f32;
        image::imageops::resize(
            picture,
            ((picture.width() as f32 * scale).round() as u32).max(1),
            ((picture.height() as f32 * scale).round() as u32).max(1),
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        picture.clone()
    };
    // PNG because OpenAI's edit endpoint requires it, and because it is lossless:
    // sending a model a JPEG's artefacts asks it to reproduce them.
    crate::save::encode(&sent, crate::save::Format::Png, 100)
}

// --- the call ----------------------------------------------------------------

/// Sends the picture and the instruction; returns the result and what was asked.
///
/// Two things keep the shape: the request carries the closest aspect ratio the
/// model offers, and in `Size::Original` the answer is resized back to the exact
/// pixels that went in. The tiers are coarse and the ratios a finite list, so
/// the answer lands close but rarely exact - asking for the right proportions
/// first is what keeps that final step a resize rather than a distortion.
pub fn edit(
    provider: &str,
    model: &str,
    prompt: &str,
    picture: &RgbaImage,
    aspect_ratios: &[String],
    resolutions: &[String],
    size: Size,
) -> Result<Outcome, String> {
    if prompt.trim().is_empty() {
        return Err(crate::words::w().write_what_to_change.into());
    }
    let key = crate::secrets::load_key(provider)
        .ok_or(crate::words::w().no_key_stored)?;

    let clock = std::time::Instant::now();
    let png = prepare(picture)?;
    let aspect = closest_aspect(picture.width(), picture.height(), aspect_ratios);
    let resolution = match size.tier() {
        // An upscale asks for the tier by name, and only if the model offers it.
        Some(tier) => resolutions.iter().find(|value| *value == tier).cloned(),
        None => closest_resolution(picture.width().max(picture.height()), resolutions),
    };

    let (payload, resolution, resolution_refused) = match provider {
        OPENAI => (through_openai(&key, model, &png, prompt)?, None, false),
        OPENROUTER => match through_openrouter(&key, model, &png, prompt, &aspect, &resolution) {
            Ok(payload) => (payload, resolution, false),
            Err(exc) => {
                // Some models enforce a minimum output size they never declare -
                // Seedream 4.5 wants 3.7 megapixels and rejects "1K" outright.
                // The provider says so in the error, so the tier is dropped and
                // the aspect ratio kept, which is the half that stops the
                // distortion.
                let refused = resolution.is_some() && exc.to_lowercase().contains("resolution");
                if !refused {
                    return Err(exc);
                }
                (
                    through_openrouter(&key, model, &png, prompt, &aspect, &None)?,
                    None,
                    true,
                )
            }
        },
        other => return Err(crate::words::fill(crate::words::w().unknown_provider, &[other])),
    };

    let usd = payload["usage"]["cost"].as_f64();
    let encoded = payload["data"][0]["b64_json"]
        .as_str()
        .ok_or(crate::words::w().provider_returned_nothing)?;
    let bytes = decode_base64(encoded).ok_or(crate::words::w().picture_unreadable)?;
    let mut result = image::load_from_memory(&bytes)
        .map_err(|exc| crate::words::fill(crate::words::w().undecodable, &[&exc.to_string()]))?
        .to_rgba8();
    let returned = result.dimensions();

    if size == Size::Original && returned != picture.dimensions() {
        result = image::imageops::resize(
            &result,
            picture.width(),
            picture.height(),
            image::imageops::FilterType::Lanczos3,
        );
    }

    Ok(Outcome {
        picture: result,
        usd,
        seconds: clock.elapsed().as_secs_f64(),
        aspect,
        resolution,
        resolution_refused,
        returned,
    })
}

/// Asks the model for a larger version of the same picture.
///
/// This is a generative upscale: the model redraws at the higher resolution
/// rather than interpolating, so it invents detail that was never in the file.
/// Fine for a screenshot or a small logo, not a way to recover something that
/// was never photographed.
pub fn upscale(
    provider: &str,
    model: &str,
    picture: &RgbaImage,
    aspect_ratios: &[String],
    resolutions: &[String],
    target: Size,
) -> Result<Outcome, String> {
    edit(provider, model, UPSCALE_PROMPT, picture, aspect_ratios, resolutions, target)
}

fn through_openrouter(
    key: &str,
    model: &str,
    png: &[u8],
    prompt: &str,
    aspect: &Option<String>,
    resolution: &Option<String>,
) -> Result<serde_json::Value, String> {
    let data_url = format!("data:image/png;base64,{}", crate::secrets::base64(png));
    let mut body = serde_json::json!({
        "model": model,
        "prompt": prompt,
        "input_references": [{"type": "image_url", "image_url": {"url": data_url}}],
    });
    // Only send what this model said it accepts: an unsupported parameter is a
    // 400, not something quietly ignored.
    if let Some(aspect) = aspect {
        body["aspect_ratio"] = serde_json::Value::from(aspect.as_str());
    }
    if let Some(resolution) = resolution {
        body["resolution"] = serde_json::Value::from(resolution.as_str());
    }

    ureq::post(&format!("{}/images", OPENROUTER_BASE))
        .timeout(PATIENCE)
        .set("Authorization", &format!("Bearer {}", key))
        .set("HTTP-Referer", REFERER)
        .set("X-Title", TITLE)
        .send_json(body)
        .map_err(explain)?
        .into_json()
        .map_err(|exc| exc.to_string())
}

fn through_openai(
    key: &str,
    model: &str,
    png: &[u8],
    prompt: &str,
) -> Result<serde_json::Value, String> {
    let (body, content_type) =
        multipart(&[("model", model), ("prompt", prompt), ("n", "1")], "image", "image.png", png);
    ureq::post(&format!("{}/images/edits", OPENAI_BASE))
        .timeout(PATIENCE)
        .set("Authorization", &format!("Bearer {}", key))
        .set("Content-Type", &content_type)
        .send_bytes(&body)
        .map_err(explain)?
        .into_json()
        .map_err(|exc| exc.to_string())
}

/// A multipart body: some text fields and one file.
fn multipart(
    fields: &[(&str, &str)],
    file_field: &str,
    filename: &str,
    file: &[u8],
) -> (Vec<u8>, String) {
    // Unique enough: the boundary only has to not occur inside the body, and a
    // PNG carrying this exact string next to a CRLF is not something that
    // happens by accident.
    let boundary = format!(
        "----cutaway{:x}{:x}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.subsec_nanos())
            .unwrap_or(0)
    );
    let mut body: Vec<u8> = Vec::with_capacity(file.len() + 512);
    for (name, value) in fields {
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{}\"\r\n\r\n", name).as_bytes(),
        );
        body.extend_from_slice(value.as_bytes());
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n",
            file_field, filename
        )
        .as_bytes(),
    );
    body.extend_from_slice(b"Content-Type: image/png\r\n\r\n");
    body.extend_from_slice(file);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());
    (body, format!("multipart/form-data; boundary={}", boundary))
}

// --- reading the answer, and the errors --------------------------------------

pub fn decode_base64(text: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for ch in text.bytes() {
        if ch == b'=' || ch.is_ascii_whitespace() {
            continue;
        }
        let value = ALPHABET.iter().position(|c| *c == ch)? as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}

/// Turns a transport error into something worth reading.
///
/// The provider's own message is the useful part: "insufficient credit" and
/// "this model does not exist" are both a 400, and telling the person only the
/// number sends them looking in the wrong place. It is also what the retry
/// above reads to recognise a refused size tier.
pub fn explain(error: ureq::Error) -> String {
    match error {
        ureq::Error::Status(code, reply) => {
            let said = reply
                .into_json::<serde_json::Value>()
                .ok()
                .and_then(|json| {
                    json["error"]["message"]
                        .as_str()
                        .map(str::to_string)
                        .or_else(|| json["error"].as_str().map(str::to_string))
                })
                .unwrap_or_default();
            if said.is_empty() {
                let w = crate::words::w();
                match code {
                    401 => w.key_not_accepted.to_string(),
                    402 => w.not_enough_credit.to_string(),
                    429 => w.too_many_requests.to_string(),
                    _ => crate::words::fill(w.provider_answered, &[&code.to_string()]),
                }
            } else {
                format!("{} ({})", said, code)
            }
        }
        ureq::Error::Transport(transport) => crate::words::fill(
            crate::words::w().could_not_reach_provider,
            &[&transport.to_string()],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ratios() -> Vec<String> {
        ["1:1", "16:9", "9:16", "4:3", "3:4"].iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_closest_ratio_is_the_one_asked_for() {
        assert_eq!(closest_aspect(1600, 900, &ratios()).as_deref(), Some("16:9"));
        assert_eq!(closest_aspect(1000, 1000, &ratios()).as_deref(), Some("1:1"));
        assert_eq!(closest_aspect(800, 600, &ratios()).as_deref(), Some("4:3"));
        // And a portrait picture must not come back landscape.
        assert_eq!(closest_aspect(600, 800, &ratios()).as_deref(), Some("3:4"));
    }

    #[test]
    fn a_ratio_is_measured_in_log_space() {
        // 2:1 and 1:2 are equally far from square, which is only true in logs.
        let two = ["2:1".to_string(), "1:2".to_string()];
        assert_eq!(closest_aspect(1000, 1000, &two).as_deref(), Some("2:1"));
        // A picture twice as wide as tall picks the wide one, not the tall one.
        assert_eq!(closest_aspect(2000, 1000, &two).as_deref(), Some("2:1"));
        assert_eq!(closest_aspect(1000, 2000, &two).as_deref(), Some("1:2"));
    }

    #[test]
    fn nonsense_ratios_are_skipped_rather_than_crashing() {
        let broken = ["nonsense".to_string(), "1:0".to_string(), "4:3".to_string()];
        assert_eq!(closest_aspect(800, 600, &broken).as_deref(), Some("4:3"));
        assert_eq!(closest_aspect(800, 600, &[]), None);
    }

    #[test]
    fn a_plain_edit_does_not_silently_upscale() {
        let tiers: Vec<String> = ["512", "1K", "2K", "4K"].iter().map(|s| s.to_string()).collect();
        // A 900 px picture belongs at 1K, not at 4K.
        assert_eq!(closest_resolution(900, &tiers).as_deref(), Some("1K"));
        assert_eq!(closest_resolution(500, &tiers).as_deref(), Some("512"));
        // Nearest, not largest: 3000 is 952 from 2048 and 1096 from 4096.
        assert_eq!(closest_resolution(3000, &tiers).as_deref(), Some("2K"));
        assert_eq!(closest_resolution(3900, &tiers).as_deref(), Some("4K"));
        // A model that lists no tiers gets asked for none.
        assert_eq!(closest_resolution(900, &[]), None);
    }

    #[test]
    fn a_big_picture_is_shrunk_before_being_sent() {
        let large = RgbaImage::from_pixel(4000, 2000, image::Rgba([10, 20, 30, 255]));
        let png = prepare(&large).expect("codificata");
        let back = image::load_from_memory(&png).expect("rileggibile");
        assert_eq!(back.width().max(back.height()), MAX_UPLOAD_EDGE);
        // And the proportions survive, or the model repaints a stretched picture.
        assert_eq!(back.width() / 2, back.height());
    }

    #[test]
    fn a_small_picture_is_sent_as_it_is() {
        let small = RgbaImage::from_pixel(300, 200, image::Rgba([10, 20, 30, 255]));
        let png = prepare(&small).expect("codificata");
        let back = image::load_from_memory(&png).expect("rileggibile");
        assert_eq!((back.width(), back.height()), (300, 200));
    }

    #[test]
    fn base64_survives_the_round_trip() {
        let bytes: Vec<u8> = (0..200u32).map(|i| (i * 7 % 251) as u8).collect();
        let text = crate::secrets::base64(&bytes);
        assert_eq!(decode_base64(&text).as_deref(), Some(bytes.as_slice()));
    }

    #[test]
    fn a_multipart_body_carries_both_halves() {
        let (body, content_type) =
            multipart(&[("model", "gpt-image-2")], "image", "image.png", b"\x89PNG-fake");
        let boundary = content_type.split("boundary=").nth(1).expect("boundary");
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("name=\"model\"\r\n\r\ngpt-image-2"));
        assert!(text.contains("filename=\"image.png\""));
        assert!(text.contains("PNG-fake"));
        // And it is closed, or the server waits for the rest of it forever.
        assert!(text.ends_with(&format!("--{}--\r\n", boundary)));
    }

    #[test]
    fn an_upscale_asks_for_the_tier_by_name() {
        assert_eq!(Size::K4.tier(), Some("4K"));
        assert_eq!(Size::Original.tier(), None);
    }
}
