"""Small Everything-style text filter: AND terms, OR groups, exclusions and phrases."""
import re
import unicodedata
from dataclasses import dataclass
from typing import Pattern


def normalize(text: str) -> str:
    """Normalize case and character width without changing the displayed metadata."""
    return unicodedata.normalize("NFKC", text).casefold()


@dataclass(frozen=True)
class Term:
    """Represent one substring/wildcard matcher and its optional negation."""
    pattern: Pattern[str]
    exclude: bool


def compile_query(query: str) -> list[list[Term]]:
    """Parse forgiving live input without treating backslashes as escape sequences."""
    groups: list[list[Term]] = [[]]
    token = []
    in_quotes = False
    quoted = False
    prefix_quoted = False

    def flush() -> None:
        """Compile one token once per query rather than once per track."""
        nonlocal token, quoted, prefix_quoted
        value = "".join(token)
        token = []
        if value:
            exclude = len(value) > 1 and value[0] in "!-" and not prefix_quoted
            if exclude:
                value = value[1:]
            pattern = re.escape(value)
            if not quoted:
                pattern = pattern.replace(r"\*", ".*").replace(r"\?", ".")
            groups[-1].append(Term(re.compile(pattern), exclude))
        quoted = False
        prefix_quoted = False

    for char in normalize(query):
        if char == '"':
            if not token and not in_quotes:
                prefix_quoted = True
            in_quotes = not in_quotes
            quoted = True
        elif not in_quotes and (char.isspace() or char == "|"):
            flush()
            if char == "|" and groups[-1]:
                groups.append([])
        else:
            token.append(char)
    flush()
    return [group for group in groups if group]


def matches(text: str, groups: list[list[Term]]) -> bool:
    """Require every term within a group and accept any nonempty alternative group."""
    text = normalize(text)
    return not groups or any(all(bool(term.pattern.search(text)) != term.exclude for term in group)
                             for group in groups)
