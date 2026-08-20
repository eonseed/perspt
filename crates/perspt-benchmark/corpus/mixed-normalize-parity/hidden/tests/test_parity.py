import pathlib, subprocess, sys
root = pathlib.Path(__file__).parents[1]
sys.path.insert(0, str(root / "python"))
from normalize import normalize

def rust(value):
    return subprocess.run(["cargo","run","--quiet","--",value], cwd=root, text=True, capture_output=True, check=True).stdout.rstrip("\n")

def test_parity_and_contract():
    for value, expected in [(" Hello__WORLD! ","hello-world"),("--a  b--","a-b"),("","")]:
        assert normalize(value) == expected
        assert rust(value) == expected
