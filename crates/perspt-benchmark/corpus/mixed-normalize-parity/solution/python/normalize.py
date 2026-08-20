def normalize(value: str) -> str:
    out = []; separator = False
    for char in value.strip(" \t\n\r\v\f"):
        if char.isascii() and char.isalnum():
            if separator and out: out.append("-")
            separator = False; out.append(char.lower())
        else: separator = True
    return "".join(out)
