import pathlib, subprocess, sys
root = pathlib.Path(__file__).parents[1]
sys.path.insert(0, str(root / "python"))
from service import lookup

def rust(key):
    value = subprocess.run(["cargo","run","--quiet","--",key], cwd=root, text=True, capture_output=True, check=True).stdout.strip()
    return None if value == "none" else int(value)

def test_both_registries_agree():
    for key, expected in [("entry-000000",0),("release-channel",5111),("entry-006999",6999),("missing",None)]:
        assert lookup(key) == rust(key) == expected
