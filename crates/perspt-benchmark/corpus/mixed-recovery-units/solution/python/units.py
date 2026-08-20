def parse_size(value):
    value = value.strip().lower(); multiplier = 1
    if value.endswith(("k","m","g")):
        multiplier = {"k":1024,"m":1024**2,"g":1024**3}[value[-1]]; value = value[:-1]
    if not value.isascii() or not value.isdigit(): return None
    result = int(value) * multiplier
    return result if result <= 2**64-1 else None
