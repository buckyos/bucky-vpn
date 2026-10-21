#!/usr/bin/env python3
"""Dependency-free parser for the canonical Harness task manifest subset.

All generated lifecycle checkers import this module so quoted scalars, null
values, inline lists, and change bindings have one interpretation.
"""

from __future__ import annotations

import ast
import contextlib
import contextvars
import copy
import functools
import hashlib
import json
import re
from pathlib import Path
from urllib.parse import unquote, urlsplit


class TaskManifestError(ValueError):
    pass


class ValidationContent:
    """One read-only validation's bytes, decoded text, digests and parse results.

    Never retain this snapshot across commands or artifact writes. Paths are
    resolved at each access so a redirected symlink does not alias old content.
    """

    def __init__(self) -> None:
        self.bytes: dict[Path, bytes] = {}
        self.text: dict[Path, str] = {}
        self.digests: dict[Path, str] = {}
        self.parsed: dict[object, object] = {}


_validation_content = contextvars.ContextVar("validation_content", default=None)


@contextlib.contextmanager
def validation_content():
    """Share nested read-only helpers; release all content on exit, even on error."""
    current = _validation_content.get()
    if current is not None:
        yield current
        return
    snapshot = ValidationContent()
    token = _validation_content.set(snapshot)
    try:
        yield snapshot
    finally:
        _validation_content.reset(token)


def read_only_validation(function):
    @functools.wraps(function)
    def wrapped(*args, **kwargs):
        with validation_content():
            return function(*args, **kwargs)
    return wrapped


def invalidate_validation_content() -> None:
    """Drop a shared snapshot at a command/write boundary before reading again."""
    snapshot = _validation_content.get()
    if snapshot is not None:
        snapshot.bytes.clear()
        snapshot.text.clear()
        snapshot.digests.clear()
        snapshot.parsed.clear()


def cached_parse(function):
    """Memoize pure parsers within a validation; callers receive independent values."""
    @functools.wraps(function)
    def wrapped(*args, **kwargs):
        snapshot = _validation_content.get()
        if snapshot is None:
            return function(*args, **kwargs)

        def normalized(value):
            return value.resolve() if isinstance(value, Path) else value

        key = (function, tuple(normalized(arg) for arg in args),
               tuple((name, normalized(value)) for name, value in sorted(kwargs.items())))
        if key not in snapshot.parsed:
            snapshot.parsed[key] = function(*args, **kwargs)
        return copy.deepcopy(snapshot.parsed[key])
    return wrapped


def read_bytes(path: Path) -> bytes:
    snapshot = _validation_content.get()
    if snapshot is None:
        return path.read_bytes()
    key = path.resolve()
    if key not in snapshot.bytes:
        snapshot.bytes[key] = path.read_bytes()
    return snapshot.bytes[key]


def read_text(path: Path) -> str:
    snapshot = _validation_content.get()
    if snapshot is None:
        return path.read_text(encoding="utf-8")
    key = path.resolve()
    if key not in snapshot.text:
        # Preserve Path.read_text's universal-newline behavior while retaining
        # the original bytes for receipt and input fingerprints.
        snapshot.text[key] = read_bytes(path).decode("utf-8").replace("\r\n", "\n").replace("\r", "\n")
    return snapshot.text[key]


def content_digest(path: Path) -> str:
    """Reuse parsed document bytes; stream hash-only inputs without retaining them."""
    snapshot = _validation_content.get()
    key = path.resolve()
    if snapshot is not None and key in snapshot.digests:
        return snapshot.digests[key]
    if snapshot is not None and key in snapshot.bytes:
        digest = hashlib.sha256(snapshot.bytes[key]).hexdigest()
    else:
        hasher = hashlib.sha256()
        with path.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                hasher.update(chunk)
        digest = hasher.hexdigest()
    if snapshot is not None:
        snapshot.digests[key] = digest
    return digest


@cached_parse
def read_json(path: Path):
    return json.loads(read_text(path))


def test_run_directory(root: Path, scope: str, testplans: list[str]) -> Path:
    """Partition levels together by scope and full, version-bound packet paths."""
    binding = json.dumps([scope, sorted(set(testplans))], ensure_ascii=False)
    identity = hashlib.sha256(binding.encode("utf-8")).hexdigest()
    return root / ".harness" / "test-results" / "test-runs" / identity


@cached_parse
def markdown_prose(text: str) -> str:
    """Exclude HTML comments and fenced/indented examples, preserving lines.

    Fence state is handled before comments so literal <!-- inside code cannot
    hide subsequent prose. Inline backtick paths remain supported references.
    List content is unwrapped so nested reference definitions are visible too.
    """
    visible: list[str] = []
    fence: str | None = None
    fence_indent = 0
    list_indents: list[int] = []
    in_comment = False
    for line in text.splitlines(keepends=True):
        expanded = line.expandtabs(4)
        if fence is not None:
            indent = len(expanded) - len(expanded.lstrip(" "))
            if expanded.strip() and indent < fence_indent:
                # Leaving a list ends even an unclosed fence in that item.
                fence = None
            else:
                if re.fullmatch(rf" {{0,3}}{re.escape(fence[0])}{{{len(fence)},}}[ \t]*\r?\n?", expanded[fence_indent:]):
                    fence = None
                visible.append("\n")
                continue
        if not in_comment:
            if not expanded.strip():
                visible.append("\n")
                continue
            indent = len(expanded) - len(expanded.lstrip(" "))
            while list_indents and indent < list_indents[-1]:
                list_indents.pop()
            base = list_indents[-1] if list_indents else 0
            # Code indentation is relative to the active list's content, not
            # the document margin. A nested marker is prose only below that
            # four-column threshold; indented example lists remain excluded.
            if indent >= base + 4:
                visible.append("\n")
                continue
            item = re.match(r" {0,3}(?:[-+*]|[0-9]{1,9}[.)])( +|$)", expanded[base:].rstrip("\r\n"))
            if item:
                gap = len(item.group(1))
                base += item.end() - (gap - 1 if gap > 4 else 0)
                list_indents.append(base)
            body = expanded[base:]
            if body.startswith("    "):
                visible.append("\n")
                continue
            opening = re.match(r"^ {0,3}(`{3,}|~{3,})", body)
            if opening:
                fence = opening.group(1)
                fence_indent = base
                visible.append("\n")
                continue
            line = body
        parts: list[str] = []
        while line:
            marker = "-->" if in_comment else "<!--"
            before, found, after = line.partition(marker)
            if not in_comment:
                parts.append(before)
            if not found:
                break
            in_comment = not in_comment
            line = after
        visible.append("".join(parts).rstrip("\r\n") + "\n")
    return "".join(visible)


@cached_parse
def markdown_destinations(text: str) -> list[str]:
    """Read local-link destinations without truncating spaces or parentheses."""
    destinations: list[str] = []
    starts = re.finditer(r"\]\(\s*|(?m:^ {0,3}\[[^\]\n]+\]:[ \t]*)", text)
    for start in starts:
        tail = text[start.end():]
        if tail.startswith("<"):
            bracketed = re.match(r"<((?:\\.|[^>\\\n])*)>", tail)
            if bracketed:
                destinations.append(bracketed.group(1))
            continue
        destination: list[str] = []
        depth = 0
        escaped = False
        for char in tail:
            if escaped:
                destination.extend(("\\", char))
                escaped = False
                continue
            if char == "\\":
                escaped = True
                continue
            if char.isspace() or (char == ")" and depth == 0):
                break
            if char == "(":
                depth += 1
            elif char == ")":
                depth -= 1
            destination.append(char)
        if destination and depth == 0 and not escaped:
            destinations.append("".join(destination))
    return [re.sub(r"\\([^\w\s])", r"\1", value) for value in destinations]


def design_input_paths(
    root: Path, design: Path, *, cache: dict[tuple[Path, Path], list[Path]] | None = None,
) -> list[Path]:
    """Resolve the root design and its recursively referenced task-local sources.

    Split sources live in design/. Markdown links (including reference-style
    destinations), backtick paths and bare design/*.md paths are supported.
    Unreferenced drafts are not governing inputs; missing references fail closed.
    A caller-owned cache is valid only within one read-only validation; discard
    it across command execution or artifact edits so reference changes are read.
    """
    root = root.resolve()
    design = design.resolve()
    key = (root, design)
    if cache is not None and key in cache:
        return list(cache[key])
    folder = design.parent / "design"
    pending = [design]
    seen: set[Path] = set()
    while pending:
        path = pending.pop()
        if path in seen:
            continue
        if not path.is_file():
            raise TaskManifestError(f"missing referenced design source: {path}")
        seen.add(path)
        text = markdown_prose(read_text(path))
        references = markdown_destinations(text)
        references += re.findall(r"`([^`\n]+\.md(?:#[^`\n]*)?)`", text)
        references += re.findall(r"(?<![\w/])((?:\./)?design/[^\s`<>\])]+\.md)", text)
        for value in references:
            url = urlsplit(value)
            if url.scheme or url.netloc or not url.path:
                continue
            # Patterns describe source families, not individual referenced files.
            # In particular, policy prose commonly names design/*.md.
            if "*" in unquote(url.path):
                continue
            relative = Path(unquote(url.path))
            if relative.suffix.lower() != ".md":
                continue
            lexical = path.parent / relative
            candidate = lexical.resolve()
            # Only split design sources are traversed, never proposal/testing
            # documents or unrelated repository documentation.
            if not lexical.is_relative_to(folder) and not candidate.is_relative_to(folder):
                continue
            if not candidate.is_relative_to(folder) or not candidate.is_relative_to(root):
                raise TaskManifestError(f"design source resolves outside task design/: {value}")
            pending.append(candidate)
    result = sorted(seen, key=lambda path: path.relative_to(root).as_posix())
    if cache is not None:
        cache[key] = result
    return list(result)


def scalar(value: str) -> str | None:
    value = value.strip()
    if not value or value.lower() in {"null", "none", "~"}:
        return None
    if value[:1] in {"\"", "'"}:
        if value[-1:] != value[:1]:
            raise TaskManifestError(f"unterminated quoted scalar {value!r}")
        try:
            parsed = ast.literal_eval(value)
        except (SyntaxError, ValueError) as error:
            raise TaskManifestError(f"invalid quoted scalar {value!r}: {error}") from error
        if not isinstance(parsed, str):
            raise TaskManifestError(f"expected string scalar, got {value!r}")
        return parsed
    return value


def inline_list(value: str, path: Path, line_number: int) -> list[str]:
    try:
        parsed = ast.literal_eval(value)
    except (SyntaxError, ValueError):
        if not (value.startswith("[") and value.endswith("]")):
            raise TaskManifestError(f"{path}:{line_number}: expected an inline list")
        body = value[1:-1].strip()
        parsed = [] if not body else [
            item.strip().strip("\"").strip("'") for item in body.split(",")
        ]
    if not isinstance(parsed, list) or not all(
        isinstance(item, str) and item for item in parsed
    ):
        raise TaskManifestError(
            f"{path}:{line_number}: expected a list of non-empty strings"
        )
    return parsed


@cached_parse
def parse_task_manifest(path: Path) -> dict[str, object]:
    """Parse top-level scalars plus the canonical changes list."""
    if not path.is_file():
        raise TaskManifestError(f"missing task manifest: {path}")
    task: dict[str, object] = {}
    changes: list[dict[str, object]] = []
    current: dict[str, object] | None = None
    in_changes = False
    for line_number, raw_line in enumerate(
        read_text(path).splitlines(), start=1
    ):
        stripped = raw_line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        if stripped == "changes:":
            if in_changes:
                raise TaskManifestError(f"{path}:{line_number}: duplicate changes section")
            in_changes = True
            current = None
            continue
        if not in_changes:
            match = re.fullmatch(r"([a-z_]+):\s*(.*?)\s*", stripped)
            if not match:
                raise TaskManifestError(
                    f"{path}:{line_number}: unsupported task manifest entry {stripped!r}"
                )
            key, value = match.groups()
            if key in task:
                raise TaskManifestError(f"{path}:{line_number}: duplicate field {key}")
            task[key] = 1 if key == "schema_version" and value == "1" else scalar(value)
            continue

        item = re.fullmatch(r"-\s+id:\s*(.*?)\s*", stripped)
        if item:
            current = {"id": scalar(item.group(1))}
            changes.append(current)
            continue
        field = re.fullmatch(r"([a-z_]+):\s*(.*?)\s*", stripped)
        if not field or current is None:
            raise TaskManifestError(
                f"{path}:{line_number}: malformed changes entry {stripped!r}"
            )
        key, value = field.groups()
        if key in current:
            raise TaskManifestError(
                f"{path}:{line_number}: duplicate change field {key}"
            )
        current[key] = (
            inline_list(value, path, line_number)
            if value.strip().startswith("[")
            else scalar(value)
        )
    removed = {"mode", "auto_pipeline_start_stage", "pipeline_plan"} & task.keys()
    if removed:
        raise TaskManifestError(f"{path}: execution-mode fields are no longer supported: {sorted(removed)}")
    task["changes"] = changes
    return task
