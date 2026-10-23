"""Strict World JSON check for agents and CI.

``WorldDoc::from_json`` ignores unknown fields. This tool does not. It builds
a pydantic model from ``docs/schemas/world.json`` (``extra='forbid'``) and
reports a path an agent can fix:

    props[0].positon: unknown field; did you mean position?
    coins: expected integer, got str
    version: missing required field

JSON integers still coerce to ``number`` fields. Integer fields stay integers
and do not accept strings. Null is allowed only where the schema type includes
null. An object with no ``properties`` (event ``data``) keeps arbitrary keys.

Desktop and CI only. This module does not import ``kagra`` or the Rust
extension, and it is not part of the wheel. The Rust parser stays the runtime.
"""

from __future__ import annotations

import difflib
import json
import sys
from pathlib import Path
from typing import Annotated, Any, Literal, Union

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = ROOT / "docs" / "schemas" / "world.json"

_TYPE_WORD = {
    "integer": "integer",
    "number": "number",
    "string": "string",
    "boolean": "boolean",
    "array": "array",
    "object": "object",
    "null": "null",
}


def _as_number(value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError("expected number")
    return float(value)


def _got_name(value: Any) -> str:
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "bool"
    if isinstance(value, int):
        return "int"
    if isinstance(value, float):
        return "float"
    if isinstance(value, str):
        return "str"
    if isinstance(value, list):
        return "list"
    if isinstance(value, dict):
        return "dict"
    return type(value).__name__


def _union(parts: list[Any]) -> Any:
    unique: list[Any] = []
    for part in parts:
        if part not in unique:
            unique.append(part)
    if len(unique) == 1:
        return unique[0]
    return Union[tuple(unique)]


def _format_loc(loc: tuple) -> str:
    out = ""
    for item in loc:
        if isinstance(item, int):
            out += f"[{item}]"
        else:
            if out:
                out += "."
            out += str(item)
    return out or "world"


class WorldChecker:
    """One schema document, compiled once."""

    def __init__(self, schema_path: Path | None = None):
        path = Path(schema_path) if schema_path else SCHEMA_PATH
        self.doc = json.loads(path.read_text(encoding="utf-8"))
        self._models: dict[int, Any] = {}
        self._seq = 0
        # Imported lazily so a machine without pydantic can import this
        # module only after pytest.importorskip. check_world imports it.
        from pydantic import BaseModel, BeforeValidator, ConfigDict, Field, create_model

        self._BeforeValidator = BeforeValidator
        self._Field = Field
        self._create_model = create_model
        self._Strict = type(
            "StrictModel",
            (BaseModel,),
            {"model_config": ConfigDict(extra="forbid")},
        )
        self.model = self._compile(self.doc, "World")

    def errors(self, data: Any) -> list[str]:
        if not isinstance(data, dict):
            return [f"world: expected object, got {_got_name(data)}"]
        from pydantic import ValidationError

        try:
            self.model.model_validate(data)
        except ValidationError as exc:
            return [self._format(err) for err in exc.errors()]
        return []

    def _compile(self, schema: dict, hint: str) -> Any:
        schema = self._unwrap(schema)
        if "oneOf" in schema or "anyOf" in schema:
            key = "oneOf" if "oneOf" in schema else "anyOf"
            parts = [
                self._compile(branch, f"{hint}_{index}")
                for index, branch in enumerate(schema[key])
            ]
            return _union(parts)
        if "const" in schema and "type" not in schema and "properties" not in schema:
            return Literal[schema["const"]]
        if "enum" in schema and "type" not in schema:
            return Literal[tuple(schema["enum"])]

        types = schema.get("type")
        if types is None:
            if "properties" in schema:
                types = "object"
            elif "items" in schema:
                types = "array"
            elif "const" in schema:
                return Literal[schema["const"]]
            else:
                return Any
        if isinstance(types, str):
            types = [types]

        parts = []
        for kind in types:
            if kind == "null":
                parts.append(type(None))
            elif kind == "integer":
                parts.append(self._number_type(schema, integer=True))
            elif kind == "number":
                parts.append(self._number_type(schema, integer=False))
            elif kind == "string":
                if "enum" in schema:
                    parts.append(Literal[tuple(schema["enum"])])
                elif "const" in schema:
                    parts.append(Literal[schema["const"]])
                else:
                    parts.append(str)
            elif kind == "boolean":
                parts.append(Annotated[bool, self._Field(strict=True)])
            elif kind == "array":
                item = self._compile(schema.get("items", {}), f"{hint}_item")
                constraints: dict[str, Any] = {}
                if "minItems" in schema:
                    constraints["min_length"] = schema["minItems"]
                if "maxItems" in schema:
                    constraints["max_length"] = schema["maxItems"]
                py = list[item]
                if constraints:
                    py = Annotated[py, self._Field(**constraints)]
                parts.append(py)
            elif kind == "object":
                parts.append(self._object(schema, hint))
            else:
                parts.append(Any)
        return _union(parts)

    def _number_type(self, schema: dict, *, integer: bool) -> Any:
        constraints: dict[str, Any] = {}
        if "minimum" in schema:
            constraints["ge"] = schema["minimum"]
        if "maximum" in schema:
            constraints["le"] = schema["maximum"]
        if integer:
            constraints["strict"] = True
            py: Any = int
            if constraints:
                py = Annotated[int, self._Field(**constraints)]
            return py
        py = Annotated[float, self._BeforeValidator(_as_number)]
        if constraints:
            py = Annotated[py, self._Field(**constraints)]
        return py

    def _object(self, schema: dict, hint: str) -> Any:
        key = id(schema)
        cached = self._models.get(key)
        if cached is not None:
            return cached
        properties = schema.get("properties")
        if not properties:
            return dict[str, Any]
        self._seq += 1
        required = set(schema.get("required") or [])
        fields: dict[str, Any] = {}
        for name, prop in properties.items():
            ann = self._compile(prop, f"{hint}_{name}")
            fields[name] = (ann, ...) if name in required else (ann, None)
        model = self._create_model(f"{hint}_{self._seq}", __base__=self._Strict, **fields)
        self._models[key] = model
        return model

    def _unwrap(self, schema: dict) -> dict:
        seen: set[str] = set()
        while isinstance(schema, dict) and "$ref" in schema:
            ref = schema["$ref"]
            if ref in seen:
                break
            seen.add(ref)
            schema = self._resolve(ref)
        return schema

    def _resolve(self, ref: str) -> dict:
        if not ref.startswith("#/"):
            raise ValueError(f"unsupported schema ref {ref}")
        node: Any = self.doc
        for part in ref[2:].split("/"):
            node = node[part]
        return node

    def _descend(self, node: dict, part: Any) -> dict:
        node = self._unwrap(node)
        for key in ("oneOf", "anyOf"):
            if key not in node:
                continue
            for branch in node[key]:
                branch = self._unwrap(branch)
                if isinstance(part, int) and (
                    branch.get("type") == "array" or "items" in branch
                ):
                    return self._descend(branch, part)
                if isinstance(part, str) and (
                    "properties" in branch or branch.get("type") == "object"
                ):
                    props = branch.get("properties") or {}
                    if not props or part in props:
                        return self._descend(branch, part)
            return self._descend(self._unwrap(node[key][0]), part)
        if isinstance(part, int):
            return node.get("items", {})
        return (node.get("properties") or {}).get(part, {})

    def _schema_at(self, loc: tuple) -> dict:
        node = self.doc
        for part in loc:
            node = self._descend(node, part)
        return self._unwrap(node) if isinstance(node, dict) else {}

    def _property_names(self, node: dict) -> list[str]:
        if not isinstance(node, dict):
            return []
        node = self._unwrap(node)
        names = list((node.get("properties") or {}).keys())
        for key in ("oneOf", "anyOf"):
            if key not in node:
                continue
            for branch in node[key]:
                for name in self._property_names(branch):
                    if name not in names:
                        names.append(name)
        return names

    def _expected(self, node: dict) -> str:
        if not isinstance(node, dict):
            return "value"
        node = self._unwrap(node)
        if "const" in node and "type" not in node:
            return json.dumps(node["const"])
        for key in ("oneOf", "anyOf"):
            if key in node:
                labels = []
                for branch in node[key]:
                    label = self._expected(branch)
                    if label not in labels:
                        labels.append(label)
                return " or ".join(labels)
        types = node.get("type")
        if isinstance(types, list):
            return " or ".join(_TYPE_WORD.get(item, item) for item in types)
        if isinstance(types, str):
            if types == "string" and "enum" in node:
                return "one of " + ", ".join(json.dumps(item) for item in node["enum"])
            return _TYPE_WORD.get(types, types)
        if "properties" in node:
            return "object"
        if "enum" in node:
            return "one of " + ", ".join(json.dumps(item) for item in node["enum"])
        return "value"

    def _format(self, err: dict) -> str:
        loc = tuple(err.get("loc") or ())
        path = _format_loc(loc)
        kind = err.get("type") or ""
        if kind == "extra_forbidden":
            parent = self._schema_at(loc[:-1]) if loc else self.doc
            names = self._property_names(parent)
            hint = difflib.get_close_matches(str(loc[-1]) if loc else "", names, n=1, cutoff=0.6)
            message = f"{path}: unknown field"
            if hint:
                message += f"; did you mean {hint[0]}?"
            return message
        if kind == "missing":
            return f"{path}: missing required field"
        node = self._schema_at(loc)
        expected = self._expected(node)
        got = _got_name(err.get("input"))
        ctx = err.get("ctx") or {}
        if kind == "greater_than_equal" and "ge" in ctx:
            return f"{path}: expected {expected} >= {ctx['ge']}, got {err.get('input')}"
        if kind == "less_than_equal" and "le" in ctx:
            return f"{path}: expected {expected} <= {ctx['le']}, got {err.get('input')}"
        if kind == "literal_error":
            wanted = ctx.get("expected", expected)
            raw = err.get("input")
            shown = json.dumps(raw) if not isinstance(raw, (dict, list)) else got
            return f"{path}: expected {wanted}, got {shown}"
        if kind == "too_short" and "min_length" in ctx:
            return (
                f"{path}: expected at least {ctx['min_length']} items, "
                f"got {ctx.get('actual_length')}"
            )
        if kind == "too_long" and "max_length" in ctx:
            return (
                f"{path}: expected at most {ctx['max_length']} items, "
                f"got {ctx.get('actual_length')}"
            )
        return f"{path}: expected {expected}, got {got}"


_CHECKERS: dict[str, WorldChecker] = {}


def check_world(data: Any, schema_path: Path | None = None) -> list[str]:
    """Return one line per schema failure. An empty list means the dump matches."""
    path = str(schema_path or SCHEMA_PATH)
    checker = _CHECKERS.get(path)
    if checker is None:
        checker = WorldChecker(Path(path))
        _CHECKERS[path] = checker
    return checker.errors(data)


def main(argv: list[str] | None = None) -> int:
    args = list(sys.argv[1:] if argv is None else argv)
    if not args:
        print("usage: check_world_json.py <world.json>...", file=sys.stderr)
        return 2
    failed = False
    for raw in args:
        path = Path(raw)
        try:
            data = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            print(f"{path}: {exc}", file=sys.stderr)
            failed = True
            continue
        found = check_world(data)
        if found:
            failed = True
            for line in found:
                print(f"{path}: {line}", file=sys.stderr)
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
