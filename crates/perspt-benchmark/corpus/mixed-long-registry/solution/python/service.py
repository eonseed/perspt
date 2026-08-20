from catalog import ENTRIES
def lookup(key):
    return next((value for candidate, value in ENTRIES if candidate == key), None)
