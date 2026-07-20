use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::net_audio_player::AudioTargetType;

const TEMPLATE_SUFFIX: &str = ".tts.toml";
const MAX_TEMPLATE_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsTemplate {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub text: String,
    pub voice_id: String,
    pub speed: f32,
    pub priority: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_type: Option<AudioTargetType>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<u32>,
    #[serde(default)]
    pub auto_saved: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct TtsTemplateDraft {
    pub id: Option<String>,
    pub name: String,
    pub text: String,
    pub voice_id: String,
    pub speed: f32,
    pub priority: u8,
    pub target_type: Option<AudioTargetType>,
    pub target_id: Option<u32>,
    pub auto_saved: bool,
}

pub(crate) fn list_templates(root: &Path) -> Result<Vec<TtsTemplate>, String> {
    let entries = fs::read_dir(root)
        .map_err(|error| format!("cannot read TTS template directory {}: {error}", root.display()))?;
    let mut templates = Vec::new();
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.ends_with(TEMPLATE_SUFFIX) {
            continue;
        }
        match read_template_file(&entry.path()) {
            Ok(template) => templates.push(template),
            Err(error) => tracing::warn!(
                "TTS templates: ignoring invalid file {}: {}",
                entry.path().display(),
                error
            ),
        }
    }
    templates.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(templates)
}

pub(crate) fn save_template(root: &Path, draft: TtsTemplateDraft) -> Result<TtsTemplate, String> {
    let now = now_rfc3339();
    let id = match draft.id.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        Some(id) => {
            validate_id(id)?;
            id.to_string()
        }
        None => generate_id(&draft.name),
    };
    let existing = read_template(root, &id).ok();
    let template = TtsTemplate {
        schema_version: 1,
        id: id.clone(),
        name: draft.name,
        text: draft.text,
        voice_id: draft.voice_id,
        speed: draft.speed,
        priority: draft.priority,
        target_type: draft.target_type,
        target_id: draft.target_id,
        auto_saved: draft.auto_saved,
        created_at: existing.map(|value| value.created_at).unwrap_or_else(|| now.clone()),
        updated_at: now,
    };
    validate_template(&template)?;
    write_template_file(root, &template)?;
    Ok(template)
}

pub(crate) fn auto_save_template(root: &Path, mut draft: TtsTemplateDraft) -> Result<TtsTemplate, String> {
    let templates = list_templates(root)?;
    if let Some(existing) = templates.iter().find(|template| same_content(template, &draft)) {
        if !existing.auto_saved {
            return Ok(existing.clone());
        }
        draft.id = Some(existing.id.clone());
        draft.name = existing.name.clone();
    }
    draft.auto_saved = true;
    save_template(root, draft)
}

pub(crate) fn delete_template(root: &Path, id: &str) -> Result<(), String> {
    validate_id(id)?;
    let path = template_path(root, id);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(format!("TTS template '{id}' does not exist"))
        }
        Err(error) => Err(format!("cannot delete TTS template {}: {error}", path.display())),
    }
}

fn read_template(root: &Path, id: &str) -> Result<TtsTemplate, String> {
    validate_id(id)?;
    read_template_file(&template_path(root, id))
}

fn read_template_file(path: &Path) -> Result<TtsTemplate, String> {
    let metadata = path
        .metadata()
        .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err("template path is not a regular file".to_string());
    }
    if metadata.len() > MAX_TEMPLATE_BYTES {
        return Err(format!("template is larger than {} KiB", MAX_TEMPLATE_BYTES / 1024));
    }
    let mut source = String::new();
    File::open(path)
        .and_then(|mut file| file.read_to_string(&mut source))
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let template: TtsTemplate = toml::from_str(&source)
        .map_err(|error| format!("invalid TOML: {error}"))?;
    validate_template(&template)?;
    let expected = template_path(path.parent().unwrap_or_else(|| Path::new(".")), &template.id);
    if expected.file_name() != path.file_name() {
        return Err(format!(
            "template id '{}' does not match file name",
            template.id
        ));
    }
    Ok(template)
}

fn write_template_file(root: &Path, template: &TtsTemplate) -> Result<(), String> {
    fs::create_dir_all(root)
        .map_err(|error| format!("cannot create TTS template directory {}: {error}", root.display()))?;
    let final_path = template_path(root, &template.id);
    let temp_path = root.join(format!(".{}.{}.tmp", template.id, Uuid::new_v4().simple()));
    let body = toml::to_string_pretty(template)
        .map_err(|error| format!("cannot serialize TTS template '{}': {error}", template.id))?;
    let write_result = (|| -> Result<(), String> {
        let mut file = File::create(&temp_path)
            .map_err(|error| format!("cannot create {}: {error}", temp_path.display()))?;
        file.write_all(body.as_bytes())
            .map_err(|error| format!("cannot write {}: {error}", temp_path.display()))?;
        file.sync_all()
            .map_err(|error| format!("cannot sync {}: {error}", temp_path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o640))
                .map_err(|error| format!("cannot set permissions on {}: {error}", temp_path.display()))?;
        }
        fs::rename(&temp_path, &final_path)
            .map_err(|error| format!("cannot finalize {}: {error}", final_path.display()))?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    write_result
}

fn template_path(root: &Path, id: &str) -> PathBuf {
    root.join(format!("{id}{TEMPLATE_SUFFIX}"))
}

fn validate_template(template: &TtsTemplate) -> Result<(), String> {
    if template.schema_version != 1 {
        return Err(format!("unsupported schema_version {}", template.schema_version));
    }
    validate_id(&template.id)?;
    let name_count = template.name.chars().count();
    if template.name.trim().is_empty() || name_count > 120 {
        return Err("template name must contain 1-120 characters".to_string());
    }
    if template.text.trim().is_empty() {
        return Err("template text cannot be empty".to_string());
    }
    if template.voice_id.trim().is_empty() {
        return Err("template voice_id cannot be empty".to_string());
    }
    if !template.speed.is_finite() || !(0.50..=1.50).contains(&template.speed) {
        return Err("template speed must be between 0.50 and 1.50".to_string());
    }
    if template.priority > 15 {
        return Err("template priority must be 0-15".to_string());
    }
    match (template.target_type, template.target_id) {
        (None, None) => {}
        (Some(_), Some(id)) if id > 0 && id <= 0x00ff_ffff => {}
        (Some(_), Some(_)) => return Err("template target must be a valid 24-bit ISSI/GSSI".to_string()),
        _ => return Err("template target_type and target_id must either both be set or both be omitted".to_string()),
    }
    Ok(())
}

fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 96
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err("template id may contain only letters, digits, '.', '-' and '_' (max. 96 bytes)".to_string());
    }
    Ok(())
}

fn same_content(template: &TtsTemplate, draft: &TtsTemplateDraft) -> bool {
    template.text == draft.text
        && template.voice_id == draft.voice_id
        && (template.speed - draft.speed).abs() < 0.0001
        && template.priority == draft.priority
        && template.target_type == draft.target_type
        && template.target_id == draft.target_id
}

fn generate_id(name: &str) -> String {
    let slug = slugify(name);
    let suffix = Uuid::new_v4().simple().to_string();
    format!("{}-{}", if slug.is_empty() { "vorlage" } else { &slug }, &suffix[..8])
}

fn slugify(value: &str) -> String {
    let mut output = String::new();
    let mut separator = false;
    for ch in value.to_lowercase().chars() {
        let replacement = match ch {
            'ä' => Some("ae"),
            'ö' => Some("oe"),
            'ü' => Some("ue"),
            'ß' => Some("ss"),
            ch if ch.is_ascii_alphanumeric() => {
                output.push(ch);
                separator = false;
                None
            }
            _ => {
                if !output.is_empty() {
                    separator = true;
                }
                None
            }
        };
        if let Some(replacement) = replacement {
            if separator && !output.ends_with('-') {
                output.push('-');
            }
            output.push_str(replacement);
            separator = false;
        } else if separator && !output.ends_with('-') {
            output.push('-');
            separator = false;
        }
        if output.len() >= 56 {
            break;
        }
    }
    output.trim_matches('-').to_string()
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_is_safe() {
        assert_eq!(slugify("Räumung – Haupthalle"), "raeumung-haupthalle");
    }
}
