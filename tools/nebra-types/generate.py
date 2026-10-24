"""Generates the Nebra declarations in nebra/src/quark.d.neb from the LuaLS definitions in library/."""

import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LIBRARY = ROOT / "library"
OUTPUT = ROOT / "nebra" / "src" / "quark.d.neb"

NANOS_TYPES = {"Vector", "Vector2D", "Rotator", "Color"}
PRIMITIVES = {
    "integer": "number",
    "number": "number",
    "string": "string",
    "boolean": "boolean",
    "any": "any",
    "nil": "nil",
    "function": "function",
    "thread": "thread",
    "userdata": "userdata",
    "table": "{ [any]: any }",
}
LOOSE_TABLE = "{ [string]: any }"


class ParseError(Exception):
    pass


class TypeParser:
    def __init__(self, text, aliases, shapes=frozenset(), loose=False):
        self.text = text
        self.position = 0
        self.aliases = aliases
        self.shapes = shapes
        self.loose = loose

    def skip_spaces(self):
        while self.position < len(self.text) and self.text[self.position] == " ":
            self.position += 1

    def peek(self, token):
        self.skip_spaces()
        return self.text.startswith(token, self.position)

    def accept(self, token):
        if self.peek(token):
            self.position += len(token)
            return True
        return False

    def expect(self, token):
        if not self.accept(token):
            raise ParseError(f"expected '{token}' at {self.position} in '{self.text}'")

    def name(self):
        self.skip_spaces()
        match = re.compile(r"[A-Za-z_][A-Za-z0-9_.]*").match(self.text, self.position)
        if not match:
            raise ParseError(f"expected a name at {self.position} in '{self.text}'")
        self.position = match.end()
        return match.group(0)

    def union(self):
        parts = [self.postfix()]
        while True:
            self.skip_spaces()
            if self.peek("|"):
                self.position += 1
                parts.append(self.postfix())
            else:
                break
        return join_union(parts)

    def postfix(self):
        atom = self.atom()
        while True:
            if self.text.startswith("[]", self.position):
                self.position += 2
                atom = f"{group(atom)}[]"
            elif self.text.startswith("?", self.position):
                self.position += 1
                atom = nullable(atom)
            else:
                return atom

    def atom(self):
        self.skip_spaces()
        if self.accept("("):
            inner = self.union()
            self.expect(")")
            return inner
        if self.peek('"') or self.peek("'"):
            quote = self.text[self.position]
            end = self.text.index(quote, self.position + 1)
            self.position = end + 1
            return "string"
        if self.peek("{"):
            return self.table()
        if re.compile(r"-?[0-9]").match(self.text, self.position):
            match = re.compile(r"-?[0-9.]+").match(self.text, self.position)
            self.position = match.end()
            return "number"
        if self.peek("..."):
            self.position += 3
            return "...any"
        name = self.name()
        if name == "fun":
            loose, self.loose = self.loose, False
            try:
                return self.function_type()
            finally:
                self.loose = loose
        if name == "table" and self.peek("<"):
            self.expect("<")
            key = self.union()
            self.expect(",")
            value = self.union()
            self.expect(">")
            return f"{{ [{key}]: {value} }}"
        if name in ("true", "false"):
            return "boolean"
        if name in PRIMITIVES:
            return PRIMITIVES[name]
        aliases = self.aliases["loose" if self.loose else "strict"]
        if name in aliases:
            return f"({aliases[name]})" if "|" in aliases[name] else aliases[name]
        if self.loose and name in self.shapes:
            return LOOSE_TABLE
        return name

    def function_type(self):
        self.expect("(")
        parameters = []
        if not self.accept(")"):
            while True:
                self.skip_spaces()
                if self.accept("..."):
                    kind = "any"
                    if self.accept(":"):
                        kind = self.union()
                    parameters.append(f"...{group(kind)}")
                else:
                    self.name()
                    optional = self.accept("?")
                    kind = "any"
                    if self.accept(":"):
                        kind = self.union()
                    parameters.append(nullable(kind) if optional else kind)
                if self.accept(")"):
                    break
                self.expect(",")
        returns = []
        if self.accept(":"):
            returns.append(self.postfix_or_variadic())
            while self.peek(",") and self.looks_like_return():
                self.expect(",")
                returns.append(self.postfix_or_variadic())
        if not returns:
            result = "void"
        elif len(returns) == 1:
            result = returns[0]
        else:
            result = "(" + ", ".join(returns) + ")"
        return f"({', '.join(parameters)}) -> {result}"

    def looks_like_return(self):
        saved = self.position
        try:
            self.expect(",")
            self.skip_spaces()
            return not re.compile(r"[A-Za-z_][A-Za-z0-9_]*\??\s*:").match(self.text, self.position)
        except ParseError:
            return False
        finally:
            self.position = saved

    def postfix_or_variadic(self):
        if self.accept("..."):
            return "...any"
        parts = [self.postfix()]
        while self.peek("|"):
            self.position += 1
            self.skip_spaces()
            parts.append(self.postfix())
        return group(join_union(parts))

    def table(self):
        if self.loose:
            loose, self.loose = self.loose, False
            try:
                self.table()
            finally:
                self.loose = loose
            return LOOSE_TABLE
        self.expect("{")
        if self.accept("}"):
            return "{ [any]: any }"
        if self.peek("["):
            self.expect("[")
            key = self.union()
            self.expect("]")
            self.expect(":")
            value = self.union()
            if self.accept(","):
                values = [value]
                while not self.accept("}"):
                    self.expect("[")
                    self.union()
                    self.expect("]")
                    self.expect(":")
                    values.append(self.union())
                    self.accept(",")
                return "any[]"
            self.expect("}")
            if key == "number":
                return f"{group(value)}[]"
            return f"{{ [{key}]: {value} }}"
        fields = []
        while True:
            name = self.name()
            optional = self.accept("?")
            self.expect(":")
            kind = self.union()
            fields.append(f"{name}: {nullable(kind) if optional else kind}")
            if self.accept("}"):
                break
            self.expect(",")
            if self.accept("}"):
                break
        return "{ " + ", ".join(fields) + " }"

    def parse_prefix(self):
        result = self.union()
        return result, self.text[self.position:].strip()


def split_union(text):
    parts, depth, current = [], 0, ""
    for character in text:
        if character in "({<[":
            depth += 1
        elif character in ")}>]":
            depth -= 1
        if character == "|" and depth == 0:
            parts.append(current.strip())
            current = ""
        else:
            current += character
    parts.append(current.strip())
    return parts


def join_union(parts):
    flat = []
    for part in parts:
        if part.startswith("(") and part.endswith(")") and balanced(part[1:-1]) and "->" not in part:
            members = split_union(part[1:-1])
        else:
            members = [part]
        for member in members:
            if member not in flat:
                flat.append(member)
    if "any" in flat:
        return "any"
    has_nil = "nil" in flat
    flat = [member for member in flat if member != "nil"]
    if has_nil and len(flat) == 1:
        return nullable(flat[0])
    if len(flat) > 1:
        functions = [group(member) for member in flat if "->" in member]
        flat = [member for member in flat if "->" not in member] + functions
    if has_nil:
        flat.insert(0, "nil")
    return " | ".join(flat)


def balanced(text):
    depth = 0
    for character in text:
        if character in "({<[":
            depth += 1
        elif character in ")}>]":
            depth -= 1
            if depth < 0:
                return False
    return depth == 0


def group(kind):
    if " | " in kind or "->" in kind:
        if kind.startswith("(") and kind.endswith(")") and balanced(kind[1:-1]) and "->" not in kind:
            return kind
        return f"({kind})"
    return kind


def nullable(kind):
    if kind in ("any", "nil") or kind.endswith("?") or kind.startswith("...") or kind.startswith("nil | "):
        return kind
    if "->" in kind:
        return f"nil | {group(kind)}"
    return f"{group(kind)}?"


def parse_type(text, aliases, shapes=frozenset(), loose=False):
    parser = TypeParser(text, aliases, shapes, loose)
    kind, rest = parser.parse_prefix()
    return kind, rest


@dataclass
class Member:
    name: str
    kind: str
    doc: list = field(default_factory=list)


@dataclass
class Function:
    name: str
    method: bool
    parameters: list
    returns: list
    doc: list
    parameter_docs: list


@dataclass
class Interface:
    name: str
    parents: list
    doc: list
    fields: list = field(default_factory=list)
    functions: list = field(default_factory=list)


def collect_aliases(files, shapes):
    raw = {}
    for path in files:
        for line in path.read_text().splitlines():
            match = re.match(r"---@alias (\w+) (.*)", line)
            if match:
                raw[match.group(1)] = match.group(2)
    resolved = {"strict": {}, "loose": {}}

    def resolve(name, mode, stack=()):
        if name in resolved[mode]:
            return resolved[mode][name]
        if name in stack:
            raise ParseError(f"recursive alias {name}")
        dependencies = {"strict": {}, "loose": {}}
        for other in raw:
            if other != name and re.search(rf"\b{other}\b", raw[name]):
                dependencies["strict"][other] = resolve(other, "strict", stack + (name,))
                dependencies["loose"][other] = resolve(other, "loose", stack + (name,))
        kind, _ = parse_type(raw[name], dependencies, shapes, mode == "loose")
        resolved[mode][name] = kind
        return kind

    for name in raw:
        resolve(name, "strict")
        resolve(name, "loose")
    return resolved


def collect_shapes(files):
    classes, owners, with_functions = set(), {}, set()
    for path in files:
        current = None
        for line in path.read_text().splitlines():
            class_match = re.match(r"---@class (\w+)", line)
            if class_match:
                current = class_match.group(1)
                classes.add(current)
                continue
            local_match = re.match(r"(?:local )?(\w+) = \{\}", line)
            if local_match and current:
                owners[local_match.group(1)] = current
                continue
            function_match = re.match(r"function (\w+)[.:]", line)
            if function_match and function_match.group(1) in owners:
                with_functions.add(owners[function_match.group(1)])
    return frozenset(classes - with_functions - NANOS_TYPES)


def parse_file(path, aliases, shapes, interfaces, globals_):
    doc, parameters, returns, parameter_docs = [], [], [], []
    current = None
    owners = {}
    for line in path.read_text().splitlines():
        if line.startswith(("---@meta", "---@alias", "---@field [")):
            continue
        class_match = re.match(r"---@class (\w+)(?:\s*:\s*([\w, ]+))?", line)
        if class_match:
            name = class_match.group(1)
            parents = [parent.strip() for parent in (class_match.group(2) or "").split(",") if parent.strip()]
            if name in NANOS_TYPES:
                current = None
                doc = []
                continue
            current = Interface(name, parents, doc)
            interfaces[name] = current
            doc = []
            continue
        field_match = re.match(r"---@field (\w+)(\?)? (.*)", line)
        if field_match:
            if current is not None:
                kind, description = parse_type(field_match.group(3), aliases)
                if field_match.group(2):
                    kind = nullable(kind)
                current.fields.append(Member(field_match.group(1), kind, [description] if description else []))
            continue
        parameter_match = re.match(r"---@param (\.\.\.|\w+)(\?)? (.*)", line)
        if parameter_match:
            name = parameter_match.group(1)
            kind, description = parse_type(parameter_match.group(3), aliases, shapes, True)
            optional = bool(parameter_match.group(2))
            parameters.append((name, kind, optional))
            parameter_docs.append((name, description))
            continue
        return_match = re.match(r"---@return (.*)", line)
        if return_match:
            text = return_match.group(1)
            if text.strip().startswith("..."):
                returns.append(("...any", text.strip()[3:].strip()))
            else:
                kind, description = parse_type(text, aliases)
                returns.append((kind, description))
            continue
        if line.startswith("---@"):
            raise ParseError(f"{path.name}: unsupported annotation '{line}'")
        if line.startswith("---"):
            doc.append(line[3:].strip())
            continue
        local_match = re.match(r"(?:local )?(\w+) = \{\}", line)
        if local_match:
            if current is not None:
                owners[local_match.group(1)] = current.name
                if not line.startswith("local "):
                    globals_[local_match.group(1)] = current.name
            continue
        function_match = re.match(r"function (\w+)([.:])(\w+)\((.*)\) end", line)
        if function_match:
            owner = owners.get(function_match.group(1))
            if owner is None:
                raise ParseError(f"{path.name}: function on unknown table '{function_match.group(1)}'")
            interfaces[owner].functions.append(
                Function(
                    function_match.group(3),
                    function_match.group(2) == ":",
                    parameters,
                    returns,
                    doc,
                    parameter_docs,
                )
            )
            doc, parameters, returns, parameter_docs = [], [], [], []
            continue
        if line.strip() == "":
            if doc and current is None:
                doc = []
            continue
        raise ParseError(f"{path.name}: unexpected line '{line}'")


def comment(lines, indent):
    return [f"{indent}--- {line}".rstrip() for line in lines if line]


def render_function(function, indent):
    output = comment(function.doc, indent)
    for name, description in function.parameter_docs:
        if description:
            output.append(f"{indent}--- @param {name} {description}")
    for _, description in function.returns:
        if description:
            output.append(f"{indent}--- @return {description}")
    parameters = []
    for name, kind, optional in function.parameters:
        if name == "...":
            parameters.append(f"...: {kind}")
        else:
            parameters.append(f"{name}: {nullable(kind) if optional else kind}")
    kinds = [kind for kind, _ in function.returns]
    if not kinds:
        result = "void"
    elif len(kinds) == 1:
        result = kinds[0]
    else:
        result = "(" + ", ".join(kinds) + ")"
    keyword = "method" if function.method else "function"
    output.append(f"{indent}{keyword} {function.name}({', '.join(parameters)}): {result}")
    return output


def render(interfaces, globals_):
    output = [
        "-- Generated from the LuaLS definitions in library/ by tools/nebra-types/generate.py.",
        "-- Do not edit by hand, run `python3 tools/nebra-types/generate.py` instead.",
        "",
    ]
    for interface in interfaces.values():
        output.extend(comment(interface.doc, ""))
        output.append("@side(server)")
        parents = [parent for parent in interface.parents if parent in interfaces]
        header = f"declare interface {interface.name}"
        if parents:
            header += " extends " + ", ".join(parents)
        output.append(header)
        for member in interface.fields:
            output.extend(comment(member.doc, "    "))
            output.append(f"    {member.name}: {member.kind}")
        for function in interface.functions:
            output.extend(render_function(function, "    "))
        output.append("end")
        output.append("")
    for name, interface in globals_.items():
        output.append("@side(server)")
        output.append(f"declare {name}: {interface}")
    output.append("")
    return "\n".join(output)


def main():
    files = sorted(LIBRARY.glob("*.lua"))
    files.sort(key=lambda path: path.name != "quark.lua")
    shapes = collect_shapes(files)
    aliases = collect_aliases(files, shapes)
    interfaces, globals_ = {}, {}
    for path in files:
        parse_file(path, aliases, shapes, interfaces, globals_)
    renamed = {}
    for name, interface in globals_.items():
        if name == interface:
            renamed[name] = f"{name}__Static"
    for old, new in renamed.items():
        interface = interfaces.pop(old)
        interface.name = new
        interfaces = {new: interface, **interfaces}
        globals_[old] = new
    text = render(interfaces, globals_)
    if "--check" in sys.argv:
        if not OUTPUT.exists() or OUTPUT.read_text() != text:
            print(f"{OUTPUT.relative_to(ROOT)} is out of date, run python3 tools/nebra-types/generate.py")
            sys.exit(1)
        print("nebra declarations are up to date")
        return
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(text)
    print(f"wrote {len(interfaces)} interfaces to {OUTPUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
