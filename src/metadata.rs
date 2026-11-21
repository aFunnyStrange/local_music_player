//! Conservative reversible tag decoding; audio files are never rewritten.
use encoding_rs::{GBK, WINDOWS_1252};

fn cjk(c: char) -> bool {
    ('\u{3400}'..='\u{9fff}').contains(&c)
}
fn suspicious(text: &str) -> bool {
    let chars: Vec<_> = text.chars().collect();
    if chars.iter().any(|c| *c == '\u{fffd}' || c.is_control()) {
        return true;
    }
    if chars.windows(2).any(|w| {
        matches!(w[0], 'Ã' | 'Â') && ('\u{80}'..='\u{bf}').contains(&w[1])
            || w[0] == 'â' && (w[1] == '€' || ('\u{80}'..='\u{bf}').contains(&w[1]))
    }) {
        return true;
    }
    let extended: Vec<_> = chars
        .iter()
        .filter(|c| ('\u{80}'..='\u{ff}').contains(c))
        .collect();
    extended.len() >= 2
        && (extended.iter().any(|c| !c.is_alphabetic())
            || chars
                .windows(3)
                .any(|w| w.iter().all(|c| ('\u{c0}'..='\u{ff}').contains(c))))
}

pub fn recover(text: &str, filename: &str) -> (String, String) {
    let text = text.trim();
    if text.is_empty()
        || [
            "kuwo",
            "kugou",
            "qqmusic",
            "未知",
            "未知歌手",
            "未知专辑",
            "未知标题",
            "unknown",
            "unknown artist",
            "unknown album",
        ]
        .contains(&text.to_lowercase().as_str())
    {
        return (
            String::new(),
            "Empty / placeholder tag · 空标签或占位标签".into(),
        );
    }
    let mut candidates = vec![];
    for source in ["latin1", "cp1252", "gbk"] {
        let bytes = match source {
            "latin1" => text
                .chars()
                .map(|c| u8::try_from(c as u32))
                .collect::<Result<Vec<_>, _>>()
                .ok(),
            _ => {
                let (b, _, error) = if source == "gbk" {
                    GBK.encode(text)
                } else {
                    WINDOWS_1252.encode(text)
                };
                (!error).then(|| b.into_owned())
            }
        };
        let Some(bytes) = bytes else { continue };
        for target in ["utf-8", "gbk"] {
            if source == "gbk" && target == "gbk" {
                continue;
            }
            let candidate = if target == "utf-8" {
                String::from_utf8(bytes.clone()).ok()
            } else {
                GBK.decode_without_bom_handling_and_without_replacement(&bytes)
                    .map(|s| s.into_owned())
            };
            let Some(candidate) = candidate else { continue };
            let candidate = candidate.trim().to_owned();
            if candidate.is_empty() || candidate == text || suspicious(&candidate) {
                continue;
            }
            let chars: Vec<_> = candidate.chars().collect();
            let agreement = chars.windows(2).any(|w| {
                w.iter().all(|c| cjk(*c)) && filename.contains(&w.iter().collect::<String>())
            });
            if (source == "gbk" && !agreement)
                || (source != "gbk" && !suspicious(text))
                || (target == "gbk" && !chars.iter().any(|c| cjk(*c)))
            {
                continue;
            }
            candidates.push((agreement, target == "utf-8", candidate, source, target));
        }
    }
    if let Some((_, _, value, source, target)) = candidates.into_iter().max() {
        return (value, format!("Recovered / 恢复编码 {source} → {target}"));
    }
    if suspicious(text) {
        (String::new(), "Unrecoverable tag / 无法可靠恢复".into())
    } else {
        (text.into(), "Original tag / 原始标签".into())
    }
}

pub fn resolve(raw: &[String; 3], stem: &str) -> ([String; 3], String) {
    let separator = regex::Regex::new(r"\s+-\s+").unwrap();
    let parts: Vec<_> = separator.splitn(stem, 2).collect();
    let (title, artist) = if parts.len() == 2 && parts.iter().all(|p| !p.trim().is_empty()) {
        (parts[1].trim(), parts[0].trim())
    } else {
        (stem, "")
    };
    let mut notes = vec![];
    let values = std::array::from_fn(|i| {
        let (mut value, mut note) = recover(&raw[i], stem);
        let fallback = [title, artist, ""][i];
        if value.is_empty() && !fallback.is_empty() {
            value = fallback.into();
            note.push_str("; filename / 使用文件名");
        }
        notes.push(format!(
            "{}: {note}",
            ["Title / 歌曲", "Artist / 歌手", "Album / 专辑"][i]
        ));
        value
    });
    (values, notes.join("\n"))
}

/// libVLC supplies the filename as a synthetic title for some untagged media.
/// Keep raw values for inspection, but resolve the display as missing metadata.
pub fn resolve_file(raw: &[String; 3], path: &std::path::Path) -> ([String; 3], String) {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let filename = path.file_name().unwrap_or_default().to_string_lossy();
    let mut display = raw.clone();
    let synthetic = !filename.is_empty()
        && crate::search::normalize(&raw[0]) == crate::search::normalize(&filename);
    if synthetic {
        display[0].clear();
    }
    let (values, mut notes) = resolve(&display, &stem);
    if synthetic {
        notes.push_str("\nDecoder filename fallback / 解码器文件名占位已替换");
    }
    (values, notes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decoder_filename_fallback_preserves_raw_and_real_titles() {
        let raw = ["歌手 - 歌曲.WAV".into(), "".into(), "".into()];
        let (values, _) = resolve_file(&raw, std::path::Path::new("歌手 - 歌曲.wav"));
        assert_eq!(values, ["歌曲", "歌手", ""]);
        assert_eq!(raw[0], "歌手 - 歌曲.WAV");
        let raw = ["真实标题".into(), "".into(), "".into()];
        assert_eq!(
            resolve_file(&raw, std::path::Path::new("歌手 - 歌曲.wav")).0[0],
            "真实标题"
        );
    }
    #[test]
    fn screenshot_and_readable_regressions() {
        for (input, expected) in [
            ("¼ÇÒä", "记忆"),
            ("Ö£ÒÁ½¡", "郑伊健"),
            ("09.Ò»Éú°®ÄãÒ»¸ö£¨¸ÖÇÙÇú£©", "09.一生爱你一个（钢琴曲）"),
            ("¸ÖÇÙÐÄÇé101 Volume2 CD4", "钢琴心情101 Volume2 CD4"),
        ] {
            assert_eq!(recover(input, "").0, expected);
        }
        for s in [
            "Beyoncé",
            "Björk",
            "Sigur Rós",
            "Déjà vu",
            "São Tomé",
            "音に出来る事",
            "月弯弯",
            "Αγάπη",
            "Любовь",
        ] {
            assert_eq!(recover(s, "其他歌曲").0, s);
        }
        for s in ["中文歌曲", "Français", "Beyoncé"] {
            let broken: String = s.as_bytes().iter().map(|b| *b as char).collect();
            assert_eq!(recover(&broken, s).0, s);
        }
        let (values, _) = resolve(
            &["kuwo".into(), "kuwo".into(), "kuwo".into()],
            "歌手 - 正常歌曲",
        );
        assert_eq!(values, ["正常歌曲", "歌手", ""]);
    }
}
