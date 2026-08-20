def resolve_logical(base: str, relative: str) -> str:
    if "\0" in base or "\0" in relative: raise ValueError("NUL")
    trailing = relative.endswith("/")
    raw = relative if relative.startswith("/") else base.rstrip("/") + "/" + relative
    parts = []
    for part in raw.split("/"):
        if not part or part == ".": continue
        if part == "..":
            if parts: parts.pop()
        else: parts.append(part)
    result = "/" + "/".join(parts)
    if trailing and result != "/": result += "/"
    return result
