import pathlib, subprocess, sys
root = pathlib.Path(__file__).parents[1]
sys.path.insert(0, str(root / "python"))
from units import parse_size

def rust(value):
    raw = subprocess.run(["cargo","run","--quiet","--",value], cwd=root, text=True, capture_output=True, check=True).stdout.strip()
    return None if raw == "none" else int(raw)

def test_parity():
    cases = [(" 2K ",2048),("1m",1048576),("42",42),("x1",None),("18446744073709551615k",None)]
    for raw, expected in cases: assert parse_size(raw) == rust(raw) == expected
