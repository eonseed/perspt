import os

def resolve_logical(base: str, relative: str) -> str:
    """Historically resolved symlinks against the process cwd."""
    return os.path.realpath(os.path.join(base, relative))
