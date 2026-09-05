// The shortlist of AI models: derived from the boards, refreshable from the app.
//
// Two sources, each for the one thing it is good at:
//
//   quality  LMArena's image *editing* arena, read through HuggingFace's
//            datasets-server. No key, no quota, no attribution clause. It gives
//            a rating, a confidence interval and - the part that matters most -
//            a vote count, so a model rated on a handful of votes cannot
//            leapfrog on noise.
//
//   reach    OpenRouter's image catalogue: whether this account can use a model
//            at all, whether it can *edit* rather than only generate, and the
//            price.
//
// The rules are two, and short. Frontier: the highest lower bounds of the arena
// rating. Economical: within MAX_ELO_GAP of the best, the cheapest first -
// quality decides who qualifies, price decides the order.
//
// `derive()` builds that list from the live boards. `refresh()` is what the
// update button calls: derive, carry over what only a person can know (`seconds`
// is measured on a real edit, no board carries it), and store the result beside
// the settings. `shortlist()` is what the menu shows: the last stored refresh,
// or the curated seed below when nobody has refreshed yet.
//
// The user's own edits keep the list honest: every plain edit records what it
// cost and how long it took, and the menu shows the median of the last five over
// anything derived - measured first, derived as the fallback, an honest blank
// when there is neither.

use std::collections::BTreeMap;
use std::path::PathBuf;

const FRONTIER_COUNT: usize = 3;
const ECONOMICAL_COUNT: usize = 3;

/// An economical model still has to be near the state of the art, or the list
/// stops being a shortlist and becomes a price comparison. If fewer than three
/// qualify, fewer are offered: two good ones beat three with a bad third.
const MAX_ELO_GAP: f64 = 120.0;

/// Below this the rating is not evidence, however good the number looks.
const MIN_VOTES: i64 = 1000;

/// Costs are compared as one edit at one size, because otherwise they are not
/// comparable at all: the catalogue prices some models per image, some per
/// megapixel and some per token.
const TARGET_MEGAPIXELS: f64 = 1024.0 * 1024.0 / 1_000_000.0;

/// The plain /rows endpoint, not /filter: the split is under a hundred rows, so
/// the category is picked out and the rank sorted here in two lines, and /filter
/// is the flakier of the two server paths.
const ARENA_URL: &str = concat!(
    "https://datasets-server.huggingface.co/rows",
    "?dataset=lmarena-ai%2Fleaderboard-dataset",
    "&config=image_edit", // not text_to_image: this program edits pictures
    "&split=latest",
    "&offset=0&length=100"
);

fn catalogue_url() -> String {
    format!("{}/images/models", crate::ai::OPENROUTER_BASE)
}

fn endpoints_url(model_id: &str) -> String {
    format!("{}/images/models/{}/endpoints", crate::ai::OPENROUTER_BASE, model_id)
}

/// How many output tokens one 1024x1024 picture costs, for the families that
/// price by the token. Read from the vendor's own pricing page and cross-checked
/// against what OpenRouter charges per token: Google lists 1120 tokens and
/// $0.067 an image for Gemini 3.1 Flash, and 1120 x $0.00006 is $0.0672. Two
/// sources agreeing is the only reason these numbers are here.
///
/// OpenAI and Microsoft are deliberately absent. OpenAI's pricing page gives a
/// per-token rate and sends you to a calculator for the count; Microsoft's MAI
/// publishes no count at all. A number nobody published is a number this module
/// will not invent: those models come out with no cost, which keeps them out of
/// the economical ranking and leaves them eligible for the frontier one, where
/// cost plays no part.
const IMAGE_TOKENS_AT_1K: &[(&str, f64)] = &[
    ("google/gemini-3.1-flash-image", 1120.0),
    ("google/gemini-3.1-flash-lite-image", 1120.0),
    ("google/gemini-3-pro-image", 1120.0),
    ("google/gemini-3-pro-image-preview", 1120.0),
    ("google/gemini-2.5-flash-image", 1290.0),
];

/// Arena names that no amount of normalising will turn into a catalogue id.
/// Every entry is a claim that two names are the same model, and a wrong claim
/// here puts one model's rating on another - so the list is short and explicit,
/// and a refresh reports the ones that stop resolving.
const ALIASES: &[(&str, &str)] = &[
    ("gemini-3-pro-image-2k", "google/gemini-3-pro-image"),
    ("qwen-image-3.0-pro", "qwen/qwen-image-3-pro"),
    ("seedream-5.0-pro", "bytedance-seed/seedream-5-0-pro"),
    ("seedream-4.5", "bytedance-seed/seedream-4.5"),
];

/// What the arena calls a lab versus what a person calls it. Cosmetic only, and
/// anything not listed keeps the arena's own spelling.
const VENDORS: &[(&str, &str)] = &[
    ("openai", "OpenAI"),
    ("xai", "xAI"),
    ("x-ai", "xAI"),
    ("google", "Google"),
    ("microsoft-ai", "Microsoft"),
    ("microsoft", "Microsoft"),
    ("bytedance", "ByteDance"),
    ("bytedance-seed", "ByteDance"),
    ("alibaba", "Alibaba"),
    ("qwen", "Alibaba"),
];

/// The tiers the arena names in brackets, as distinct from a codename: the board
/// writes both `gpt-image-2 (medium)` and `gemini-3.1-flash-image (nano-banana-2)`,
/// and only the first is a variant with its own price.
const ARENA_TIERS: &[&str] = &["low", "medium", "high", "standard"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tier {
    /// The frontier list: the best evidence of quality, cost no object.
    Top,
    /// The economical list: near the top, cheapest first.
    Value,
    /// The rest of the catalogue: reachable, but nobody has weighed it.
    Unweighed,
}

impl Tier {
    fn as_str(self) -> &'static str {
        match self {
            Tier::Top => "top",
            Tier::Value => "value",
            Tier::Unweighed => "",
        }
    }

    fn from_str(text: &str) -> Tier {
        match text {
            "top" => Tier::Top,
            "value" => Tier::Value,
            _ => Tier::Unweighed,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tier::Top => crate::words::w().tier_top,
            Tier::Value => crate::words::w().tier_value,
            Tier::Unweighed => crate::words::w().tier_unweighed,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub tier: Tier,
    pub elo: Option<u32>,
    pub rank: Option<u32>,
    /// What one edit costs. None is honest and means "no comparable price",
    /// which is not the same as free - the expensive models are also the ones
    /// whose price cannot be put on this scale.
    pub usd: Option<f64>,
    /// Measured on a real edit here, never derived.
    pub seconds: Option<f64>,
    pub aspect_ratios: Vec<String>,
    pub resolutions: Vec<String>,
}

impl Model {
    fn bare(id: &str, name: &str) -> Model {
        Model {
            id: id.to_string(),
            name: name.to_string(),
            vendor: String::new(),
            tier: Tier::Unweighed,
            elo: None,
            rank: None,
            usd: None,
            seconds: None,
            aspect_ratios: Vec::new(),
            resolutions: Vec::new(),
        }
    }
}

/// The seed: what the menu shows before anybody has pressed update. Curated by
/// hand from the boards as they stood on CURATED_AT; a refresh replaces it in
/// the store, never in this file.
///
/// GPT Image 2 stays despite being beaten by MAI-Image 2.5 Pro on both quality
/// and price, because it answers in nine seconds against twenty-three. Speed is
/// the third axis, and on an interactive tool it is not a small one.
pub const CURATED_AT: &str = "2026-08-30";

/// id, name, vendor, tier, elo, rank, usd, seconds
const CURATED: &[(&str, &str, &str, Tier, u32, u32, f64, f64)] = &[
    ("microsoft/mai-image-2.5", "MAI-Image 2.5", "Microsoft", Tier::Value, 1257, 5, 0.048, 22.0),
    (
        "microsoft/mai-image-2.5-pro",
        "MAI-Image 2.5 Pro",
        "Microsoft",
        Tier::Top,
        1272,
        2,
        0.109,
        23.0,
    ),
    ("openai/gpt-image-2", "GPT Image 2", "OpenAI", Tier::Top, 1259, 4, 0.211, 9.0),
    ("google/gemini-3.1-flash-image", "Nano Banana 2", "Google", Tier::Top, 1251, 7, 0.067, 10.0),
    ("qwen/qwen-image-3-pro", "Qwen Image 3 Pro", "Alibaba", Tier::Value, 1244, 10, 0.043, 48.0),
    ("qwen/qwen-image-3", "Qwen Image 3", "Alibaba", Tier::Value, 1218, 16, 0.033, 51.0),
];

fn seed() -> Vec<Model> {
    CURATED
        .iter()
        .map(|(id, name, vendor, tier, elo, rank, usd, seconds)| Model {
            id: id.to_string(),
            name: name.to_string(),
            vendor: vendor.to_string(),
            tier: *tier,
            elo: Some(*elo),
            rank: Some(*rank),
            usd: Some(*usd),
            seconds: Some(*seconds),
            aspect_ratios: Vec::new(),
            resolutions: Vec::new(),
        })
        .collect()
}

// --- the store ---------------------------------------------------------------

fn store_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("Cutaway")
}

fn store_path() -> PathBuf {
    store_dir().join("models.json")
}

fn measurements_path() -> PathBuf {
    store_dir().join("measurements.json")
}

fn read_json(path: &PathBuf) -> Option<serde_json::Value> {
    // A file that cannot be read counts as no refresh: this is a cache of public
    // data, not a document, and the seed is always there behind it.
    crate::settings::read_json(path)
}

fn model_from_json(entry: &serde_json::Value) -> Option<Model> {
    let id = entry["id"].as_str()?;
    Some(Model {
        id: id.to_string(),
        name: entry["name"].as_str().unwrap_or(id).to_string(),
        vendor: entry["vendor"].as_str().unwrap_or_default().to_string(),
        tier: Tier::from_str(entry["tier"].as_str().unwrap_or_default()),
        elo: entry["elo"].as_f64().map(|value| value.round() as u32),
        rank: entry["rank"].as_u64().map(|value| value as u32),
        usd: entry["usd"].as_f64(),
        seconds: entry["seconds"].as_f64(),
        aspect_ratios: Vec::new(),
        resolutions: Vec::new(),
    })
}

fn model_to_json(model: &Model) -> serde_json::Value {
    serde_json::json!({
        "id": model.id,
        "name": model.name,
        "vendor": model.vendor,
        "tier": model.tier.as_str(),
        "elo": model.elo,
        "rank": model.rank,
        "usd": model.usd,
        "seconds": model.seconds,
    })
}

/// What the menu shows, and the day it was true: the stored refresh, else the seed.
pub fn shortlist() -> (Vec<Model>, String) {
    let Some(stored) = read_json(&store_path()) else { return (seed(), CURATED_AT.to_string()) };
    let (Some(entries), Some(at)) = (stored["models"].as_array(), stored["generated_at"].as_str())
    else {
        return (seed(), CURATED_AT.to_string());
    };
    let models: Vec<Model> = entries.iter().filter_map(model_from_json).collect();
    if models.is_empty() || at.is_empty() {
        return (seed(), CURATED_AT.to_string());
    }
    (models, at.to_string())
}

// --- what the user's own edits keep teaching the list ------------------------

const MEASUREMENTS_KEEP: usize = 5;

/// Writes down one real edit: the cost the response declared, the seconds the
/// clock saw, and the longest edge of what came back.
///
/// The size travels with the numbers because cost and time both scale with it.
/// Nothing is normalised to a canonical size: the menu reports what the model
/// cost and took on the edits actually run here.
///
/// The last MEASUREMENTS_KEEP per model are kept and the menu shows their
/// median, so one odd edit does not move the figure while a real change works
/// its way in within a handful of edits. A failed write is swallowed: the edit
/// itself succeeded, and losing the note is not worth an error message.
pub fn record(model_id: &str, seconds: Option<f64>, usd: Option<f64>, px: Option<u32>) {
    if seconds.is_none() && usd.is_none() {
        return;
    }
    let mut entry = serde_json::json!({ "at": crate::clock::today() });
    if let Some(seconds) = seconds {
        entry["seconds"] = serde_json::json!((seconds * 10.0).round() / 10.0);
    }
    if let Some(usd) = usd {
        entry["usd"] = serde_json::json!((usd * 100_000.0).round() / 100_000.0);
    }
    if let Some(px) = px {
        entry["px"] = serde_json::json!(px);
    }

    let mut all = match read_json(&measurements_path()) {
        Some(serde_json::Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    };
    let mut rows = match all.get(model_id) {
        Some(serde_json::Value::Array(rows)) => rows.clone(),
        _ => Vec::new(),
    };
    while rows.len() >= MEASUREMENTS_KEEP {
        rows.remove(0);
    }
    rows.push(entry);
    all.insert(model_id.to_string(), serde_json::Value::Array(rows));

    let path = measurements_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(&serde_json::Value::Object(all)) {
        let _ = std::fs::write(&path, text);
    }
}

fn median(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        Some(values[middle])
    } else {
        Some((values[middle - 1] + values[middle]) / 2.0)
    }
}

/// A measured edit beats a derived figure.
///
/// The derived price only covers the output side of the bill; a measured cost is
/// the whole of it, and the prompt side can be a third of the total. A
/// measurement also cannot go stale, because using the model is what refreshes
/// it. The median of the kept edits is what shows; the derived number stays only
/// for a model nobody here has used yet.
pub fn apply_measurements(models: &mut [Model]) {
    let Some(serde_json::Value::Object(all)) = read_json(&measurements_path()) else { return };
    for model in models.iter_mut() {
        let Some(serde_json::Value::Array(rows)) = all.get(&model.id) else { continue };
        let pick = |field: &str| -> Vec<f64> {
            rows.iter().filter_map(|row| row[field].as_f64()).collect()
        };
        if let Some(seconds) = median(pick("seconds")) {
            model.seconds = Some(seconds.round());
        }
        if let Some(usd) = median(pick("usd")) {
            model.usd = Some((usd * 1000.0).round() / 1000.0);
        }
    }
}

// --- reading the catalogue ---------------------------------------------------

fn get(url: &str, key: Option<&str>) -> Result<serde_json::Value, String> {
    let mut request = ureq::get(url)
        .timeout(std::time::Duration::from_secs(60))
        .set("User-Agent", "cutaway");
    if let Some(key) = key {
        request = request.set("Authorization", &format!("Bearer {}", key));
    }
    request.call().map_err(crate::ai::explain)?.into_json().map_err(|exc| exc.to_string())
}

/// One GET, retried on the kind of failure that goes away by itself.
///
/// Both boards answer 500 now and then, and a run that reads twenty addresses
/// should not be lost to one of them. Only server errors and timeouts are
/// retried: a 401 or a 404 means the same thing however often it is asked.
fn get_patiently(url: &str, key: Option<&str>) -> Result<serde_json::Value, String> {
    let mut last = String::new();
    for attempt in 0..3 {
        let mut request = ureq::get(url)
            .timeout(std::time::Duration::from_secs(90))
            .set("User-Agent", "cutaway");
        if let Some(key) = key {
            request = request.set("Authorization", &format!("Bearer {}", key));
        }
        match request.call() {
            Ok(reply) => return reply.into_json().map_err(|exc| exc.to_string()),
            Err(ureq::Error::Status(code, reply)) if code < 500 => {
                return Err(crate::ai::explain(ureq::Error::Status(code, reply)))
            }
            Err(exc) => last = crate::ai::explain(exc),
        }
        std::thread::sleep(std::time::Duration::from_secs(2 * (attempt + 1)));
    }
    Err(last)
}

/// A model can edit rather than only generate when it takes a picture in *and*
/// accepts a reference image. Both halves are required: the first alone lets a
/// model describe a picture, which is not the same job.
fn can_edit(entry: &serde_json::Value) -> bool {
    let takes_pictures = entry["architecture"]["input_modalities"]
        .as_array()
        .is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str() == Some("image")));
    let references = entry["supported_parameters"]["input_references"]["max"]
        .as_u64()
        .unwrap_or(0);
    takes_pictures && references > 0
}

fn values_at(entry: &serde_json::Value, field: &str) -> Vec<String> {
    entry["supported_parameters"][field]["values"]
        .as_array()
        .map(|values| {
            values.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()
        })
        .unwrap_or_default()
}

pub struct Catalogue {
    /// The shortlist, filtered to what this account can actually reach.
    pub models: Vec<Model>,
    /// The rest of the catalogue that can edit a picture. A name and the sizes
    /// it accepts, and nothing else: an elo and a price are the work of the
    /// ranking, and inventing them for a model nobody has weighed would be worse
    /// than leaving them out.
    pub others: Vec<Model>,
    /// The day the shortlist was true. OpenAI has no board behind it, so there
    /// it is None.
    pub refreshed_at: Option<String>,
    /// True when the catalogue could not be read and the menu is unverified.
    pub offline: bool,
}

/// What this account can reach: the shortlist, and everything else.
///
/// The catalogue is queried rather than trusted: a model that has been retired,
/// or that this key has no access to, drops out of the menu instead of failing
/// at the moment it is picked.
pub fn list(provider: &str) -> Result<Catalogue, String> {
    let key = crate::secrets::load_key(provider).ok_or(crate::words::w().no_key_stored)?;

    if provider == crate::ai::OPENAI {
        let ids = match get(&format!("{}/models", crate::ai::OPENAI_BASE), Some(&key)) {
            Ok(payload) => payload["data"]
                .as_array()
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(|entry| entry["id"].as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            // Offline: fall back to the one id known to exist, unverified.
            Err(_) => vec!["gpt-image-1".to_string()],
        };
        let (mut models, others) = split_openai(&ids);
        apply_measurements(&mut models);
        return Ok(Catalogue { models, others, refreshed_at: None, offline: false });
    }

    let (mut shortlist, refreshed) = shortlist();
    let payload = match get(&catalogue_url(), Some(&key)) {
        Ok(payload) => payload,
        Err(_) => {
            // Offline or the catalogue is down: better an unverified menu than none.
            apply_measurements(&mut shortlist);
            return Ok(Catalogue {
                models: shortlist,
                others: Vec::new(),
                refreshed_at: Some(refreshed),
                offline: true,
            });
        }
    };

    let mut reachable: BTreeMap<String, (String, Vec<String>, Vec<String>)> = BTreeMap::new();
    for entry in payload["data"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        if !can_edit(entry) {
            continue;
        }
        let Some(id) = entry["id"].as_str() else { continue };
        reachable.insert(
            id.to_string(),
            (
                entry["name"].as_str().unwrap_or(id).to_string(),
                values_at(entry, "aspect_ratio"),
                values_at(entry, "resolution"),
            ),
        );
    }

    let mut models = Vec::new();
    for mut model in shortlist {
        let Some((_, aspect, resolution)) = reachable.get(&model.id) else { continue };
        model.aspect_ratios = aspect.clone();
        model.resolutions = resolution.clone();
        models.push(model);
    }
    let shortlisted: Vec<String> = models.iter().map(|model| model.id.clone()).collect();
    let others = reachable
        .iter()
        .filter(|(id, _)| !shortlisted.contains(id))
        .map(|(id, (name, aspect, resolution))| {
            let mut model = Model::bare(id, name);
            model.aspect_ratios = aspect.clone();
            model.resolutions = resolution.clone();
            model
        })
        .collect();
    apply_measurements(&mut models);
    Ok(Catalogue { models, others, refreshed_at: Some(refreshed), offline: false })
}

fn openai_name(model_id: &str) -> String {
    model_id.replace("gpt-image", "GPT Image").replace('-', " ")
}

/// The version number in `gpt-image-2` / `gpt-image-2-mini`, and whether it is
/// the mini. None for anything that is not exactly one of those two shapes: a
/// dated snapshot belongs behind the search, not in a two-line menu.
fn openai_version(model_id: &str) -> Option<(f64, bool)> {
    let rest = model_id.strip_prefix("gpt-image-")?;
    let (number, mini) = match rest.strip_suffix("-mini") {
        Some(number) => (number, true),
        None => (rest, false),
    };
    if number.is_empty() || !number.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    Some((number.parse().ok()?, mini))
}

/// OpenAI's image family split into a two-model menu and the rest.
///
/// /v1/models lists ids and nothing else - no capability, no price - so the menu
/// is the two sane defaults: the newest full gpt-image model and the newest
/// mini. Dated snapshots and everything else image-shaped stay behind the
/// search, unweighed and said so. No arena covers the direct API, so nothing
/// here wears a rank or a cost: a number with no source is a number that does
/// not appear.
fn split_openai(ids: &[String]) -> (Vec<Model>, Vec<Model>) {
    let mut family: Vec<&String> = ids
        .iter()
        .filter(|id| id.starts_with("gpt-image") || id.starts_with("dall-e"))
        .collect();
    family.sort();

    let newest = |mini: bool| -> Option<&String> {
        family
            .iter()
            .filter_map(|id| openai_version(id).filter(|(_, is_mini)| *is_mini == mini).map(|(v, _)| (v, *id)))
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(_, id)| id)
    };

    let mut models = Vec::new();
    for (id, tier) in [(newest(false), Tier::Top), (newest(true), Tier::Value)] {
        let Some(id) = id else { continue };
        let mut model = Model::bare(id, &openai_name(id));
        model.vendor = "OpenAI".to_string();
        model.tier = tier;
        models.push(model);
    }
    let chosen: Vec<String> = models.iter().map(|model| model.id.clone()).collect();
    let others = family
        .iter()
        .filter(|id| !chosen.contains(id))
        .map(|id| Model::bare(id, &openai_name(id)))
        .collect();
    (models, others)
}

// --- the derivation ----------------------------------------------------------

/// A model name reduced to what two sources are likely to agree on.
///
/// The arena writes `gemini-3.1-flash-image (nano-banana-2)`; the catalogue
/// writes `google/gemini-3.1-flash-image`. The codename in brackets and the
/// punctuation are the noise; the rest has to match exactly, because a partial
/// match is how `gpt-image-1.5-high-fidelity` becomes `gpt-image-1`.
fn normalise(name: &str) -> String {
    name.split('(')
        .next()
        .unwrap_or("")
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// The quality tier the arena rated, when its name carries one.
fn arena_variant(name: &str) -> Option<String> {
    let inside = name.split_once('(')?.1.split(')').next()?.trim().to_lowercase();
    ARENA_TIERS.contains(&inside.as_str()).then_some(inside)
}

/// What one edit costs from this provider, or None if unknown.
///
/// An edit is one 1024x1024 output plus the reference picture sent in: Grok, for
/// one, bills $0.01 for the input on top of $0.04 for the output, and an editor
/// pays both on every call.
///
/// Some models carry one base price, some only per-variant prices ("low_1k",
/// "medium_2k", "high_resolution"). The base entry wins when there is one.
/// Otherwise only 1k-sized entries qualify - the size this comparison is made at
/// - and among them the tier the arena rated, because the rating shown beside
/// this cost belongs to that tier. A price that cannot be put on this scale is
/// None, not a guess: never invented, never averaged.
fn cost_per_edit<'a>(
    pricing: &'a [serde_json::Value],
    model_id: &str,
    variant: Option<&str>,
) -> Option<f64> {
    let outputs: Vec<&'a serde_json::Value> = pricing
        .iter()
        .filter(|entry| entry["billable"].as_str() == Some("output_image"))
        .collect();

    let cheapest = |pool: &[&'a serde_json::Value]| -> Option<&'a serde_json::Value> {
        pool.iter()
            .copied()
            .min_by(|a, b| {
                let (a, b) =
                    (a["cost_usd"].as_f64().unwrap_or(0.0), b["cost_usd"].as_f64().unwrap_or(0.0));
                a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)
            })
    };

    let entry = match outputs.iter().copied().find(|entry| entry["variant"].as_str().is_none()) {
        Some(base) => Some(base),
        None => {
            // Only 1k-sized entries qualify - the size this comparison is made
            // at - and among them the tier the arena rated, because the rating
            // shown beside this cost belongs to that tier.
            let at_1k: Vec<&serde_json::Value> = outputs
                .iter()
                .copied()
                .filter(|entry| entry["variant"].as_str().is_some_and(|name| name.contains("1k")))
                .collect();
            let tiered: Vec<&serde_json::Value> = at_1k
                .iter()
                .copied()
                .filter(|entry| match (variant, entry["variant"].as_str()) {
                    (Some(tier), Some(name)) => name.starts_with(tier),
                    _ => false,
                })
                .collect();
            cheapest(if tiered.is_empty() { &at_1k } else { &tiered })
        }
    }?;

    let cost = entry["cost_usd"].as_f64().unwrap_or(0.0);
    let output = match entry["unit"].as_str() {
        Some("image") => cost,
        Some("megapixel") => cost * TARGET_MEGAPIXELS,
        Some("token") => {
            let tokens = IMAGE_TOKENS_AT_1K
                .iter()
                .find(|(id, _)| *id == model_id)
                .map(|(_, tokens)| *tokens)?;
            cost * tokens
        }
        _ => return None,
    };

    // The reference picture is billed on top, and an editor pays it every call.
    let reference = pricing
        .iter()
        .find(|entry| {
            entry["billable"].as_str() == Some("input_image")
                && entry["unit"].as_str() == Some("image")
        })
        .and_then(|entry| entry["cost_usd"].as_f64())
        .unwrap_or(0.0);
    Some(output + reference)
}

struct Candidate {
    model_id: String,
    name: String,
    organization: String,
    rank: u32,
    rating: f64,
    rating_lower: f64,
    votes: i64,
    cost: Option<f64>,
}

/// What a refresh found, including the failure modes that would otherwise be
/// silent: a model quietly missing from the list is worth seeing.
pub struct Refresh {
    pub models: Vec<Model>,
    pub generated_at: String,
    pub reachable: usize,
    pub board_size: usize,
    /// Best-ranked first, with vote counts: on the board, not in the catalogue.
    pub unmatched: Vec<(u32, String, i64)>,
    /// Rated on too few votes to be evidence.
    pub thin: Vec<String>,
    /// Aliases that no longer resolve. Each one is a model silently dropped.
    pub rotten: Vec<String>,
}

/// Reads both boards and builds the shortlist.
pub fn derive(key: &str) -> Result<Refresh, String> {
    let arena = get_patiently(ARENA_URL, None)
        .map_err(|exc| crate::words::fill(crate::words::w().could_not_read_board, &[&exc]))?;
    let catalogue = get_patiently(&catalogue_url(), Some(key))
        .map_err(|exc| crate::words::fill(crate::words::w().could_not_read_catalogue, &[&exc]))?;

    let mut board: Vec<&serde_json::Value> = arena["rows"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .map(|item| &item["row"])
        .filter(|row| row["category"].as_str() == Some("overall"))
        .collect();
    board.sort_by_key(|row| row["rank"].as_u64().unwrap_or(u64::MAX));

    // Only what this account can actually edit a picture with.
    let mut editors: BTreeMap<String, String> = BTreeMap::new();
    for entry in catalogue["data"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        if !can_edit(entry) {
            continue;
        }
        let Some(id) = entry["id"].as_str() else { continue };
        editors.insert(id.to_string(), entry["name"].as_str().unwrap_or(id).to_string());
    }
    let by_name: BTreeMap<String, String> = editors
        .keys()
        .map(|id| (normalise(id.rsplit('/').next().unwrap_or(id)), id.clone()))
        .collect();

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut unmatched = Vec::new();
    let mut thin = Vec::new();
    let rotten: Vec<String> = ALIASES
        .iter()
        .filter(|(_, id)| !editors.contains_key(*id))
        .map(|(name, _)| name.to_string())
        .collect();

    for row in &board {
        let votes = row["vote_count"].as_i64().unwrap_or(0);
        let arena_name = row["model_name"].as_str().unwrap_or_default().to_string();
        let rank = row["rank"].as_u64().unwrap_or(0) as u32;
        if votes < MIN_VOTES {
            thin.push(arena_name);
            continue;
        }

        let base = arena_name.split('(').next().unwrap_or("").trim().to_string();
        let model_id = ALIASES
            .iter()
            .find(|(name, _)| *name == base)
            .map(|(_, id)| id.to_string())
            .or_else(|| by_name.get(&normalise(&base)).cloned());
        let Some(model_id) = model_id.filter(|id| editors.contains_key(id)) else {
            unmatched.push((rank, arena_name, votes));
            continue;
        };
        // The board rates several variants of the same model; the best-ranked won.
        if candidates.iter().any(|found| found.model_id == model_id) {
            continue;
        }

        let Ok(endpoints) = get_patiently(&endpoints_url(&model_id), Some(key)) else {
            unmatched.push((rank, arena_name, votes));
            continue;
        };
        let tier = arena_variant(&arena_name);
        let costs: Vec<f64> = endpoints["endpoints"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .filter_map(|provider| {
                let pricing = provider["pricing"].as_array().map(Vec::as_slice).unwrap_or(&[]);
                cost_per_edit(pricing, &model_id, tier.as_deref())
            })
            .collect();

        candidates.push(Candidate {
            name: editors.get(&model_id).cloned().unwrap_or_else(|| model_id.clone()),
            organization: row["organization"].as_str().unwrap_or_default().to_string(),
            rank,
            rating: row["rating"].as_f64().unwrap_or(0.0),
            rating_lower: row["rating_lower"].as_f64().unwrap_or(0.0),
            votes,
            // The cheapest provider serving it, which is what gets paid.
            cost: costs
                .iter()
                .copied()
                .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)),
            model_id,
        });
    }

    if candidates.is_empty() {
        return Err(crate::words::w().nothing_reachable.into());
    }

    // Frontier: the best evidence, not the best number. The lower bound rewards
    // a model for being both well rated and well established, which is why a
    // newcomer with a thousand votes cannot leapfrog on noise.
    let mut order: Vec<usize> = (0..candidates.len()).collect();
    order.sort_by(|a, b| {
        let (a, b) = (&candidates[*a], &candidates[*b]);
        b.rating_lower
            .partial_cmp(&a.rating_lower)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.rating.partial_cmp(&a.rating).unwrap_or(std::cmp::Ordering::Equal))
            .then(b.votes.cmp(&a.votes))
    });
    let frontier: Vec<usize> = order.iter().copied().take(FRONTIER_COUNT).collect();

    // Economical: quality decides who qualifies, price decides the order.
    // Whatever is already in the frontier list is not offered again as a
    // bargain: the two lists exist to widen the choice, and a model in both
    // narrows it.
    let best = candidates
        .iter()
        .map(|model| model.rating)
        .fold(f64::MIN, f64::max);
    let mut economical: Vec<usize> = (0..candidates.len())
        .filter(|i| {
            let model = &candidates[*i];
            model.cost.is_some_and(|cost| cost > 0.0)
                && !frontier.contains(i)
                && model.rating >= best - MAX_ELO_GAP
        })
        .collect();
    economical.sort_by(|a, b| {
        let (a, b) = (&candidates[*a], &candidates[*b]);
        a.cost
            .unwrap_or(f64::MAX)
            .partial_cmp(&b.cost.unwrap_or(f64::MAX))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                b.rating_lower.partial_cmp(&a.rating_lower).unwrap_or(std::cmp::Ordering::Equal),
            )
    });
    economical.truncate(ECONOMICAL_COUNT);

    let entry = |model: &Candidate, tier: Tier| -> Model {
        // The catalogue's display name repeats the lab ("xAI: Grok..."); the
        // vendor has its own column, so the prefix goes.
        let name = model.name.split_once(": ").map(|(_, rest)| rest).unwrap_or(&model.name);
        let organization = model.organization.to_lowercase();
        Model {
            id: model.model_id.clone(),
            name: name.to_string(),
            vendor: VENDORS
                .iter()
                .find(|(key, _)| *key == organization)
                .map(|(_, shown)| shown.to_string())
                .unwrap_or_else(|| model.organization.clone()),
            tier,
            elo: Some(model.rating.round() as u32),
            rank: Some(model.rank),
            usd: model.cost.map(|cost| (cost * 1000.0).round() / 1000.0),
            seconds: None,
            aspect_ratios: Vec::new(),
            resolutions: Vec::new(),
        }
    };

    let models: Vec<Model> = frontier
        .iter()
        .map(|i| entry(&candidates[*i], Tier::Top))
        .chain(economical.iter().map(|i| entry(&candidates[*i], Tier::Value)))
        .collect();

    Ok(Refresh {
        models,
        generated_at: crate::clock::today(),
        reachable: editors.len(),
        board_size: board.len(),
        unmatched,
        thin,
        rotten,
    })
}

/// Derives a fresh shortlist, keeps the hand-measured columns, and stores it.
///
/// When the derivation fails the stored list stays exactly as it was and the
/// menu keeps showing it under its own date: a refresh that did not happen is
/// not an update.
pub fn refresh(key: &str) -> Result<Refresh, String> {
    let mut fresh = derive(key)?;
    // `seconds` is measured on a real edit: no board can derive it, so a model
    // that survives the refresh keeps its measurement and a new arrival shows
    // none until somebody times it, which is the truth.
    let (previous, _) = shortlist();
    for model in fresh.models.iter_mut() {
        if let Some(old) = previous.iter().find(|old| old.id == model.id) {
            model.seconds = old.seconds;
        }
    }

    let path = store_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|exc| exc.to_string())?;
    }
    let payload = serde_json::json!({
        "generated_at": fresh.generated_at,
        "models": fresh.models.iter().map(model_to_json).collect::<Vec<_>>(),
    });
    std::fs::write(&path, serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?)
        .map_err(|exc| exc.to_string())?;
    Ok(fresh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_spellings_of_one_model_normalise_together() {
        assert_eq!(normalise("gemini-3.1-flash-image (nano-banana-2)"), "gemini31flashimage");
        assert_eq!(normalise("gemini-3.1-flash-image"), "gemini31flashimage");
        // And two different models do not.
        assert_ne!(normalise("gpt-image-1"), normalise("gpt-image-1.5-high-fidelity"));
    }

    #[test]
    fn a_quality_tier_is_told_from_a_codename() {
        assert_eq!(arena_variant("gpt-image-2 (medium)").as_deref(), Some("medium"));
        assert_eq!(arena_variant("grok-imagine-image-2.0 (low)").as_deref(), Some("low"));
        // A codename in the same brackets is not a price variant.
        assert_eq!(arena_variant("gemini-3.1-flash-image (nano-banana-2)"), None);
        assert_eq!(arena_variant("mai-image-2.5"), None);
    }

    #[test]
    fn a_price_per_image_is_taken_as_it_is_plus_the_reference() {
        let pricing = serde_json::json!([
            {"billable": "output_image", "unit": "image", "cost_usd": 0.04},
            {"billable": "input_image", "unit": "image", "cost_usd": 0.01}
        ]);
        // Grok's input is a quarter of the bill and invisible to anyone
        // comparing output prices.
        let cost = cost_per_edit(pricing.as_array().unwrap(), "x-ai/grok", None).unwrap();
        assert!((cost - 0.05).abs() < 1e-9, "{}", cost);
    }

    #[test]
    fn a_price_per_megapixel_is_converted() {
        let pricing = serde_json::json!([
            {"billable": "output_image", "unit": "megapixel", "cost_usd": 0.03}
        ]);
        let cost = cost_per_edit(pricing.as_array().unwrap(), "black-forest-labs/flux", None)
            .unwrap();
        assert!((cost - 0.03 * TARGET_MEGAPIXELS).abs() < 1e-9, "{}", cost);
    }

    #[test]
    fn a_price_per_token_needs_a_published_token_count() {
        let pricing = serde_json::json!([
            {"billable": "output_image", "unit": "token", "cost_usd": 0.00006}
        ]);
        let entries = pricing.as_array().unwrap();
        // Google publishes 1120 tokens, and 1120 x $0.00006 is $0.0672.
        let known = cost_per_edit(entries, "google/gemini-3.1-flash-image", None).unwrap();
        assert!((known - 0.0672).abs() < 1e-6, "{}", known);
        // Microsoft publishes none, so there is no price rather than a guess.
        assert_eq!(cost_per_edit(entries, "microsoft/mai-image-2.5", None), None);
    }

    #[test]
    fn a_variant_price_matches_the_tier_the_arena_rated() {
        let pricing = serde_json::json!([
            {"billable": "output_image", "unit": "image", "variant": "low_1k", "cost_usd": 0.02},
            {"billable": "output_image", "unit": "image", "variant": "medium_1k", "cost_usd": 0.05},
            {"billable": "output_image", "unit": "image", "variant": "low_2k", "cost_usd": 0.09}
        ]);
        let entries = pricing.as_array().unwrap();
        // The board scored the low tier, so the price is low_1k - not the
        // cheapest entry that happens to come first, and not a 2k one.
        let low = cost_per_edit(entries, "x-ai/grok", Some("low")).unwrap();
        assert!((low - 0.02).abs() < 1e-9, "{}", low);
        let medium = cost_per_edit(entries, "x-ai/grok", Some("medium")).unwrap();
        assert!((medium - 0.05).abs() < 1e-9, "{}", medium);
        // With no tier named, the 1k entries are the pool and the cheapest wins.
        let untiered = cost_per_edit(entries, "x-ai/grok", None).unwrap();
        assert!((untiered - 0.02).abs() < 1e-9, "{}", untiered);
    }

    #[test]
    fn a_base_price_beats_the_variants() {
        let pricing = serde_json::json!([
            {"billable": "output_image", "unit": "image", "cost_usd": 0.03},
            {"billable": "output_image", "unit": "image", "variant": "high_resolution",
             "cost_usd": 0.12}
        ]);
        let cost =
            cost_per_edit(pricing.as_array().unwrap(), "bytedance-seed/seedream", None).unwrap();
        assert!((cost - 0.03).abs() < 1e-9, "{}", cost);
    }

    #[test]
    fn an_unknown_unit_has_no_price_rather_than_a_zero() {
        let pricing = serde_json::json!([
            {"billable": "output_image", "unit": "furlong", "cost_usd": 0.03}
        ]);
        assert_eq!(cost_per_edit(pricing.as_array().unwrap(), "who/knows", None), None);
        assert_eq!(cost_per_edit(&[], "who/knows", None), None);
    }

    #[test]
    fn only_a_model_that_edits_counts() {
        let editor = serde_json::json!({
            "architecture": {"input_modalities": ["text", "image"]},
            "supported_parameters": {"input_references": {"max": 4}}
        });
        assert!(can_edit(&editor));
        // Reads pictures but takes no reference: it can describe, not edit.
        let reader = serde_json::json!({
            "architecture": {"input_modalities": ["text", "image"]},
            "supported_parameters": {"input_references": {"max": 0}}
        });
        assert!(!can_edit(&reader));
        // Generates from words alone.
        let painter = serde_json::json!({
            "architecture": {"input_modalities": ["text"]},
            "supported_parameters": {"input_references": {"max": 4}}
        });
        assert!(!can_edit(&painter));
        assert!(!can_edit(&serde_json::json!({})));
    }

    #[test]
    fn the_openai_menu_is_the_newest_full_and_the_newest_mini() {
        let ids: Vec<String> = [
            "gpt-image-1",
            "gpt-image-2",
            "gpt-image-2-mini",
            "gpt-image-1-mini",
            "gpt-image-2-2026-01-30",
            "dall-e-3",
            "gpt-4o",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let (models, others) = split_openai(&ids);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "gpt-image-2");
        assert_eq!(models[1].id, "gpt-image-2-mini");
        // No board covers the direct API, so no rank and no price are invented.
        assert!(models.iter().all(|model| model.elo.is_none() && model.usd.is_none()));
        // A dated snapshot stays behind the search; a chat model is not here at all.
        let ids: Vec<&str> = others.iter().map(|model| model.id.as_str()).collect();
        assert!(ids.contains(&"gpt-image-2-2026-01-30"), "{:?}", ids);
        assert!(ids.contains(&"dall-e-3"), "{:?}", ids);
        assert!(!ids.contains(&"gpt-4o"), "{:?}", ids);
    }

    #[test]
    fn a_version_is_read_only_from_the_exact_shape() {
        assert_eq!(openai_version("gpt-image-2"), Some((2.0, false)));
        assert_eq!(openai_version("gpt-image-1.5-mini"), Some((1.5, true)));
        assert_eq!(openai_version("gpt-image-2-2026-01-30"), None);
        assert_eq!(openai_version("dall-e-3"), None);
    }

    #[test]
    fn the_median_is_the_middle_and_not_the_mean() {
        assert_eq!(median(vec![]), None);
        assert_eq!(median(vec![5.0]), Some(5.0));
        // One odd edit does not move the figure.
        assert_eq!(median(vec![10.0, 11.0, 90.0]), Some(11.0));
        assert_eq!(median(vec![10.0, 20.0]), Some(15.0));
    }

    #[test]
    fn the_seed_is_what_the_menu_shows_before_any_refresh() {
        let models = seed();
        assert_eq!(models.len(), CURATED.len());
        assert!(models.iter().any(|model| model.tier == Tier::Top));
        assert!(models.iter().any(|model| model.tier == Tier::Value));
        // Every seeded model carries the four columns the menu prints.
        assert!(models
            .iter()
            .all(|model| model.elo.is_some() && model.usd.is_some() && model.seconds.is_some()));
    }
}
