"""Conservatively recover misdecoded tags without rewriting source audio."""
import re
import unicodedata


PLACEHOLDERS = frozenset({"kuwo", "kugou", "qqmusic", "未知", "未知歌手", "未知专辑",
                          "未知标题", "unknown", "unknown artist", "unknown album"})


def cjk_count(text: str) -> int:
    """Count Chinese/Japanese ideographs as evidence, not as a quality ranking."""
    return sum("\u3400" <= char <= "\u9fff" for char in text)


def suspicious(text: str) -> bool:
    """Recognize strong Latin mojibake evidence while leaving ordinary accents alone."""
    if "\ufffd" in text or any(unicodedata.category(char) == "Cc" for char in text):
        return True
    if re.search(r"[ÃÂ][\u0080-\u00bf]|â[€\u0080-\u00bf]", text):
        return True
    extended = [char for char in text if "\u0080" <= char <= "\u00ff"]
    if len(extended) < 2:
        return False
    symbols = any(not char.isalpha() for char in extended)
    dense_run = re.search(r"[\u00c0-\u00ff]{3,}", text) is not None
    return symbols or dense_run


def recover_tag(text: str, filename: str) -> tuple[str, str]:
    """Try strict reversible decoding only; return unresolved tags as empty values."""
    text = text.strip()
    if not text or text.casefold() in PLACEHOLDERS:
        return "", "占位或空标签"
    candidates = []
    for source in ("latin1", "cp1252", "gbk"):
        for target in (("utf-8",) if source == "gbk" else ("utf-8", "gbk")):
            try:
                raw = text.encode(source)
                candidate = raw.decode(target).strip()
            except UnicodeError:
                continue
            if candidate == text or not candidate or suspicious(candidate):
                continue
            # Filename agreement is useful for UTF-8 accidentally decoded as GBK.
            agreement = any(candidate[i:i + 2] in filename for i in range(len(candidate) - 1)
                            if cjk_count(candidate[i:i + 2]) == 2)
            if source == "gbk" and not agreement:
                continue
            if source != "gbk" and not suspicious(text):
                continue
            if target == "gbk" and not cjk_count(candidate):
                continue
            candidates.append((agreement, target == "utf-8", candidate, source, target))
    if candidates:
        _, _, candidate, source, target = max(candidates)
        return candidate, f"恢复编码 {source} → {target}"
    if suspicious(text):
        return "", "标签疑似乱码且无法可靠恢复"
    return text, "原始标签"


def filename_parts(stem: str) -> tuple[str, str]:
    """Split only a spaced artist-title separator; preserve ambiguous filenames whole."""
    parts = re.split(r"\s+-\s+", stem, maxsplit=1)
    if len(parts) == 2 and all(part.strip() for part in parts):
        return parts[1].strip(), parts[0].strip()
    return stem, ""


def resolve_metadata(title: str, artist: str, album: str, stem: str) -> tuple:
    """Prefer recoverable tags and use readable filenames for missing/broken fields."""
    file_title, file_artist = filename_parts(stem)
    resolved = []
    notes = []
    for label, raw, fallback in (("歌曲", title, file_title), ("歌手", artist, file_artist),
                                 ("专辑", album, "")):
        value, reason = recover_tag(raw, stem)
        if not value and fallback:
            value = fallback
            reason += "；使用文件名"
        if not value and label == "专辑" and raw:
            reason += "；不展示无效专辑"
        resolved.append(value)
        notes.append(f"{label}：{reason}")
    return *resolved, "\n".join(notes)
