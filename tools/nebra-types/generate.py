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
OPERATORS = {
    "add": "+",
    "sub": "-",
    "mul": "*",
    "div": "/",
    "idiv": "//",
    "mod": "%",
    "pow": "^",
    "unm": "-",
    "concat": "..",
    "len": "#",
    "lt": "<",
    "le": "<=",
    "band": "&",
    "bor": "|",
    "bxor": "~",
    "shl": "<<",
    "shr": ">>",
    "bnot": "~",
    "call": "()",
}


class ParseError(Exception):
    pass


class TypeParser:
    def __init__(self, text, aliases):
        self.text = text
        self.position = 0
        self.aliases = aliases

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
        while self.peek("|"):
            self.position += 1
            parts.append(self.postfix())
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
            self.position = self.text.index(quote, self.position + 1) + 1
            return "string"
        if self.peek("{"):
            return self.table()
        number = re.compile(r"-?[0-9][0-9.]*").match(self.text, self.position)
        if number:
            self.position = number.end()
            return "number"
        if self.accept("..."):
            return "...any"
        name = self.name()
        if name == "fun":
            return self.function_type()
        if name == "table" and self.accept("<"):
            key = self.union()
            self.expect(",")
            value = self.union()
            self.expect(">")
            return f"{{ [{key}]: {value} }}"
        if name in ("true", "false"):
            return "boolean"
        if name in PRIMITIVES:
            return PRIMITIVES[name]
        if name in self.aliases:
            alias = self.aliases[name]
            return f"({alias})" if "|" in alias else alias
        return name

    def function_type(self):
        self.expect("(")
        parameters = []
        if not self.accept(")"):
            while True:
                if self.accept("..."):
                    kind = self.union() if self.accept(":") else "any"
                    parameters.append(f"...{group(kind)}")
                else:
                    self.name()
                    optional = self.accept("?")
                    kind = self.union() if self.accept(":") else "any"
                    parameters.append(nullable(kind) if optional else kind)
                if self.accept(")"):
                    break
                self.expect(",")
        returns = []
        if self.accept(":"):
            returns.append(self.return_type())
            while self.peek(",") and self.continues_returns():
                self.expect(",")
                returns.append(self.return_type())
        return f"({', '.join(parameters)}) -> {tuple_of(returns)}"

    def continues_returns(self):
        saved = self.position
        try:
            self.expect(",")
            self.skip_spaces()
            return not re.compile(r"[A-Za-z_][A-Za-z0-9_]*\??\s*:").match(self.text, self.position)
        except ParseError:
            return False
        finally:
            self.position = saved

    def return_type(self):
        if self.accept("..."):
            return "...any"
        parts = [self.postfix()]
        while self.peek("|"):
            self.position += 1
            parts.append(self.postfix())
        return group(join_union(parts))

    def table(self):
        self.expect("{")
        if self.accept("}"):
            return "{ [any]: any }"
        if self.accept("["):
            key = self.union()
            self.expect("]")
            self.expect(":")
            value = self.union()
            if self.accept(","):
                while not self.accept("}"):
                    self.expect("[")
                    self.union()
                    self.expect("]")
                    self.expect(":")
                    self.union()
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


def is_grouped(kind):
    return kind.startswith("(") and kind.endswith(")") and balanced(kind[1:-1])


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
    members = []
    for part in parts:
        inner = split_union(part[1:-1]) if is_grouped(part) and "->" not in part else [part]
        for member in inner:
            if member not in members:
                members.append(member)
    if "any" in members:
        return "any"
    has_nil = "nil" in members
    members = [member for member in members if member != "nil"]
    if has_nil and len(members) == 1:
        return nullable(members[0])
    if len(members) > 1:
        members = [group(member) if "->" in member else member for member in members]
    if has_nil:
        members.append("nil")
    return " | ".join(members)


def group(kind):
    if (" | " in kind or "->" in kind) and not (is_grouped(kind) and "->" not in kind[1:-1].replace(") -> ", "")):
        return f"({kind})"
    return kind


def nullable(kind):
    if kind in ("any", "nil") or kind.endswith("?") or kind.startswith("..."):
        return kind
    return f"{group(kind)}?"


def tuple_of(kinds):
    if not kinds:
        return "void"
    if len(kinds) == 1:
        return kinds[0]
    return "(" + ", ".join(kinds) + ")"


def parse_type(text, aliases):
    return TypeParser(text, aliases).parse_prefix()


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
class Operator:
    symbol: str
    parameters: list
    result: str


@dataclass
class Interface:
    name: str
    parents: list
    doc: list
    fields: list = field(default_factory=list)
    functions: list = field(default_factory=list)
    operators: list = field(default_factory=list)

    @property
    def is_class(self):
        return bool(self.operators) or any(function.method for function in self.functions)


def collect_aliases(files):
    raw = {}
    for path in files:
        for line in path.read_text().splitlines():
            match = re.match(r"---@alias (\w+) (.*)", line)
            if match:
                raw[match.group(1)] = match.group(2)
    resolved = {}

    def resolve(name, stack=()):
        if name in resolved:
            return resolved[name]
        if name in stack:
            raise ParseError(f"recursive alias {name}")
        dependencies = {
            other: resolve(other, stack + (name,))
            for other in raw
            if other != name and re.search(rf"\b{other}\b", raw[name])
        }
        resolved[name] = parse_type(raw[name], dependencies)[0]
        return resolved[name]

    for name in raw:
        resolve(name)
    return resolved


def parse_operator(text, aliases):
    match = re.match(r"(\w+)(?:\((.*?)\))?\s*:\s*(.*)", text)
    if not match or match.group(1) not in OPERATORS:
        raise ParseError(f"unsupported operator '{text}'")
    name, operand, result = match.groups()
    parameters = []
    if operand:
        kind, _ = parse_type(operand, aliases)
        parameters.append(("variables" if name == "call" else "other", kind))
    returns = []
    parser = TypeParser(result, aliases)
    returns.append(parser.return_type())
    while parser.accept(","):
        returns.append(parser.return_type())
    return Operator(OPERATORS[name], parameters, tuple_of(returns))


def parse_file(path, aliases, interfaces, globals_):
    doc, parameters, returns, parameter_docs = [], [], [], []
    current = None
    owners = {}
    for line in path.read_text().splitlines():
        if line.startswith(("---@meta", "---@alias", "---@diagnostic")):
            continue
        class_match = re.match(r"---@class (\w+)(?:\s*:\s*([\w, ]+))?", line)
        if class_match:
            name = class_match.group(1)
            parents = [parent.strip() for parent in (class_match.group(2) or "").split(",") if parent.strip()]
            current = None if name in NANOS_TYPES else Interface(name, parents, doc)
            if current:
                interfaces[name] = current
            doc = []
            continue
        index_match = re.match(r"---@field \[(\w+)\] (.*)", line)
        if index_match:
            if current is not None:
                key, _ = parse_type(index_match.group(1), aliases)
                value, _ = parse_type(index_match.group(2), aliases)
                current.operators.append(Operator("[]", [("index", key)], value))
                current.operators.append(Operator("[]=", [("index", key), ("value", value)], "void"))
            continue
        field_match = re.match(r"---@field (\w+)(\?)? (.*)", line)
        if field_match:
            if current is not None:
                kind, description = parse_type(field_match.group(3), aliases)
                if field_match.group(2):
                    kind = nullable(kind)
                current.fields.append(Member(field_match.group(1), kind, [description] if description else []))
            continue
        operator_match = re.match(r"---@operator (.*)", line)
        if operator_match:
            if current is not None:
                current.operators.append(parse_operator(operator_match.group(1), aliases))
            continue
        parameter_match = re.match(r"---@param (\.\.\.|\w+)(\?)? (.*)", line)
        if parameter_match:
            name = parameter_match.group(1)
            kind, description = parse_type(parameter_match.group(3), aliases)
            parameters.append((name, kind, bool(parameter_match.group(2))))
            parameter_docs.append((name, description))
            continue
        return_match = re.match(r"---@return (.*)", line)
        if return_match:
            text = return_match.group(1).strip()
            if text.startswith("..."):
                returns.append(("...any", text[3:].strip()))
            else:
                returns.append(parse_type(text, aliases))
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
            if current is None:
                doc = []
            continue
        raise ParseError(f"{path.name}: unexpected line '{line}'")


def comment(lines, indent):
    return [f"{indent}--- {line}".rstrip() for line in lines if line]


def render_function(function, indent, keyword):
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
    result = tuple_of([kind for kind, _ in function.returns])
    output.append(f"{indent}{keyword} {function.name}({', '.join(parameters)}): {result}")
    return output


def render_operator(operator, indent):
    parameters = ", ".join(f"{name}: {kind}" for name, kind in operator.parameters)
    return f"{indent}operator {operator.symbol}({parameters}): {operator.result}"


def render_header(interface, interfaces):
    known = [parent for parent in interface.parents if parent in interfaces]
    if not interface.is_class:
        if any(interfaces[parent].is_class for parent in known):
            raise ParseError(f"interface {interface.name} cannot extend a class")
        header = f"declare interface {interface.name}"
        return header + (" extends " + ", ".join(known) if known else "")
    classes = [parent for parent in known if interfaces[parent].is_class]
    contracts = [parent for parent in known if not interfaces[parent].is_class]
    if len(classes) > 1:
        raise ParseError(f"class {interface.name} extends more than one class")
    header = f"declare class {interface.name}"
    if classes:
        header += f" extends {classes[0]}"
    if contracts:
        header += " implements " + ", ".join(contracts)
    return header


def render(interfaces, globals_):
    output = [
        "-- Generated from the LuaLS definitions in library/ by tools/nebra-types/generate.py.",
        "-- Do not edit by hand, run `python3 tools/nebra-types/generate.py` instead.",
        "",
    ]
    for interface in interfaces.values():
        output.extend(comment(interface.doc, ""))
        output.append("@side(server)")
        output.append(render_header(interface, interfaces))
        for member in interface.fields:
            output.extend(comment(member.doc, "    "))
            output.append(f"    {member.name}: {member.kind}")
        for operator in interface.operators:
            output.append(render_operator(operator, "    "))
        for function in interface.functions:
            if interface.is_class:
                keyword = "function" if function.method else "static function"
            else:
                keyword = "method" if function.method else "function"
            output.extend(render_function(function, "    ", keyword))
        output.append("end")
        output.append("")
    for name, interface in globals_.items():
        output.append("@side(server)")
        output.append(f"declare {name}: {interface}")
    output.append("")
    return "\n".join(output)


def main():
    files = sorted(LIBRARY.glob("*.lua"), key=lambda path: (path.name != "quark.lua", path.name))
    aliases = collect_aliases(files)
    interfaces, globals_ = {}, {}
    for path in files:
        parse_file(path, aliases, interfaces, globals_)
    for name, interface_name in list(globals_.items()):
        if name == interface_name:
            interface = interfaces.pop(interface_name)
            interface.name = f"{name}__Static"
            interfaces = {interface.name: interface, **interfaces}
            globals_[name] = interface.name
    text = render(interfaces, globals_)
    if "--check" in sys.argv:
        if not OUTPUT.exists() or OUTPUT.read_text() != text:
            print(f"{OUTPUT.relative_to(ROOT)} is out of date, run python3 tools/nebra-types/generate.py")
            sys.exit(1)
        print("nebra declarations are up to date")
        return
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(text)
    print(f"wrote {len(interfaces)} declarations to {OUTPUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
