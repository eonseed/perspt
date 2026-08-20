from catalog import ENTRIES

def lookup(key: str):
    return next((value for candidate, value in ENTRIES if candidate == key), None)
